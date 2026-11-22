//! Movies, as HyperCard 2.4 played them: `Movie "Intro", "windoid", "card"`
//! opens a movie file in a movie window, which scripts reach as `movie 1`,
//! `movie "Intro"` or `window "Intro"`, play and pause (`play movie 1`,
//! `send "rewind" to movie 1`), and control with properties (`set the rate
//! of movie 1 to 2`). HyperCard did this with Apple's Movie XCMD and
//! QuickTime; RandomCard does it itself, in today's movie formats (see
//! `crate::video`). Movie windows are drawn in the card window, where
//! scripts place them in card coordinates, as HyperCard placed them.
//!
//! Times are HyperCard's: "SystemTime" is hours, minutes, seconds and ticks
//! (sixtieths), written right to left, so "2,30" is two and a half seconds;
//! "MovieTime" is a count of the movie's time units, 600 a second.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::hypertalk::ast::Ordinal;
use crate::hypertalk::value::{bool_str, format_number, parse_point, parse_rect};
use crate::model::Rect;
use crate::video::Player;

use super::interp::{Interp, R, err, to_bool, to_number};
use super::world::{Event, World};

/// Movie time units in a second, `the timeScale of movie 1`.
pub const TIME_SCALE: f64 = 600.0;

/// The height of the movie controller, under the movie.
pub const CONTROLLER: i32 = 16;

/// A movie window's look (the Movie command's second parameter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Windoid,
    TallWindoid,
    Plain,
    Zoom,
    Dialog,
    Shadow,
    Rect,
    Borderless,
}

impl Style {
    fn parse(s: &str) -> Option<Style> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "" | "windoid" => Style::Windoid,
            "tallwindoid" => Style::TallWindoid,
            "plain" | "document" => Style::Plain,
            "zoom" => Style::Zoom,
            "dialog" => Style::Dialog,
            "shadow" => Style::Shadow,
            "rect" | "rectangle" => Style::Rect,
            "borderless" => Style::Borderless,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Style::Windoid => "windoid",
            Style::TallWindoid => "tallWindoid",
            Style::Plain => "plain",
            Style::Zoom => "zoom",
            Style::Dialog => "dialog",
            Style::Shadow => "shadow",
            Style::Rect => "rect",
            Style::Borderless => "borderLess",
        }
    }

    /// The title bar's height above the window's rect: none for the
    /// borderless styles.
    pub fn title_bar(self) -> i32 {
        match self {
            Style::Windoid => 11,
            Style::TallWindoid => 16,
            Style::Plain | Style::Zoom => 19,
            Style::Dialog | Style::Shadow | Style::Rect | Style::Borderless => 0,
        }
    }

    /// Whether the window shows its name in its title bar.
    pub fn titled(self) -> bool {
        matches!(self, Style::Plain | Style::Zoom | Style::TallWindoid)
    }

    /// Plain and zoom windows go with the card window; the others float
    /// over it, as palettes do.
    fn floats(self) -> bool {
        !matches!(self, Style::Plain | Style::Zoom)
    }
}

/// An open movie window.
pub struct MovieWindow {
    pub id: u32,
    /// The window's name, shown in its title bar: at first the file's name.
    pub name: String,
    pub path: PathBuf,
    pub style: Style,
    /// Where the window is on the card, not counting its title bar: the
    /// movie, and the controller under it when it shows.
    pub rect: Rect,
    /// Where the movie is in the window, which can be stretched.
    pub movie_rect: Rect,
    pub visible: bool,
    pub floating: bool,
    pub controller: bool,
    pub badge: bool,
    /// Clicks go to the movie: a click pauses it, a double-click plays it.
    pub pass_click: bool,
    pub looping: bool,
    pub palindrome: bool,
    pub close_on_finish: bool,
    /// The part of the movie that plays, in seconds.
    pub start: f64,
    pub end: f64,
    /// Times are "MovieTime" rather than "SystemTime".
    pub movie_time: bool,
    /// 256 is as recorded; negative is muted, keeping the level.
    pub volume: i32,
    /// The rate as last set: 0 paused, 1 playing, below 0 backward.
    pub rate: f64,
    /// QuickTime's properties RandomCard keeps for scripts to read back but
    /// doesn't act on (fastIdle, seeAllFrames, dontErase, ...).
    pub other: HashMap<String, String>,
    pub player: Arc<Player>,
}

