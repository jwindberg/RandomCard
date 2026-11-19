//! The `picture` command, HyperCard's Picture XCMD (built into HyperCard
//! 2): a picture from a file, from the stack's pictures, or from the
//! clipboard, in a window of its own.
//!
//! `picture name [, file | resource | clipboard [, style [, visible [,
//! depth [, floatingLayer]]]]]`. The window takes the picture's name and
//! goes in the middle of the card; the card gets openPicture name, id, and
//! closePicture when it closes. Like the XCMD, a problem goes in the result,
//! starting "Couldn't display picture."

use std::sync::Arc;

use super::externals::FileDialog;
use super::interp::{Interp, R};
use super::world::{PictureStyle, ScriptWindow, WindowKind};
use crate::model::{Picture, Rect};

impl Interp {
    pub(super) fn picture_command(&mut self, args: &[String]) -> R<()> {
        let arg = |i: usize| args.get(i).map(|a| a.trim().to_string()).unwrap_or_default();
        let fail = |i: &mut Interp, why: &str| {
            i.result = format!("Couldn't display picture. {why}");
            Ok(())
        };
        let Some(style) = PictureStyle::named(&arg(2)) else {
            return fail(self, &format!("There's no window style \"{}\".", arg(2)));
        };
        let visible = !arg(3).eq_ignore_ascii_case("false");
        let source = arg(1).to_ascii_lowercase();
        let (name, picture) = match source.as_str() {
            "" | "file" => {
                // With no name, the user chooses the file.
                let path = if arg(0).is_empty() {
                    match self.file_dialog("Choose a picture to show:", FileDialog::Open) {
                        Some(p) => p,
                        None => {
                            self.result = "cancel".into();
                            return Ok(());
                        }
                    }
                } else {
                    match self.find_beside_stack(&arg(0)) {
                        Some(p) => p,
                        None => return fail(self, &format!("There's no file \"{}\".", arg(0))),
                    }
                };
                let picture = match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|d| crate::paint::image_file::decode(&d)) {
                    Ok(p) => p,
                    Err(e) => return fail(self, &e),
                };
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| arg(0));
                (name, picture)
            }
            "resource" => {
                let w = self.w();
                let wanted = arg(0);
                let found = match wanted.parse::<i32>() {
                    Ok(id) => w.stack.pictures.get(&id),
                    Err(_) => w.stack.pictures.values().find(|p| p.name.eq_ignore_ascii_case(&wanted)),
                };
                let found = found.map(|p| (if p.name.is_empty() { wanted.clone() } else { p.name.clone() }, p.picture.clone()));
                // The pictures kept for AddColor and the built-in externals,
                // by name, as a last resort.
                let found = found.or_else(|| {
                    let key = crate::model::color_picture_key(&wanted, false);
                    w.stack.color_pictures.get(&key).map(|p| (wanted.clone(), (**p).clone()))
                });
                drop(w);
                match found {
                    Some(f) => f,
                    None => return fail(self, &format!("The stack has no picture \"{wanted}\".")),
                }
            }
            "clipboard" => match clipboard_picture() {
                Some(p) => (arg(0), p),
                None => return fail(self, "There's no picture on the clipboard."),
            },
            _ => return fail(self, &format!("\"{}\" isn't file, resource or clipboard.", arg(1))),
        };
        let id = {
            let mut w = self.w();
            // One window to a name: showing it again replaces it.
            w.script_windows.retain(|t| !t.name.eq_ignore_ascii_case(&name));
            let id = 1 + w
                .script_windows
                .iter()
                .filter_map(|t| match t.kind {
                    WindowKind::Picture { id, .. } => Some(id),
                    _ => None,
                })
                .max()
                .unwrap_or(1000);
            // In the middle of the card, at its own size, but no bigger
            // than the screen.
            let screen = w.screen_rect;
            let (pw, ph) = (picture.width as i32, picture.height as i32);
            let (ww, wh) = (pw.min(screen.width() - 40).max(4), ph.min(screen.height() - 80).max(4));
            let (cw, ch) = (w.stack.width as i32, w.stack.height as i32);
            let (x, y) = ((cw - ww) / 2, (ch - wh) / 2);
            w.script_windows.push(ScriptWindow {
                name: name.clone(),
                kind: WindowKind::Picture { id, style, view: Default::default() },
                text: String::new(),
                rect: Rect::new(x, y, x + ww, y + wh),
                font: String::new(),
                size: 12,
                visible,
                dirty: false,
                locked: false,
                palette: None,
                apple: None,
                image: Some(Arc::new(picture)),
            });
            id
        };
        self.result = String::new();
        let card = self.current_card_ref();
        self.send_message(card, "openPicture", &[name, id.to_string()]).map(|_| ())
    }

    /// The name of picture window `which`: a number counts the picture
    /// windows, front first; anything else is a window's name.
    pub(super) fn picture_window_name(&self, which: &str) -> String {
        let w = self.w();
        let pictures: Vec<&ScriptWindow> =
            w.script_windows.iter().rev().filter(|t| matches!(t.kind, WindowKind::Picture { .. })).collect();
        match which.trim().parse::<usize>() {
            Ok(n) if n >= 1 => pictures.get(n - 1).map(|t| t.name.clone()).unwrap_or_default(),
            _ => which.trim().to_string(),
        }
    }

    /// `the number of pictures`: the picture windows open.
    pub(super) fn picture_count(&self) -> usize {
        self.w().script_windows.iter().filter(|t| matches!(t.kind, WindowKind::Picture { .. })).count()
    }
}

