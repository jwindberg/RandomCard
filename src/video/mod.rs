//! Movies: playing video files (MOV, MP4, M4V, MPEG) with the system's own
//! media framework, which decodes them (in hardware where it can) and plays
//! their sound. macOS uses AVFoundation, Windows uses Media Foundation, and
//! Linux uses GStreamer, loaded when the first movie opens, so RandomCard
//! runs without it and only movies need it.
//!
//! A `Player` is one open movie. Scripts ask it the time and set its rate
//! from the script thread; the UI takes its frames to draw. It's safe to
//! share between them.

use std::path::Path;
use std::sync::Arc;

#[cfg(target_os = "linux")]
mod gstreamer;
#[cfg(target_os = "linux")]
mod palette;
#[cfg(target_os = "linux")]
use gstreamer as imp;

#[cfg(target_os = "macos")]
mod avfoundation;
#[cfg(target_os = "macos")]
mod avplayer;
#[cfg(target_os = "macos")]
use avfoundation as imp;

#[cfg(windows)]
mod mediafoundation;
#[cfg(windows)]
use mediafoundation as imp;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
use unsupported as imp;

/// One picture of a movie, in RGBA.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Counts the frames a movie has shown, so the UI knows a new one.
    pub serial: u64,
}

/// An open movie.
pub struct Player(imp::Player);

impl Player {
    /// Open a movie file, paused on its first frame.
    pub fn open(path: &Path) -> Result<Player, String> {
        imp::Player::open(path).map(Player)
    }

    /// Open a movie on the web (http or https), paused on its first frame.
    pub fn open_url(url: &str) -> Result<Player, String> {
        imp::Player::open_url(url).map(Player)
    }

    /// The movie's size in pixels, or 0,0 if it has no picture (sound only).
    pub fn size(&self) -> (u32, u32) {
        self.0.size()
    }

    /// How long the movie is, in seconds.
    pub fn duration(&self) -> f64 {
        self.0.duration()
    }

    /// Where the movie is now, in seconds.
    pub fn time(&self) -> f64 {
        self.0.time()
    }

    /// Go to this time, in seconds, keeping the rate.
    pub fn seek(&self, secs: f64) {
        self.0.seek(secs.clamp(0.0, self.duration().max(0.0)))
    }

    /// Play at this rate: 1 is forward at normal speed, 0 is paused, and
    /// below 0 plays backward.
    pub fn set_rate(&self, rate: f64) {
        self.0.set_rate(rate)
    }

    /// The sound's level, 1 for as recorded and 0 for silent.
    pub fn set_volume(&self, level: f64) {
        self.0.set_volume(level.max(0.0))
    }

    /// The newest picture, which stays until another comes.
    pub fn frame(&self) -> Option<Arc<Frame>> {
        self.0.frame()
    }

    /// It has played to its end (or its start, backward) and stopped.
    pub fn finished(&self) -> bool {
        self.0.finished()
    }
}

/// What the movie files RandomCard plays are called.
pub const EXTENSIONS: [&str; 6] = ["mov", "mp4", "m4v", "mpg", "mpeg", "m2v"];

/// Whether a file is a movie, by its name or, for a classic Mac movie with
/// no extension, by what's in it (see `movie_type`).
pub fn is_movie(path: &Path) -> bool {
    if path.extension().is_some_and(|e| EXTENSIONS.iter().any(|m| e.eq_ignore_ascii_case(m))) {
        return true;
    }
    let mut head = [0u8; 12];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let Ok(n) = std::io::Read::read(&mut f, &mut head) else { return false };
    movie_type(&head[..n]).is_some()
}

/// What kind of movie these first bytes start, as a MIME type: a QuickTime
/// or MP4 atom at the start (an `ftyp` names its brand), or an MPEG start
/// code.
pub fn movie_type(head: &[u8]) -> Option<&'static str> {
    if head.len() >= 8 {
        let atom = &head[4..8];
        if atom == b"ftyp" {
            return Some(if head.get(8..12) == Some(b"qt  ") { "video/quicktime" } else { "video/mp4" });
        }
        const QT: [&[u8; 4]; 6] = [b"moov", b"mdat", b"wide", b"free", b"skip", b"pnot"];
        if QT.iter().any(|a| atom == &a[..]) {
            return Some("video/quicktime");
        }
    }
    if head.len() >= 4 && (head[..4] == [0, 0, 1, 0xBA] || head[..4] == [0, 0, 1, 0xB3]) {
        return Some("video/mpeg");
    }
    None
}

/// The type to tell the player a movie on the web is, when its server
/// doesn't say: a `Content-Type` that names no kind of media (often
/// `application/octet-stream`, for an address with no extension) leaves
/// the movie's first bytes to tell. None when the server's type will do,
/// or the bytes don't say either.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn web_movie_type(content_type: &str, head: &[u8]) -> Option<&'static str> {
    let ct = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if ct.starts_with("video/") || ct.starts_with("audio/") || ct.contains("mpegurl") {
        return None;
    }
    if head.starts_with(b"#EXTM3U") {
        return Some("application/vnd.apple.mpegurl");
    }
    movie_type(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_movie_on_the_web_says_what_it_is() {
        let mp4 = b"\0\0\0\x1cftypmp42\0\0\0\0";
        let qt = b"\0\0\0\x14ftypqt  \0\0\0\0";
        // The server says nothing useful: the bytes tell.
        assert_eq!(web_movie_type("application/octet-stream", mp4), Some("video/mp4"));
        assert_eq!(web_movie_type("", qt), Some("video/quicktime"));
        assert_eq!(web_movie_type("binary/octet-stream", b"\0\0\0\x08moov"), Some("video/quicktime"));
        assert_eq!(web_movie_type("application/octet-stream", &[0, 0, 1, 0xBA, 0x44]), Some("video/mpeg"));
        assert_eq!(web_movie_type("text/plain", b"#EXTM3U\n"), Some("application/vnd.apple.mpegurl"));
        // The server's own type stands.
        assert_eq!(web_movie_type("video/mp4", mp4), None);
        assert_eq!(web_movie_type("Video/QuickTime; charset=binary", qt), None);
        assert_eq!(web_movie_type("application/vnd.apple.mpegurl", b"#EXTM3U"), None);
        // Neither says.
        assert_eq!(web_movie_type("application/octet-stream", b"<html>"), None);
        assert_eq!(web_movie_type("application/octet-stream", b""), None);
    }

    #[test]
    fn knows_a_movie() {
        let tour = concat!(env!("CARGO_MANIFEST_DIR"), "/src/model/tour_movie/Card Trick.mp4");
        assert!(is_movie(Path::new(tour)));
        assert!(!is_movie(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))));
        assert!(!is_movie(Path::new("/no/such/file")));
        // A classic Mac movie, with no extension (when the stacks are here).
        let simple = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/apple/hypercard_241_stacks/HyperCard Stacks/Simple Movie"));
        if simple.exists() {
            assert!(is_movie(simple));
            // Its 8-bit Graphics frames come out opaque, as the system can
            // decode them.
            if let Ok(p) = Player::open(simple) {
                let f = p.frame().expect("a first frame");
                assert_eq!((f.width, f.height), (206, 243));
                assert!(f.rgba.as_chunks::<4>().0.iter().all(|px| px[3] == 255));
            }
        }
    }
}