/// What a movie did at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    Playing,
    /// It stopped at its end, or its start backward.
    Stopped,
    /// It stopped and closes (closeOnFinish).
    Close,
}

impl MovieWindow {
    /// The movie's own size: 0,0 for sound only.
    pub fn natural_size(&self) -> (i32, i32) {
        let (w, h) = self.player.size();
        (w as i32, h as i32)
    }

    pub fn duration(&self) -> f64 {
        self.player.duration()
    }

    pub fn time(&self) -> f64 {
        self.player.time()
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate.clamp(-5.0, 5.0);
        self.player.set_rate(self.rate);
    }

    /// Play forward from where it is, or from the start once it's done.
    pub fn play(&mut self) {
        if self.time() >= self.end - 0.01 {
            self.player.seek(self.start);
        }
        self.set_rate(1.0);
    }

    pub fn pause(&mut self) {
        self.set_rate(0.0);
    }

    /// Go to a time and stop there.
    pub fn go_to(&mut self, secs: f64) {
        self.set_rate(0.0);
        self.player.seek(secs.clamp(0.0, self.duration()));
    }

    fn apply_volume(&self) {
        self.player.set_volume(self.volume.max(0) as f64 / 256.0);
    }

    /// Show or hide the controller, which the window grows or shrinks for.
    pub fn show_controller(&mut self, show: bool) {
        if show != self.controller {
            self.rect.bottom += if show { CONTROLLER } else { -CONTROLLER };
            self.controller = show;
        }
    }

    /// Keep it within its start and end times, looping or stopping there.
    /// The UI calls this as it draws.
    pub fn tick(&mut self) -> Tick {
        if self.rate == 0.0 {
            self.player.finished();
            return Tick::Playing;
        }
        let done = self.player.finished();
        let t = self.time();
        let ended = if self.rate > 0.0 { done || t >= self.end } else { done || t <= self.start };
        if !ended {
            return Tick::Playing;
        }
        if self.looping {
            if self.palindrome {
                self.set_rate(-self.rate);
            } else {
                let rate = self.rate;
                self.player.seek(if rate > 0.0 { self.start } else { self.end });
                self.player.set_rate(0.0);
                self.player.set_rate(rate);
            }
            return Tick::Playing;
        }
        let at = if self.rate > 0.0 { self.end } else { self.start };
        self.go_to(at);
        if self.close_on_finish { Tick::Close } else { Tick::Stopped }
    }

    /// A time as scripts see it, in the movie's time format.
    pub fn time_text(&self, secs: f64) -> String {
        if self.movie_time { format!("{}", (secs * TIME_SCALE).round() as i64) } else { system_time(secs) }
    }

    fn parse_time(&self, s: &str) -> R<f64> {
        let t = if self.movie_time { to_number(s)? / TIME_SCALE } else { parse_system_time(s).ok_or(()).or_else(|_| err(format!("Expected a time here, not \"{s}\"")))? };
        Ok(t.max(0.0))
    }
}

/// Seconds as HyperCard's "SystemTime": hours, minutes, seconds, ticks,
/// leaving out the leading parts that are 0.
pub fn system_time(secs: f64) -> String {
    let mut ticks = (secs.max(0.0) * 60.0 * 100.0).round() / 100.0;
    let mut parts = Vec::new();
    for _ in 0..3 {
        parts.push(format_number(ticks % 60.0, "0.##"));
        ticks = (ticks / 60.0).floor();
        if ticks == 0.0 {
            break;
        }
    }
    if ticks > 0.0 {
        parts.push(format!("{ticks}"));
    }
    parts.reverse();
    parts.join(",")
}

/// "1,2,30" is a minute, two seconds and 30 ticks.
pub fn parse_system_time(s: &str) -> Option<f64> {
    let mut secs = 0.0;
    for (n, part) in s.split(',').rev().enumerate() {
        let v: f64 = part.trim().parse().ok()?;
        secs += v * match n {
            0 => 1.0 / 60.0,
            1 => 1.0,
            2 => 60.0,
            3 => 3600.0,
            _ => return None,
        };
    }
    Some(secs)
}

