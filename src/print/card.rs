//! Drawing a card onto a PDF page: its pictures as 1-bit images, its
//! buttons and fields as vector shapes, and text placed by HyperCard's
//! metrics, in the card's own coordinates.

use std::fmt::Write;

use super::layout::{self, Line, Setting};
use super::pdf::{Document, text_literal};
use crate::model::metrics;
use crate::model::styles::{self, StyleRun};
use crate::model::{Id, Layer, POPUP_TEXT_INSET, Part, PartKind, PartStyle, Picture, Pixel, Rect, Stack, TextAlign};

/// A page's drawing commands, in a card's coordinates (y down).
pub struct Canvas<'a> {
    pub doc: &'a mut Document,
    pub ops: String,
}

const BLACK: f32 = 0.0;
const WHITE: f32 = 1.0;
const GRAY: f32 = 0.55;

impl Canvas<'_> {
    fn op(&mut self, s: impl AsRef<str>) {
        self.ops.push_str(s.as_ref());
        self.ops.push('\n');
    }

    fn fill_color(&mut self, g: f32) {
        self.op(format!("{g} g"));
    }

    fn rect_path(&mut self, r: Rect, inset: f32) {
        let (x, y) = (r.left as f32 + inset, r.top as f32 + inset);
        let (w, h) = ((r.right - r.left) as f32 - 2.0 * inset, (r.bottom - r.top) as f32 - 2.0 * inset);
        self.op(format!("{x} {y} {w} {h} re"));
    }

    fn fill_rect(&mut self, r: Rect, g: f32) {
        self.fill_color(g);
        self.rect_path(r, 0.0);
        self.op("f");
    }

    /// A 1-pixel frame just inside the rectangle.
    fn frame_rect(&mut self, r: Rect) {
        self.op(format!("{BLACK} G 1 w"));
        self.rect_path(r, 0.5);
        self.op("S");
    }

    fn round_path(&mut self, r: Rect, radius: f32, inset: f32) {
        let (l, t) = (r.left as f32 + inset, r.top as f32 + inset);
        let (rt, b) = (r.right as f32 - inset, r.bottom as f32 - inset);
        let rad = radius.min((rt - l) / 2.0).min((b - t) / 2.0);
        // Bézier circle approximation.
        let k = rad * 0.552;
        self.op(format!(
            "{} {t} m {} {t} l {} {t} {rt} {} {rt} {} c {rt} {} l {rt} {} {} {b} {} {b} c {} {b} l {} {b} {l} {} {l} {} c {l} {} l {l} {} {} {t} {} {t} c h",
            l + rad,
            rt - rad,
            rt - rad + k,
            t + rad - k,
            t + rad,
            b - rad,
            b - rad + k,
            rt - rad + k,
            rt - rad,
            l + rad,
            l + rad - k,
            b - rad + k,
            b - rad,
            t + rad,
            t + rad - k,
            l + rad - k,
            l + rad,
        ));
    }

    fn oval_path(&mut self, r: Rect, inset: f32) {
        let (l, t) = (r.left as f32 + inset, r.top as f32 + inset);
        let (rt, b) = (r.right as f32 - inset, r.bottom as f32 - inset);
        let (cx, cy, rx, ry) = ((l + rt) / 2.0, (t + b) / 2.0, (rt - l) / 2.0, (b - t) / 2.0);
        let (kx, ky) = (rx * 0.552, ry * 0.552);
        self.op(format!(
            "{rt} {cy} m {rt} {} {} {b} {cx} {b} c {} {b} {l} {} {l} {cy} c {l} {} {} {t} {cx} {t} c {} {t} {rt} {} {rt} {cy} c h",
            cy + ky,
            cx + kx,
            cx - kx,
            cy + ky,
            cy - ky,
            cx - kx,
            cx + kx,
            cy - ky,
        ));
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, g: f32) {
        self.op(format!("{g} G 1 w {x1} {y1} m {x2} {y2} l S"));
    }

    fn clip(&mut self, r: Rect) {
        self.rect_path(r, 0.0);
        self.op("W n");
    }

    /// Paint a picture's pixels of one kind, where the picture is placed.
    fn picture_pixels(&mut self, pic: &Picture, kind: Pixel, at: (i32, i32), g: f32) {
        let (w, h) = (pic.width as usize, pic.height as usize);
        let row = w.div_ceil(8);
        let mut bits = vec![0u8; row * h];
        let mut any = false;
        for y in 0..h {
            for x in 0..w {
                if pic.pixels[y * w + x] == kind {
                    bits[y * row + x / 8] |= 0x80 >> (x % 8);
                    any = true;
                }
            }
        }
        if !any {
            return;
        }
        let name = self.doc.image_mask(pic.width, pic.height, &bits);
        self.fill_color(g);
        // Image space has its first row at the top of the unit square.
        self.op(format!("q {w} 0 0 -{h} {} {} cm /{name} Do Q", at.0, at.1 + h as i32));
    }

    fn picture(&mut self, pic: &Picture, at: (i32, i32)) {
        self.picture_pixels(pic, Pixel::White, at, WHITE);
        self.picture_pixels(pic, Pixel::Black, at, BLACK);
    }

    /// Black text placed by `layout`, from the origin.
    pub fn text_lines(&mut self, lines: &[Line]) {
        self.lines(lines, 0.0, 0.0, BLACK);
    }

    /// Text placed by `layout`, with its top-left at (x, y).
    fn lines(&mut self, lines: &[Line], x: f32, y: f32, g: f32) {
        for line in lines {
            for p in &line.pieces {
                let base = pdf_font(&p.font, &p.style);
                let font = self.doc.font(base);
                let (px, py) = (x + p.x, y + line.baseline);
                // Stretch or squeeze the word to the width it had in HyperCard.
                let word = p.text.trim_end();
                let theirs = metrics::metrics(&p.font, p.size, &p.style).text_width(word);
                let ours = pdf_text_width(base, word) * p.size as f32;
                let tz = if ours > 0.0 { (theirs / ours * 100.0).clamp(75.0, 135.0) } else { 100.0 };
                self.op(format!(
                    "BT {g} g /{font} {} Tf {tz:.1} Tz 1 0 0 -1 {px} {py} Tm {} Tj ET",
                    p.size,
                    text_literal(word)
                ));
                if styles::has_style(&p.style, "underline") {
                    let w = metrics::metrics(&p.font, p.size, &p.style).text_width(p.text.trim_end());
                    self.line(px, py + 1.5, px + w, py + 1.5, g);
                }
            }
        }
    }
}

