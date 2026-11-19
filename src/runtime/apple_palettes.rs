//! GeneralPalette, FieldPalette and StackTemplatePal: palettes Apple's own
//! stacks made with XCMDs of their own (Jay Hodgdon's, 1990), built in.
//!
//! Each draws a picture the stack carries (a PICT named like the XCMD) in a
//! palette window, puts the names it's given on its buttons, and keeps two
//! globals: Have<window> (true while it's there, false once dismissed) and
//! <window>Loc (where it was put, in screen coordinates). Scripts then treat
//! it as a window: hide, show, close, `set the loc`, and `send "move"`,
//! which puts it back at <window>Loc. Closing it sends paletteDismiss.

use std::sync::Arc;

use super::interp::{Interp, R};
use super::world::{ScriptWindow, WindowKind};
use crate::model::{Palette, PaletteButton, Picture, Rect};
use crate::paint::{PaintText, draw_text};

/// A box in a palette's picture: left, top, right, bottom.
type Box4 = (i32, i32, i32, i32);

/// Which of the three palettes: their pictures differ, and so do their
/// buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layout {
    /// Left and right arrows with a named button between them.
    General,
    /// The same, under three lines of text the script sets (`the fields`).
    Field,
    /// Info and Home buttons beside a named one, over a second named one
    /// and the arrows.
    StackTemplate,
}

impl Layout {
    fn named(name: &str) -> Option<Layout> {
        match name {
            "generalpalette" => Some(Layout::General),
            "fieldpalette" => Some(Layout::Field),
            "stacktemplatepal" => Some(Layout::StackTemplate),
            _ => None,
        }
    }

    /// The window's name, and the PICT's.
    fn window(self) -> &'static str {
        match self {
            Layout::General => "GeneralPalette",
            Layout::Field => "FieldPalette",
            Layout::StackTemplate => "StackTemplatePal",
        }
    }

    /// The PICT's resource id, which the window takes as its id.
    fn id(self) -> i32 {
        match self {
            Layout::General => 4001,
            Layout::Field => 4000,
            Layout::StackTemplate => 4002,
        }
    }

    /// Each button's rectangle in the picture (its frame and shadow) and
    /// the message it sends.
    fn buttons(self) -> &'static [(Box4, &'static str)] {
        match self {
            Layout::General => &[((5, 4, 35, 30), "goPrev"), ((45, 4, 165, 30), "goCategories"), ((177, 4, 207, 30), "goNext")],
            Layout::Field => &[((5, 59, 35, 85), "goPrev"), ((45, 59, 165, 85), "goCategories"), ((177, 59, 207, 85), "goNext")],
            Layout::StackTemplate => &[
                ((4, 4, 33, 33), "showInfo"),
                ((37, 4, 152, 33), "createStack"),
                ((156, 4, 185, 33), "goHome"),
                ((4, 36, 119, 65), "goCategories"),
                ((123, 36, 152, 65), "goPrev"),
                ((156, 36, 185, 65), "goNext"),
            ],
        }
    }

    /// Where the given names go, inside the buttons' frames.
    fn labels(self) -> &'static [Box4] {
        match self {
            Layout::General => &[(46, 5, 163, 28)],
            Layout::Field => &[(46, 60, 163, 83)],
            // The bottom button's name sits right of its arrow.
            Layout::StackTemplate => &[(38, 5, 150, 31), (30, 37, 117, 63)],
        }
    }

    /// How many names the XCMD takes before its left and top.
    fn name_count(self) -> usize {
        match self {
            Layout::StackTemplate => 2,
            _ => 1,
        }
    }
}

/// What a built-in palette window was made from, to draw it again when its
/// fields change.
#[derive(Clone, Debug)]
pub struct ApplePalette {
    pub layout: Layout,
    pub picture: Arc<Picture>,
    pub names: Vec<String>,
    /// GeneralPalette's optional title, above its buttons.
    pub title: Option<String>,
    /// FieldPalette's three lines.
    pub fields: String,
}

/// The height of GeneralPalette's title strip, when it has a title.
const TITLE: i32 = 20;

impl ApplePalette {
    /// The palette to show: the picture with the names and fields drawn
    /// on it, and its buttons.
    pub fn render(&self) -> Palette {
        let src = &self.picture;
        let dy = if self.title.is_some() { TITLE } else { 0 };
        let mut pic = Picture::new(src.width, src.height + dy as u32);
        for y in 0..pic.height as i32 {
            for x in 0..pic.width as i32 {
                let p = if y < dy { crate::model::Pixel::WHITE } else { src.get(x, y - dy) };
                pic.set(x, y, if p.is_clear() { crate::model::Pixel::WHITE } else { p });
            }
        }
        let shift = |(l, t, r, b): Box4| (l, t + dy, r, b + dy);
        if let Some(title) = &self.title {
            let w = pic.width as i32;
            label(&mut pic, (0, 2, w, TITLE), title, 12, Align::Center);
        }
        for (name, &rect) in self.names.iter().zip(self.layout.labels()) {
            label(&mut pic, shift(rect), name, 14, Align::Center);
        }
        if self.layout == Layout::Field {
            // The first line across the top, centered; the second and
            // third under it, at the left and the right.
            let mut lines = self.fields.lines();
            let (one, two, three) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""), lines.next().unwrap_or(""));
            label(&mut pic, (9, 8, 203, 26), one, 12, Align::Center);
            label(&mut pic, (11, 28, 203, 46), two, 12, Align::Left);
            label(&mut pic, (11, 28, 201, 46), three, 12, Align::Right);
        }
        let buttons = self
            .layout
            .buttons()
            .iter()
            .map(|&(rect, message)| {
                let (l, t, r, b) = shift(rect);
                PaletteButton { rect: Rect::new(l, t, r, b), message: message.into() }
            })
            .collect();
        Palette { name: self.layout.window().into(), picture: pic, buttons }
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}