/// A rect as scripts see it.
fn rect_text(r: Rect) -> String {
    format!("{},{},{},{}", r.left, r.top, r.right, r.bottom)
}

/// The QuickTime properties RandomCard keeps without acting on, with what
/// they start as.
const KEPT: &[(&str, &str)] = &[
    ("allowballoonhelp", "false"),
    ("backcolor", "65535,65535,65535"),
    ("callbackmsg", ""),
    ("callbacktime", "0"),
    ("callbackwhendragging", "false"),
    ("cursorid", "0"),
    ("donterase", "false"),
    ("dragenabled", "false"),
    ("enabled", "true"),
    ("fastidle", "false"),
    ("graphicsmode", "ditherCopy"),
    ("highqualitymode", "false"),
    ("idlefrequency", "0"),
    ("imagequality", "normal"),
    ("language", "English"),
    ("lockmessages", "false"),
    ("lockscreen", "false"),
    ("seeallframes", "false"),
    ("sendmousewithinmsg", "false"),
    ("showhotspots", "false"),
    ("suppresshotspotnames", "false"),
    ("usecustomclut", "false"),
];

/// A rect property's new rect: `set the loc of window "Intro" to 10,20`
/// moves it; `set the width …` changes its width from its left.
fn changed_rect(r: Rect, prop: &str, value: &str) -> Option<Rect> {
    let n = || value.trim().parse::<f64>().ok().map(|v| v.round() as i32);
    Some(match prop {
        "rect" | "rectangle" => {
            let (l, t, rt, b) = parse_rect(value)?;
            Rect::new(l, t, rt, b)
        }
        "loc" | "location" | "topleft" => {
            let (x, y) = parse_point(value)?;
            r.translate(x - r.left, y - r.top)
        }
        "botright" | "bottomright" => {
            let (x, y) = parse_point(value)?;
            Rect::new(r.left, r.top, x, y)
        }
        "left" => r.translate(n()? - r.left, 0),
        "top" => r.translate(0, n()? - r.top),
        "right" => r.translate(n()? - r.right, 0),
        "bottom" => r.translate(0, n()? - r.bottom),
        "width" => Rect::new(r.left, r.top, r.left + n()?, r.bottom),
        "height" => Rect::new(r.left, r.top, r.right, r.top + n()?),
        _ => return None,
    })
}

fn rect_property(r: Rect, prop: &str) -> Option<String> {
    Some(match prop {
        "rect" | "rectangle" => rect_text(r),
        "loc" | "location" | "topleft" => format!("{},{}", r.left, r.top),
        "botright" | "bottomright" => format!("{},{}", r.right, r.bottom),
        "left" => r.left.to_string(),
        "top" => r.top.to_string(),
        "right" => r.right.to_string(),
        "bottom" => r.bottom.to_string(),
        "width" => r.width().to_string(),
        "height" => r.height().to_string(),
        _ => return None,
    })
}

impl World {
    /// The movie a script names: `movie id 1001` (what `movie 1` comes
    /// to), a window's name, or its movie file. And whether it was named
    /// as a window, whose rect is the window's rather than the movie's.
    pub fn find_movie(&self, name: &str) -> Option<(usize, bool)> {
        let name = name.trim();
        let lower = name.to_ascii_lowercase();
        for (prefix, window) in [("movie id ", false), ("window id ", true)] {
            if let Some(id) = lower.strip_prefix(prefix) {
                let id: u32 = id.trim().parse().ok()?;
                return self.movies.iter().position(|m| m.id == id).map(|i| (i, window));
            }
        }
        if let Some(i) = self.movies.iter().position(|m| m.name.eq_ignore_ascii_case(name)) {
            return Some((i, true));
        }
        // A file's name or path, as HyperCard matched the movie property.
        let file = super::paths::native(name);
        self.movies
            .iter()
            .position(|m| {
                m.path == file
                    || super::paths::shown(&m.path).eq_ignore_ascii_case(name)
                    || m.path.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))
                    || m.path.file_stem().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))
            })
            .map(|i| (i, true))
    }
}

