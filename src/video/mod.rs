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
/// no extension, by what's in it: a QuickTime or MP4 atom at the start, or
/// an MPEG start code.
pub fn is_movie(path: &Path) -> bool {
    if path.extension().is_some_and(|e| EXTENSIONS.iter().any(|m| e.eq_ignore_ascii_case(m))) {
        return true;
    }
    let mut head = [0u8; 8];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    if std::io::Read::read_exact(&mut f, &mut head).is_err() {
        return false;
    }
    const ATOMS: [&[u8; 4]; 7] = [b"ftyp", b"moov", b"mdat", b"wide", b"free", b"skip", b"pnot"];
    ATOMS.iter().any(|a| head[4..8] == a[..]) || head[..4] == [0, 0, 1, 0xBA] || head[..4] == [0, 0, 1, 0xB3]
}

#[cfg(test)]
mod tests {
    use super::*;

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