/// Helvetica's character widths (from its AFM metrics), in thousandths of
/// the size, for characters 32 to 126.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556,
    556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334,
    260, 334, 584,
];

/// About how wide text is in a standard font, in units of its size. Courier
/// is exact; the others go by Helvetica, scaled to the family.
fn pdf_text_width(base: &str, text: &str) -> f32 {
    if base.starts_with("Courier") {
        return text.chars().count() as f32 * 0.6;
    }
    let scale = if base.starts_with("Times") {
        0.9
    } else if base.contains("Bold") {
        1.06
    } else {
        1.0
    };
    let units: u32 = text.chars().map(|c| HELVETICA.get((c as usize).wrapping_sub(32)).copied().unwrap_or(556) as u32).sum();
    units as f32 / 1000.0 * scale
}

/// The standard PDF font nearest a Macintosh font and style.
fn pdf_font(font: &str, style: &str) -> &'static str {
    let f = font.to_ascii_lowercase();
    let bold = styles::has_style(style, "bold");
    let italic = styles::has_style(style, "italic");
    let family = if f.contains("courier") || f.contains("monaco") || f.contains("mono") {
        0
    } else if f.contains("times") || f.contains("new york") || f.contains("palatino") || f.contains("serif") {
        1
    } else {
        2
    };
    const NAMES: [[&str; 4]; 3] = [
        ["Courier", "Courier-Bold", "Courier-Oblique", "Courier-BoldOblique"],
        ["Times-Roman", "Times-Bold", "Times-Italic", "Times-BoldItalic"],
        ["Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Helvetica-BoldOblique"],
    ];
    NAMES[family][bold as usize + 2 * italic as usize]
}