impl Interp {
    /// The movie window a window name or `movie id N` names, unless a
    /// script's own window has that name.
    pub(super) fn movie_named(&self, name: &str) -> Option<(usize, bool)> {
        let w = self.w();
        if w.script_windows.iter().any(|t| t.name.eq_ignore_ascii_case(name.trim())) {
            return None;
        }
        w.find_movie(name)
    }

    /// Send a movie message to the current card once the script running
    /// now is done, as the movie window would.
    fn post_movie_message(&self, message: &str, args: Vec<String>) {
        if let Some(tx) = &self.event_tx {
            let target = self.current_card_ref();
            let _ = tx.send(Event::Send { target, message: message.into(), args });
        }
    }

    /// The movie file a script names: as given, beside the stack, in a
    /// folder beside it, with one of the movie extensions, or chosen in a
    /// file dialog when no name is given.
    pub(super) fn find_movie_file(&self, name: &str) -> Option<PathBuf> {
        if name.trim().is_empty() {
            return self.file_dialog("Open which movie?", super::externals::FileDialog::Open);
        }
        if let Some(p) = self.find_beside_stack(name.trim()) {
            return Some(p);
        }
        if let Some(p) = crate::video::EXTENSIONS.iter().find_map(|ext| self.find_beside_stack(&format!("{}.{ext}", name.trim()))) {
            return Some(p);
        }
        // The Tour's own movie, written out for the movie player to read.
        let (file, bytes) = crate::model::demo::builtin_movie(name)?;
        let dir = std::env::temp_dir().join("RandomCard");
        let path = dir.join(file);
        if std::fs::metadata(&path).ok().map(|m| m.len()) != Some(bytes.len() as u64) {
            std::fs::create_dir_all(&dir).ok()?;
            std::fs::write(&path, bytes).ok()?;
        }
        Some(path)
    }

    /// The Movie command: `Movie file, style, location, visible, layer`.
    /// Errors go in the result, as the Movie XCMD's did.
    pub(super) fn open_movie(&mut self, args: &[String]) -> R<()> {
        let arg = |i: usize| args.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        self.result.clear();
        let Some(style) = Style::parse(&arg(1)) else {
            self.result = format!("Movie windows have no style \"{}\"", arg(1));
            return Ok(());
        };
        // A movie on the web plays from its address; anything else is a
        // file.
        let url = is_url(&arg(0)).then(|| arg(0));
        let path = match &url {
            Some(u) => PathBuf::from(u),
            None => match self.find_movie_file(&arg(0)) {
                Some(p) => p,
                None => {
                    if !arg(0).is_empty() {
                        self.result = format!("Can't find the movie \"{}\"", arg(0));
                    }
                    return Ok(());
                }
            },
        };
        let opened = match &url {
            // Under a connection's Address: with its credentials, asking
            // for any settings still needed.
            Some(u) => match self.credentials(u)? {
                Ok(c) => Player::open_url(u, c.as_ref()),
                Err(why) => Err(why),
            },
            None => Player::open(&path),
        };
        let player = match opened {
            Ok(p) => Arc::new(p),
            Err(e) => {
                self.result = e;
                return Ok(());
            }
        };
        let (mw, mh) = {
            let (w, h) = player.size();
            (w as i32, h as i32)
        };
        let mut w = self.w();
        let (cw, ch) = (w.stack.width as i32, w.stack.height as i32);
        // A sound-only movie is a controller alone.
        let controller = mh == 0;
        let (ww, wh) = (mw.max(if controller { 160 } else { 1 }), mh + if controller { CONTROLLER } else { 0 });
        let location = arg(2).to_ascii_lowercase();
        let rect = if let Some((l, t, r, b)) = parse_rect(&location) {
            Rect::new(l, t, r, b)
        } else if let Some((x, y)) = parse_point(&location) {
            Rect::new(x, y, x + ww, y + wh)
        } else {
            match location.as_str() {
                // Down and to the right of the card window's corner.
                "document" => Rect::new(20, 20, 20 + ww, 20 + wh),
                // The screen's middle: in RandomCard, the card's.
                "" | "card" | "largest" | "deepest" | "main" => {
                    let (l, t) = ((cw - ww) / 2, (ch - wh + style.title_bar()) / 2);
                    Rect::new(l, t, l + ww, t + wh)
                }
                other => {
                    drop(w);
                    self.result = format!("Movie windows can't go at \"{other}\"");
                    return Ok(());
                }
            }
        };
        let movie_height = rect.height() - if controller { CONTROLLER } else { 0 };
        let id = w.next_movie_id;
        w.next_movie_id += 1;
        // A movie on the web is named by the last part of its address.
        let name = match &url {
            Some(u) => u.trim_end_matches('/').rsplit('/').next().unwrap_or(u).split(['?', '#']).next().unwrap_or("").to_string(),
            None => path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        };
        let duration = player.duration();
        let floating = match arg(4).to_ascii_lowercase().as_str() {
            "floating" => true,
            "nonfloating" => false,
            _ => style.floats(),
        };
        w.movies.push(MovieWindow {
            id,
            name: name.clone(),
            path,
            style,
            rect,
            movie_rect: Rect::new(0, 0, rect.width(), movie_height.max(0)),
            visible: !arg(3).eq_ignore_ascii_case("invisible"),
            floating,
            controller,
            badge: true,
            pass_click: true,
            looping: false,
            palindrome: false,
            close_on_finish: false,
            start: 0.0,
            end: duration,
            movie_time: false,
            volume: 256,
            rate: 0.0,
            other: HashMap::new(),
            player,
        });
        drop(w);
        self.post_movie_message("openMovie", vec![name, id.to_string()]);
        Ok(())
    }