/// An image on the system clipboard.
fn clipboard_picture() -> Option<Picture> {
    let img = arboard::Clipboard::new().ok()?.get_image().ok()?;
    let (w, h) = (img.width as u32, img.height as u32);
    let mut pic = Picture::new(w, h);
    for (i, px) in img.bytes.as_chunks::<4>().0.iter().enumerate() {
        let (x, y) = ((i as u32 % w.max(1)) as i32, (i as u32 / w.max(1)) as i32);
        pic.set(x, y, crate::model::Pixel::rgba(px[0], px[1], px[2], px[3]));
    }
    Some(pic)
}

/// The words the Picture XCMD took in place of a point or rectangle: a
/// screen to center the window on. RandomCard knows the screen the card
/// window is on, so they all mean that one.
fn screen_word(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "cardscreen" | "card" | "largestscreen" | "largest" | "deepestscreen" | "deepest" | "mainscreen" | "main"
    )
}

impl Interp {
    /// A picture window's own property (`the scale of window "Klee"`), or
    /// None if the window isn't a picture window or that isn't one of them.
    pub(super) fn picture_property(&self, window: &str, prop: &str) -> Option<String> {
        let w = self.w();
        let t = w.script_windows.iter().find(|t| t.name.eq_ignore_ascii_case(window))?;
        let WindowKind::Picture { view, .. } = t.kind else { return None };
        let (ox, oy) = w.window_origin;
        let r = t.rect;
        Some(match prop.to_ascii_lowercase().as_str() {
            "globalloc" => format!("{},{}", r.left + ox, r.top + oy),
            "globalrect" => format!("{},{},{},{}", r.left + ox, r.top + oy, r.right + ox, r.bottom + oy),
            "scroll" => format!("{},{}", view.scroll.0, view.scroll.1),
            "scale" => view.scale.to_string(),
            "zoom" => if view.unzoomed.is_some() { "out" } else { "in" }.into(),
            "dithering" => view.dithering.to_string(),
            _ => return None,
        })
    }