/// Draw a card as it looks: background picture and parts, then the card's.
pub fn draw_card(c: &mut Canvas, stack: &Stack, card: Id) {
    let Some(cd) = stack.card(card) else { return };
    let full = Rect::new(0, 0, stack.width as i32, stack.height as i32);
    c.op("q");
    c.clip(full);
    c.fill_rect(full, WHITE);
    if let Some(bg) = stack.background(cd.background) {
        if bg.show_pict {
            c.picture(&bg.picture, (0, 0));
        }
        for p in bg.parts.iter().filter(|p| p.visible) {
            draw_part(c, stack, card, Layer::Background, p);
        }
    }
    if cd.show_pict {
        c.picture(&cd.picture, (0, 0));
    }
    for p in cd.parts.iter().filter(|p| p.visible) {
        draw_part(c, stack, card, Layer::Card, p);
    }
    c.op("Q");
}

fn draw_part(c: &mut Canvas, stack: &Stack, card: Id, layer: Layer, p: &Part) {
    c.op("q");
    match p.kind {
        PartKind::Button => draw_button(c, stack, card, layer, p),
        PartKind::Field => draw_field(c, stack, card, layer, p),
    }
    c.op("Q");
}

fn one_line(align: TextAlign, text: &str, size: u32, font: &str, style: &str, width: f32) -> Vec<Line> {
    let s = Setting { font, size, style, align, wrap: false, line_height: None };
    layout::layout(text, &[], &s, width)
}