    /// What `movie 1`, `movie "Intro"`, `movie id 1001` and `last movie`
    /// come to: `movie id N`, which the movie commands and properties take.
    pub(super) fn movie_reference(&mut self, how: &str, which: &str) -> R<String> {
        let w = self.w();
        let count = w.movies.len();
        let index = match how {
            "id" => which.trim().parse::<u32>().ok().and_then(|id| w.movies.iter().position(|m| m.id == id)),
            "ord" => {
                let n = match crate::hypertalk::parser::ordinal_of(which) {
                    Some(Ordinal::Nth(n)) => n as usize,
                    Some(Ordinal::Last) => count,
                    Some(Ordinal::Middle) => count / 2 + 1,
                    Some(Ordinal::Any) if count > 0 => rand::random_range(1..=count),
                    _ => 0,
                };
                n.checked_sub(1).filter(|i| *i < count)
            }
            _ => match which.trim().parse::<f64>() {
                Ok(n) if n.fract() == 0.0 => (n as usize).checked_sub(1).filter(|i| *i < count),
                _ => w.find_movie(which).map(|(i, _)| i),
            },
        };
        match index {
            Some(i) => Ok(format!("movie id {}", w.movies[i].id)),
            None => {
                let what = if how == "id" { format!("id {which}") } else { which.to_string() };
                drop(w);
                err(format!("No such movie {what}"))
            }
        }
    }