    /// Set a picture window's own property, or one that takes the screen
    /// words; false if that's left to windows in general.
    pub(super) fn set_picture_property(&mut self, window: &str, prop: &str, value: &str) -> R<bool> {
        let prop = prop.to_ascii_lowercase();
        let mut w = self.w();
        let (ox, oy) = w.window_origin;
        let screen = w.screen_rect;
        let Some(t) = w.script_windows.iter_mut().find(|t| t.name.eq_ignore_ascii_case(window)) else { return Ok(false) };
        let WindowKind::Picture { id, style, mut view } = t.kind else { return Ok(false) };
        let Some(image) = t.image.clone() else { return Ok(false) };
        let bad = |what: &str| super::interp::err(format!("Expected {what}, not \"{value}\""));
        let r = t.rect;
        match prop.as_str() {
            "loc" | "location" | "topleft" | "globalloc" | "rect" | "rectangle" | "globalrect" if screen_word(value) => {
                // Centered on the screen; a rect also as big as it allows.
                let (ww, wh) = if prop.contains("rect") { fit(&image, view, screen) } else { (r.width(), r.height()) };
                let (x, y) = (screen.left + (screen.width() - ww) / 2 - ox, screen.top + (screen.height() - wh) / 2 - oy);
                t.rect = Rect::new(x, y, x + ww, y + wh);
            }
            "globalloc" => {
                let Some((x, y)) = crate::hypertalk::value::parse_point(value) else { return bad("a point") };
                t.rect = r.translate(x - ox - r.left, y - oy - r.top);
            }
            "globalrect" => {
                let Some((l, tp, rt, b)) = crate::hypertalk::value::parse_rect(value) else { return bad("a rectangle") };
                t.rect = Rect::new(l - ox, tp - oy, rt - ox, b - oy);
            }
            "scroll" => {
                let Some(p) = crate::hypertalk::value::parse_point(value) else { return bad("a point") };
                view.scroll = p;
            }
            "scale" => {
                let Ok(n) = value.trim().parse::<f64>() else { return bad("a number") };
                view.scale = (n as i32).clamp(-5, 5);
            }
            "zoom" => match value.trim().to_ascii_lowercase().as_str() {
                "out" if view.unzoomed.is_none() => {
                    view.unzoomed = Some(r);
                    let (ww, wh) = fit(&image, view, screen);
                    t.rect = Rect::new(r.left, r.top, r.left + ww, r.top + wh);
                }
                "in" => {
                    if let Some(old) = view.unzoomed.take() {
                        t.rect = old;
                    }
                }
                "out" => {}
                _ => return bad("in or out"),
            },
            "dithering" => view.dithering = value.trim().eq_ignore_ascii_case("true"),
            _ => return Ok(false),
        }
        view.scroll = clamp_scroll(&image, view, t.rect);
        t.kind = WindowKind::Picture { id, style, view };
        Ok(true)
    }

    /// The zoom box: out to show the whole picture, and back in.
    pub(super) fn toggle_picture_zoom(&mut self, window: &str) -> R<()> {
        let out = self.picture_property(window, "zoom").as_deref() == Some("in");
        self.set_picture_property(window, "zoom", if out { "out" } else { "in" }).map(|_| ())
    }

    /// A click in a picture window: mouseDownInPicture or mouseUpInPicture
    /// to the card, with the window's name and the point in its picture.
    pub(super) fn picture_mouse(&mut self, window: &str, down: bool, (x, y): (i32, i32)) -> R<()> {
        let card = self.current_card_ref();
        let message = if down { "mouseDownInPicture" } else { "mouseUpInPicture" };
        self.dispatch_top(card, message, vec![window.to_string(), format!("{x},{y}")])
    }
}

/// The biggest a picture window can be for its picture at its scale, on
/// this screen.
fn fit(image: &Picture, view: super::world::PictureView, screen: Rect) -> (i32, i32) {
    let f = view.factor();
    let (pw, ph) = ((image.width as f32 * f).ceil() as i32, (image.height as f32 * f).ceil() as i32);
    (pw.min(screen.width() - 40).max(4), ph.min(screen.height() - 80).max(4))
}

/// A scroll that keeps the window on the picture.
fn clamp_scroll(image: &Picture, view: super::world::PictureView, rect: Rect) -> (i32, i32) {
    let f = view.factor();
    let max_x = (image.width as f32 - rect.width() as f32 / f).max(0.0) as i32;
    let max_y = (image.height as f32 - rect.height() as f32 / f).max(0.0) as i32;
    (view.scroll.0.clamp(0, max_x), view.scroll.1.clamp(0, max_y))
}