fn draw_button(c: &mut Canvas, stack: &Stack, card: Id, layer: Layer, p: &Part) {
    let r = p.rect;
    let hilite = stack.hilite(card, layer, p.id);
    let checkable = matches!(p.style, PartStyle::CheckBox | PartStyle::Radio);
    let has_icon = p.icon != 0 && !checkable;
    let inverted = hilite && !checkable && !(has_icon && matches!(p.style, PartStyle::Transparent | PartStyle::Oval));
    let (fg, bg) = if inverted { (WHITE, BLACK) } else { (BLACK, WHITE) };
    let fg = if p.enabled { fg } else { GRAY };
    let shadow = |c: &mut Canvas, by: i32, radius: f32| {
        c.fill_color(BLACK);
        let s = Rect::new(r.left + by, r.top + by, r.right + by, r.bottom + by);
        if radius > 0.0 { c.round_path(s, radius, 0.0) } else { c.rect_path(s, 0.0) }
        c.op("f");
    };
    match p.style {
        PartStyle::Transparent => {
            if inverted {
                c.fill_rect(r, BLACK);
            }
        }
        PartStyle::Opaque => c.fill_rect(r, bg),
        PartStyle::Rectangle => {
            c.fill_rect(r, bg);
            c.frame_rect(r);
        }
        PartStyle::Shadow => {
            shadow(c, 2, 0.0);
            c.fill_rect(r, bg);
            c.frame_rect(r);
        }
        PartStyle::RoundRect => {
            shadow(c, 1, 8.0);
            c.fill_color(bg);
            c.op(format!("{BLACK} G 1 w"));
            c.round_path(r, 8.0, 0.5);
            c.op("B");
        }
        PartStyle::Standard => {
            c.fill_color(bg);
            c.op(format!("{BLACK} G 1 w"));
            c.round_path(r, 8.0, 0.5);
            c.op("B");
        }
        PartStyle::Popup => {
            draw_popup(c, p, fg, bg);
            return;
        }
        PartStyle::Default => {
            c.op(format!("{BLACK} G 3 w"));
            c.round_path(r, 10.0, 1.5);
            c.op("S");
            let inner = Rect::new(r.left + 4, r.top + 4, r.right - 4, r.bottom - 4);
            c.fill_color(bg);
            c.op(format!("{BLACK} G 1 w"));
            c.round_path(inner, 6.0, 0.5);
            c.op("B");
        }
        PartStyle::Oval => {
            c.fill_color(bg);
            c.op(format!("{BLACK} G 1 w"));
            c.oval_path(r, 0.5);
            c.op("B");
        }
        PartStyle::CheckBox | PartStyle::Radio | PartStyle::Scrolling => {}
    }
    let label = p.name.clone();
    let cy = (r.top + r.bottom) as f32 / 2.0;
    if checkable {
        let b = Rect::new(r.left + 4, (cy - 6.0) as i32, r.left + 16, (cy + 6.0) as i32);
        if p.style == PartStyle::CheckBox {
            c.fill_rect(b, WHITE);
            c.frame_rect(b);
            if hilite {
                c.line(b.left as f32, b.top as f32, b.right as f32, b.bottom as f32, BLACK);
                c.line(b.right as f32, b.top as f32, b.left as f32, b.bottom as f32, BLACK);
            }
        } else {
            c.fill_color(WHITE);
            c.op(format!("{BLACK} G 1 w"));
            c.oval_path(b, 0.5);
            c.op("B");
            if hilite {
                c.fill_color(BLACK);
                c.oval_path(Rect::new(b.left + 3, b.top + 3, b.right - 3, b.bottom - 3), 0.0);
                c.op("f");
            }
        }
        if p.show_name {
            let lines = one_line(TextAlign::Left, &label, p.text_size, &p.text_font, &p.text_style, 0.0);
            let m = metrics::metrics(&p.text_font, p.text_size, &p.text_style);
            c.lines(&lines, b.right as f32 + 5.0, cy - (m.ascent as f32 + m.descent as f32) / 2.0, fg);
        }
        return;
    }
    if has_icon {
        let label_lines =
            (p.show_name && !label.is_empty()).then(|| one_line(TextAlign::Left, &label, 9, "Geneva", "plain", 0.0));
        let block = 32.0 + label_lines.as_ref().map_or(2.0, |l| 2.0 + l[0].height);
        let top = (cy - block / 2.0 + 1.0).floor();
        let left = ((r.left + r.right) as f32 / 2.0 - 16.0).floor();
        if let Some(icon) = stack.icons.get(&p.icon) {
            let mut pic = icon.picture.clone();
            if hilite {
                for px in &mut pic.pixels {
                    *px = match *px {
                        Pixel::Black => Pixel::White,
                        Pixel::White => Pixel::Black,
                        other => other,
                    };
                }
            }
            c.picture(&pic, (left as i32, top as i32));
        }
        if let Some(lines) = label_lines {
            let w: f32 = metrics::metrics("Geneva", 9, "plain").text_width(&label);
            let x = match p.text_align {
                TextAlign::Left => r.left as f32 + 4.0,
                TextAlign::Right => r.right as f32 - 4.0 - w,
                TextAlign::Center => ((r.left + r.right) as f32 / 2.0 - w / 2.0).floor(),
            };
            if p.style == PartStyle::Transparent {
                let y = top + 33.0;
                c.fill_rect(Rect::new(x as i32 - 2, y as i32, (x + w) as i32 + 2, (y + lines[0].height) as i32), WHITE);
            }
            c.lines(&lines, x, top + 33.0, fg);
        }
        return;
    }
    if p.show_name && !label.is_empty() {
        let m = metrics::metrics(&p.text_font, p.text_size, &p.text_style);
        let width = (r.right - r.left) as f32 - 12.0;
        let lines = one_line(p.text_align, &label, p.text_size, &p.text_font, &p.text_style, width);
        let top = cy - (m.ascent + m.descent) as f32 / 2.0;
        c.op("q");
        c.clip(r);
        c.lines(&lines, r.left as f32 + 6.0, top, fg);
        c.op("Q");
    }
}