    /// A movie window's property. `as_window`: it was named as a window,
    /// so its rect is the window's on the card; as a movie, its rect is
    /// the movie's in the window.
    pub(super) fn movie_property(&mut self, i: usize, as_window: bool, prop: &str) -> R<String> {
        let prop = prop.trim().to_ascii_lowercase();
        let w = self.w();
        let m = &w.movies[i];
        let rect = if as_window { m.rect } else { m.movie_rect };
        if let Some(v) = rect_property(rect, &prop) {
            return Ok(v);
        }
        let (nw, nh) = m.natural_size();
        Ok(match prop.as_str() {
            "name" if as_window => m.name.clone(),
            "name" | "movie" => super::paths::shown(&m.path),
            "id" => m.id.to_string(),
            "number" => (i + 1).to_string(),
            "owner" => if as_window { "Movie".into() } else { format!("Window ID {}", m.id) },
            "visible" => bool_str(m.visible),
            "controllervisible" => bool_str(m.controller),
            "badge" => bool_str(m.badge),
            "passmovieclick" => bool_str(m.pass_click),
            "loop" => bool_str(m.looping),
            "loopispalindrome" => bool_str(m.palindrome),
            "closeonfinish" => bool_str(m.close_on_finish),
            "mute" | "controllermute" => bool_str(m.volume < 0),
            "soundvolume" | "audiolevel" => m.volume.to_string(),
            "rate" => format_number(m.rate, "0.####"),
            "preferredrate" => "1".into(),
            "preferredsoundvolume" => "256".into(),
            "currtime" => m.time_text(m.time()),
            "duration" => m.time_text(m.duration()),
            "starttime" => m.time_text(m.start),
            "endtime" => m.time_text(m.end),
            "timeformat" => if m.movie_time { "MovieTime" } else { "SystemTime" }.into(),
            "timescale" => format!("{TIME_SCALE}"),
            "defaultrect" => rect_text(Rect::new(0, 0, nw, nh)),
            "scale" | "moviescale" => {
                let s = |a: i32, b: i32| format_number(if b > 0 { a as f64 / b as f64 } else { 1.0 }, "0.0###");
                format!("{},{}", s(m.movie_rect.width(), nw), s(m.movie_rect.height(), nh))
            }
            // QuickTime's transform: here only ever a scale.
            "matrix" => {
                let s = |a: i32, b: i32| format_number(if b > 0 { a as f64 / b as f64 } else { 1.0 }, "0.0###");
                format!("{},0,0\n0,{},0\n0,0,1", s(m.movie_rect.width(), nw), s(m.movie_rect.height(), nh))
            }
            "layer" => if m.floating { "floating" } else { "nonFloating" }.into(),
            "windowstyle" | "style" => m.style.name().into(),
            "isvrmovie" => "false".into(),
            "type" => if as_window { "Movie" } else { "movie" }.into(),
            other => match m.other.get(other) {
                Some(v) => v.clone(),
                None => match KEPT.iter().find(|(k, _)| *k == other) {
                    Some((_, v)) => v.to_string(),
                    None => {
                        let other = other.to_string();
                        drop(w);
                        return err(format!("Movies have no property \"{other}\""));
                    }
                },
            },
        })
    }