/// Text in a box, centered top to bottom and cut off at its edges.
fn label(pic: &mut Picture, (l, t, r, b): Box4, text: &str, size: u32, align: Align) {
    let text = text.trim();
    if text.is_empty() || r <= l || b <= t {
        return;
    }
    let how = PaintText { size, font: "Palatino", style: "plain", color: crate::model::Pixel::BLACK };
    let width = {
        let mut scratch = Picture::new(1, 1);
        draw_text(&mut scratch, 0, 0, text, &how).0
    };
    let (w, h) = (r - l, b - t);
    let line = (size as f32 * 1.2).round() as i32;
    let x = match align {
        Align::Left => 0,
        Align::Center => (w - width) / 2,
        Align::Right => w - width,
    };
    let mut boxed = Picture::new(w as u32, h as u32);
    draw_text(&mut boxed, x.max(0), (h - line) / 2, text, &how);
    for y in 0..h {
        for x in 0..w {
            let p = boxed.get(x, y);
            if !p.is_clear() {
                pic.set(l + x, t + y, p);
            }
        }
    }
}

impl Interp {
    /// GeneralPalette name, left, top (or name, title, left, top),
    /// FieldPalette name, left, top, StackTemplatePal top name, bottom
    /// name, left, top: open the palette with its top left at that point on
    /// the screen. Like the XCMDs, a problem goes in the result.
    pub(super) fn apple_palette(&mut self, name: &str, args: &[String]) -> Option<R<()>> {
        let layout = Layout::named(name)?;
        let window = layout.window();
        let names = layout.name_count();
        let title_form = layout == Layout::General && args.len() == 4;
        if args.len() != names + 2 && !title_form {
            self.result = "Error:  Incorrect number of parameters.".into();
            return Some(Ok(()));
        }
        let key = crate::model::color_picture_key(window, false);
        let Some(picture) = self.w().stack.color_pictures.get(&key).cloned() else {
            self.result = format!("Error:  Can't access {window} PICT.");
            return Some(Ok(()));
        };
        if self.w().script_windows.iter().any(|t| t.name.eq_ignore_ascii_case(window)) {
            self.result = format!("Error:  There already is a {window} window.");
            return Some(Ok(()));
        }
        let int = |s: &String| s.trim().parse::<f64>().map(|n| n as i32).unwrap_or(0);
        let (left, top) = (int(&args[args.len() - 2]), int(&args[args.len() - 1]));
        let built = ApplePalette {
            layout,
            picture,
            names: args[..names].to_vec(),
            title: title_form.then(|| args[1].clone()),
            fields: String::new(),
        };
        let palette = Arc::new(built.render());
        let mut w = self.w();
        let (ox, oy) = w.window_origin;
        let (x, y) = (left - ox, top - oy);
        let (pw, ph) = (palette.picture.width as i32, palette.picture.height as i32);
        w.script_windows.push(ScriptWindow {
            name: window.into(),
            kind: WindowKind::Palette { id: layout.id(), hilited: -1 },
            text: String::new(),
            rect: Rect::new(x, y, x + pw, y + ph),
            font: String::new(),
            size: 12,
            visible: true,
            dirty: false,
            locked: false,
            palette: Some(palette),
            apple: Some(Box::new(built)),
            image: None,
        });
        drop(w);
        self.globals.insert(format!("have{}", window.to_lowercase()), "true".into());
        self.globals.insert(format!("{}loc", window.to_lowercase()), format!("{left},{top}"));
        self.result = String::new();
        Some(Ok(()))
    }

    /// A built-in palette closing: Have<window> becomes false and
    /// <window>Loc where it was, on the screen.
    pub(super) fn apple_palette_closed(&mut self, t: &ScriptWindow) {
        let (ox, oy) = self.w().window_origin;
        let name = t.name.to_lowercase();
        self.globals.insert(format!("have{name}"), "false".into());
        self.globals.insert(format!("{name}loc"), format!("{},{}", t.rect.left + ox, t.rect.top + oy));
    }

    /// `send "move" to window "GeneralPalette"`: back to <window>Loc.
    pub(super) fn apple_palette_move(&mut self, name: &str) {
        let at = self.globals.get(&format!("{}loc", name.to_lowercase())).cloned().unwrap_or_default();
        let Some((x, y)) = crate::hypertalk::value::parse_point(&at) else { return };
        let mut w = self.w();
        let (ox, oy) = w.window_origin;
        if let Some(t) = w.script_windows.iter_mut().find(|t| t.name.eq_ignore_ascii_case(name)) {
            let r = t.rect;
            t.rect = r.translate(x - ox - r.left, y - oy - r.top);
        }
    }
}