/// A popup, as on screen: the title at the left, then the shadowed menu box
/// with the chosen item and a downward triangle.
fn draw_popup(c: &mut Canvas, p: &Part, fg: f32, bg: f32) {
    let b = p.popup_box();
    c.fill_rect(Rect::new(b.left + 1, b.top + 1, b.right + 1, b.bottom + 1), BLACK);
    c.fill_rect(b, bg);
    c.frame_rect(b);
    let [(x1, y1), (x2, y2), (x3, y3)] = p.popup_triangle();
    c.fill_color(fg);
    c.op(format!("{x1} {y1} m {x2} {y2} l {x3} {y3} l h f"));
    let m = metrics::metrics(&p.text_font, p.text_size, &p.text_style);
    let top = (b.top + b.bottom) as f32 / 2.0 - (m.ascent + m.descent) as f32 / 2.0;
    let text = |c: &mut Canvas, s: &str, clip: Rect, x: i32, g: f32| {
        let lines = one_line(TextAlign::Left, s, p.text_size, &p.text_font, &p.text_style, 0.0);
        c.op("q");
        c.clip(clip);
        c.lines(&lines, x as f32, top, g);
        c.op("Q");
    };
    if p.show_name && p.title_width > 0 && !p.name.is_empty() {
        let title = Rect::new(p.rect.left, p.rect.top, b.left, p.rect.bottom);
        text(c, &p.name, title, p.rect.left + 2, if p.enabled { BLACK } else { GRAY });
    }
    let inside = Rect::new(b.left + POPUP_TEXT_INSET, b.top, x1 - 2, b.bottom);
    text(c, p.selected_text(), inside, inside.left, fg);
}

fn draw_field(c: &mut Canvas, stack: &Stack, card: Id, layer: Layer, p: &Part) {
    let r = p.rect;
    match p.style {
        PartStyle::Transparent => {}
        PartStyle::Opaque => c.fill_rect(r, WHITE),
        PartStyle::Shadow => {
            c.fill_rect(Rect::new(r.left + 2, r.top + 2, r.right + 2, r.bottom + 2), BLACK);
            c.fill_rect(r, WHITE);
            c.frame_rect(r);
        }
        _ => {
            c.fill_rect(r, WHITE);
            c.frame_rect(r);
        }
    }
    let wide = p.wide_margins as i32;
    let (mut right, mut bottom) = (4 + 3 * wide, 1);
    match p.style {
        PartStyle::Shadow => {
            right += 2;
            bottom += 2;
        }
        PartStyle::Scrolling => {
            right += 16;
            // The scroll bar's edge.
            let x = (r.right - 17) as f32 + 0.5;
            c.line(x, r.top as f32, x, r.bottom as f32, BLACK);
        }
        _ => {}
    }
    let text_rect = Rect::new(r.left + 4 + 5 * wide, r.top + 1 + 4 * wide, r.right - right, r.bottom - bottom);
    let content = Rect::new(r.left + 1, r.top + 1, text_rect.right + 3 + 3 * wide, r.bottom - bottom);
    let text = stack.field_text(card, layer, p.id);
    let runs: Vec<StyleRun> = stack.field_styles(card, layer, p.id);
    let setting = Setting {
        font: &p.text_font,
        size: p.text_size,
        style: &p.text_style,
        align: p.text_align,
        wrap: !p.dont_wrap,
        line_height: p.fixed_line_height.then(|| p.line_height()),
    };
    let width = (text_rect.right - text_rect.left) as f32;
    let lines = layout::layout(&text, &runs, &setting, width);
    let scroll = if p.style == PartStyle::Scrolling { p.scroll as f32 } else { 0.0 };
    c.op("q");
    c.clip(content);
    if p.show_lines && p.style != PartStyle::Scrolling {
        let h = p.line_height() as f32;
        let mut ys: Vec<f32> = lines.iter().map(|l| l.baseline).collect();
        let mut next = lines.last().map_or(0.0, |l| l.top + l.height);
        while next < (text_rect.bottom - text_rect.top) as f32 {
            ys.push(next + h - (h / 4.0).floor());
            next += h;
        }
        c.op("[1 1] 0 d");
        for y in ys {
            let y = text_rect.top as f32 + y + 0.5;
            let _ = writeln!(c.ops, "{GRAY} G 1 w {} {y} m {} {y} l S", text_rect.left, text_rect.right);
        }
        c.op("[] 0 d");
    }
    c.lines(&lines, text_rect.left as f32, text_rect.top as f32 - scroll, BLACK);
    c.op("Q");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts() {
        assert_eq!(pdf_font("Geneva", "bold"), "Helvetica-Bold");
        assert_eq!(pdf_font("Monaco", "plain"), "Courier");
        assert_eq!(pdf_font("New York", "bold,italic"), "Times-BoldItalic");
    }
}