    pub(super) fn set_movie_property(&mut self, i: usize, as_window: bool, prop: &str, value: &str) -> R<()> {
        let prop = prop.trim().to_ascii_lowercase();
        let value = value.trim();
        let flag = || to_bool(value);
        let mut rate_changed = None;
        {
            let mut w = self.w();
            let m = &mut w.movies[i];
            let rect = if as_window { m.rect } else { m.movie_rect };
            if let Some(r) = changed_rect(rect, &prop, value) {
                if as_window {
                    m.rect = r;
                } else {
                    m.movie_rect = r;
                }
                return Ok(());
            }
            if rect_property(rect, &prop).is_some() {
                return err(format!("Expected a point or rect here, not \"{value}\""));
            }
            match prop.as_str() {
                "name" if as_window => m.name = value.to_string(),
                "visible" => m.visible = flag()?,
                "controllervisible" => m.show_controller(flag()?),
                "badge" => m.badge = flag()?,
                "passmovieclick" => m.pass_click = flag()?,
                "loop" => m.looping = flag()?,
                "loopispalindrome" => m.palindrome = flag()?,
                "closeonfinish" => m.close_on_finish = flag()?,
                "mute" => {
                    let level = m.volume.abs();
                    m.volume = if flag()? { -level } else { level };
                    m.apply_volume();
                }
                "soundvolume" | "audiolevel" => {
                    m.volume = to_number(value)?.round() as i32;
                    m.apply_volume();
                }
                "rate" => {
                    m.set_rate(to_number(value)?);
                    rate_changed = Some((m.name.clone(), m.id, m.rate));
                }
                "currtime" => {
                    let t = m.parse_time(value)?;
                    m.go_to(t);
                }
                "starttime" => {
                    let t = m.parse_time(value)?;
                    m.start = t.min(m.end);
                }
                "endtime" => {
                    let t = m.parse_time(value)?;
                    m.end = t.clamp(m.start, m.duration());
                }
                "timeformat" => {
                    m.movie_time = match value.to_ascii_lowercase().as_str() {
                        "movietime" => true,
                        "systemtime" => false,
                        _ => return err(format!("Expected SystemTime or MovieTime here, not \"{value}\"")),
                    }
                }
                "layer" => m.floating = !value.eq_ignore_ascii_case("nonfloating"),
                // The movie's size in its window, as factors of its own:
                // "2,2", "double", "half,1". The window keeps its size
                // until sent resizeToMovie.
                "scale" | "moviescale" | "matrix" => {
                    let (nw, nh) = m.natural_size();
                    let screen = w.screen_rect;
                    let m = &mut w.movies[i];
                    let factor = |v: &str| -> Option<f64> {
                        match v.trim().to_ascii_lowercase().as_str() {
                            "half" => Some(0.5),
                            "normal" => Some(1.0),
                            "double" => Some(2.0),
                            "fullscreen" => {
                                let (sw, sh) = (screen.width() as f64 / nw.max(1) as f64, screen.height() as f64 / nh.max(1) as f64);
                                Some(sw.min(sh))
                            }
                            n => n.parse::<f64>().ok().filter(|f| *f > 0.0),
                        }
                    };
                    let (sx, sy) = if prop == "matrix" {
                        if value.eq_ignore_ascii_case("identity") {
                            (1.0, 1.0)
                        } else {
                            let n: Vec<f64> = value.split([',', '\n']).filter_map(|v| v.trim().parse().ok()).collect();
                            match (n.first(), n.get(4)) {
                                (Some(&a), Some(&d)) if a > 0.0 && d > 0.0 => (a, d),
                                _ => return err(format!("Expected a matrix here, not \"{value}\"")),
                            }
                        }
                    } else {
                        let mut parts = value.split(',');
                        let x = parts.next().and_then(factor);
                        let y = match parts.next() {
                            Some(v) => factor(v),
                            None => x,
                        };
                        match (x, y) {
                            (Some(x), Some(y)) => (x, y),
                            _ => return err(format!("Expected a scale here, not \"{value}\"")),
                        }
                    };
                    let r = m.movie_rect;
                    let (w2, h2) = ((nw as f64 * sx).round() as i32, (nh as f64 * sy).round() as i32);
                    m.movie_rect = Rect::new(r.left, r.top, r.left + w2.max(1), r.top + h2.max(1));
                }
                // A new movie in the window, which keeps its rect.
                "movie" if as_window => {
                    drop(w);
                    let Some(path) = self.find_movie_file(value) else {
                        return err(format!("Can't find the movie \"{value}\""));
                    };
                    let player = Player::open(&path).or_else(err)?;
                    let mut w = self.w();
                    let m = &mut w.movies[i];
                    let (mw, mh) = player.size();
                    m.end = player.duration();
                    m.start = 0.0;
                    m.rate = 0.0;
                    m.path = path;
                    m.player = Arc::new(player);
                    m.movie_rect = Rect::new(0, 0, mw as i32, mh as i32);
                    m.apply_volume();
                    let (name, id) = (m.name.clone(), m.id);
                    drop(w);
                    self.post_movie_message("openMovie", vec![name, id.to_string()]);
                    return Ok(());
                }
                other if KEPT.iter().any(|(k, _)| *k == other) => {
                    m.other.insert(other.to_string(), value.to_string());
                }
                other => return err(format!("Can't set the {other} of a movie")),
            }
        }
        if let Some((name, id, rate)) = rate_changed {
            self.post_movie_message("movieRateChanged", vec![name, id.to_string(), format_number(rate, "0.####")]);
        }
        Ok(())
    }

    /// A message sent to a movie window: `play movie 1`, `send "rewind" to
    /// movie "Intro"`, `send "close" to window "Intro"`.
    pub(super) fn movie_message(&mut self, i: usize, message: &str) -> R<()> {
        let message = message.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        let changed = {
            let mut w = self.w();
            let m = &mut w.movies[i];
            let before = m.rate;
            match message.as_str() {
                "play" => m.play(),
                "pause" | "stop" => m.pause(),
                "rewind" => {
                    let start = m.start;
                    m.go_to(start);
                }
                "reverse" => {
                    let rate = if m.rate == 0.0 { -1.0 } else { -m.rate };
                    m.set_rate(rate);
                }
                "stepfwd" | "steprev" => {
                    // A frame at 30 a second: movies don't say their own.
                    let step = if message == "stepfwd" { 1.0 / 30.0 } else { -1.0 / 30.0 };
                    let t = (m.time() + step).clamp(m.start, m.end);
                    m.go_to(t);
                }
                "showposter" => m.go_to(0.0),
                "resizetomovie" => {
                    let r = m.movie_rect;
                    m.movie_rect = Rect::new(0, 0, r.width(), r.height());
                    let extra = if m.controller { CONTROLLER } else { 0 };
                    m.rect = Rect::new(m.rect.left, m.rect.top, m.rect.left + r.width(), m.rect.top + r.height() + extra);
                }
                "select" => {
                    m.visible = true;
                    let m = w.movies.remove(i);
                    w.movies.push(m);
                    return Ok(());
                }
                "close" => {
                    let id = m.id;
                    drop(w);
                    return self.close_movie(id);
                }
                // QuickTime's own: idle, preroll, rotate, skew, exportMovie ...
                _ => {}
            }
            (m.rate != before).then(|| (m.name.clone(), m.id, m.rate))
        };
        if let Some((name, id, rate)) = changed {
            self.post_movie_message("movieRateChanged", vec![name, id.to_string(), format_number(rate, "0.####")]);
        }
        Ok(())
    }

    /// A movie file dropped on the card window, opened as HyperCard 2.4
    /// opened one: a plain window near the card's corner. If it can't be
    /// played, an alert says why.
    pub(super) fn open_dropped_movie(&mut self, path: &std::path::Path) -> R<()> {
        let args = [super::paths::shown(path), "plain".into(), "document".into(), "visible".into(), "nonFloating".into()];
        self.open_movie(&args)?;
        if !self.result.is_empty() {
            let prompt = format!("Can't play \"{}\": {}", path.file_name().unwrap_or_default().to_string_lossy(), self.result);
            self.dialog(super::world::Dialog::Answer { prompt, buttons: vec!["OK".into()] })?;
        }
        Ok(())
    }

    /// Close a movie window, telling the card first with closeMovie.
    pub fn close_movie(&mut self, id: u32) -> R<()> {
        let Some(name) = self.w().movies.iter().find(|m| m.id == id).map(|m| m.name.clone()) else {
            return Ok(());
        };
        let card = self.current_card_ref();
        let r = self.send_message(card, "closeMovie", &[name, id.to_string()]);
        self.w().movies.retain(|m| m.id != id);
        r.map(|_| ())
    }

    /// movieInfo(file): a movie file's rect, volume, rate, duration (as
    /// SystemTime, then MovieTime), time scale, and preview and poster
    /// times, a line each. `movieInfo(".")` is the version of the movie
    /// player, QuickTime's in HyperCard.
    pub(super) fn movie_info(&mut self, file: &str) -> String {
        if file.trim() == "." {
            return "3.0".into();
        }
        let Some(path) = self.find_movie_file(file) else { return String::new() };
        let Ok(p) = Player::open(&path) else { return String::new() };
        let (w, h) = p.size();
        let d = p.duration();
        let units = |s: f64| format!("{}", (s * TIME_SCALE).round() as i64);
        [rect_text(Rect::new(0, 0, w as i32, h as i32)), "256".into(), "1".into(), system_time(d), units(d), format!("{TIME_SCALE}"), "0".into(), "0".into(), "0".into(), "0".into(), "0".into(), "0".into()].join("\n")
    }
}

/// An address on the web, http or https: a movie that plays from there.
pub fn is_url(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    s.starts_with("http://") || s.starts_with("https://")
}

#[cfg(test)]
mod tests {
    #[test]
    fn web_addresses_are_movies_on_the_web() {
        assert!(super::is_url("http://example.com/videos/782") && super::is_url(" HTTPS://x/y.mp4"));
        assert!(!super::is_url("Intro.mov") && !super::is_url("ftp://x/y"));
    }

    use super::*;

    #[test]
    fn system_times() {
        assert_eq!(system_time(0.0), "0");
        assert_eq!(system_time(2.5), "2,30");
        assert_eq!(system_time(62.0), "1,2,0");
        assert_eq!(system_time(3600.0 + 0.25), "1,0,0,15");
        assert_eq!(parse_system_time("2,30"), Some(2.5));
        assert_eq!(parse_system_time("1,2,0"), Some(62.0));
        assert_eq!(parse_system_time("6,13.9").map(|s| (s * 1000.0).round()), Some(6232.0));
        assert_eq!(parse_system_time("x"), None);
    }
}
