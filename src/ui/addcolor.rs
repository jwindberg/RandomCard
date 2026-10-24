//! Drawing AddColor. HyperCard 2.x's AddColor painted its colors over the
//! black-and-white card in "darken" mode: black stays black and white takes
//! the color. RandomCard does the same by coloring the white of the card's
//! pictures from a color map, and filling opaque parts with their color.

use std::hash::{DefaultHasher, Hash, Hasher};

use eframe::egui::Color32;

use crate::model::color::ColorTarget;
use crate::model::{Id, Part, PartKind, PartStyle, Rect, Stack};

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum Shape {
    Rect,
    Oval,
    /// A rounded rectangle with this corner radius.
    Round(i32),
}

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct ColorShape {
    pub rect: Rect,
    pub shape: Shape,
    /// RGBA; transparent parts are colored at partial opacity.
    pub color: [u8; 4],
    pub bevel: i32,
}

/// How much color a transparent part gets, as in HyperCardPreview.
const TRANSPARENT_ALPHA: u8 = 161;

/// The shapes to color for a card, background items first. Editing the
/// background shows only its own.
pub fn shapes(stack: &Stack, card: Id, edit_bg: bool) -> Vec<ColorShape> {
    let Some(c) = stack.card(card) else { return Vec::new() };
    let bg = stack.background(c.background);
    let mut out = Vec::new();
    let mut add = |items: &[crate::model::color::ColorItem], parts: &[Part]| {
        for item in items.iter().filter(|i| i.enabled) {
            let part = |id: Id, kind: PartKind| parts.iter().find(|p| p.id == id && p.kind == kind && p.visible);
            let shape = match &item.target {
                ColorTarget::Button { id, color, bevel } => part(*id, PartKind::Button).and_then(|p| part_shape(p, *color, *bevel)),
                ColorTarget::Field { id, color, bevel } => part(*id, PartKind::Field).and_then(|p| part_shape(p, *color, *bevel)),
                ColorTarget::Rect { rect, color, bevel } => {
                    Some(ColorShape { rect: *rect, shape: Shape::Rect, color: opaque(*color), bevel: *bevel as i32 })
                }
                // PICT pictures aren't decoded yet.
                ColorTarget::Picture { .. } => None,
            };
            out.extend(shape);
        }
    };
    if let Some(bg) = bg {
        add(&bg.colors, &bg.parts);
    }
    if !edit_bg {
        add(&c.colors, &c.parts);
    }
    out
}

fn opaque([r, g, b]: [u8; 3]) -> [u8; 4] {
    [r, g, b, 255]
}

/// The area a colored part covers: its white, without the shadow.
fn part_shape(p: &Part, color: [u8; 3], bevel: u16) -> Option<ColorShape> {
    let r = p.rect;
    let mut color = opaque(color);
    let (rect, shape) = match (p.kind, p.style) {
        (_, PartStyle::Transparent) => {
            color[3] = TRANSPARENT_ALPHA;
            (r, Shape::Rect)
        }
        (PartKind::Button, PartStyle::Shadow) => (Rect::new(r.left, r.top, r.right - 1, r.bottom - 1), Shape::Rect),
        (PartKind::Field, PartStyle::Shadow) => (Rect::new(r.left, r.top, r.right - 2, r.bottom - 2), Shape::Rect),
        (PartKind::Button, PartStyle::RoundRect) => (r, Shape::Round(8)),
        (PartKind::Button, PartStyle::Default) => (Rect::new(r.left + 4, r.top + 4, r.right - 4, r.bottom - 4), Shape::Round(6)),
        (PartKind::Button, PartStyle::Oval) => (r, Shape::Oval),
        _ => (r, Shape::Rect),
    };
    Some(ColorShape { rect, shape, color, bevel: bevel as i32 })
}

fn covers(s: &ColorShape, x: i32, y: i32) -> bool {
    let r = s.rect;
    if !r.contains(x, y) {
        return false;
    }
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    match s.shape {
        Shape::Rect => true,
        Shape::Oval => {
            let (rx, ry) = ((r.right - r.left) as f32 / 2.0, (r.bottom - r.top) as f32 / 2.0);
            let (dx, dy) = ((fx - r.left as f32 - rx) / rx, (fy - r.top as f32 - ry) / ry);
            dx * dx + dy * dy <= 1.0
        }
        Shape::Round(radius) => {
            let rad = radius.min((r.right - r.left) / 2).min((r.bottom - r.top) / 2) as f32;
            let cx = fx.clamp(r.left as f32 + rad, r.right as f32 - rad);
            let cy = fy.clamp(r.top as f32 + rad, r.bottom as f32 - rad);
            (fx - cx).powi(2) + (fy - cy).powi(2) <= rad * rad
        }
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The color the card's white takes at a pixel.
pub fn color_at(shapes: &[ColorShape], x: i32, y: i32) -> Color32 {
    let mut c = [255.0f32; 3];
    for s in shapes.iter().filter(|s| covers(s, x, y)) {
        let mut col = [s.color[0] as f32, s.color[1] as f32, s.color[2] as f32];
        if s.bevel > 0 {
            // A raised edge: lighter at the top and left, darker at the bottom and right.
            let r = s.rect;
            let near = (x - r.left).min(y - r.top);
            let far = (r.right - 1 - x).min(r.bottom - 1 - y);
            if near < s.bevel && near <= far {
                col = mix(col, [255.0; 3], 0.5);
            } else if far < s.bevel {
                col = mix(col, [0.0; 3], 0.5);
            }
        }
        c = mix(c, col, s.color[3] as f32 / 255.0);
    }
    Color32::from_rgb(c[0].round() as u8, c[1].round() as u8, c[2].round() as u8)
}

/// The color of every pixel of a card-sized map, row by row.
pub fn color_map(shapes: &[ColorShape], width: u32, height: u32) -> Vec<Color32> {
    let mut out = Vec::with_capacity((width * height) as usize);
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            out.push(color_at(shapes, x, y));
        }
    }
    out
}

/// Identifies a set of shapes, for caching what's drawn from them; 0 for none.
pub fn key(shapes: &[ColorShape]) -> u64 {
    if shapes.is_empty() {
        return 0;
    }
    let mut h = DefaultHasher::new();
    shapes.hash(&mut h);
    h.finish() | 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_bevels() {
        let red = ColorShape { rect: Rect::new(0, 0, 10, 10), shape: Shape::Rect, color: [255, 0, 0, 255], bevel: 2 };
        assert_eq!(color_at(std::slice::from_ref(&red), 5, 5), Color32::from_rgb(255, 0, 0));
        assert_eq!(color_at(std::slice::from_ref(&red), 0, 5), Color32::from_rgb(255, 128, 128));
        assert_eq!(color_at(std::slice::from_ref(&red), 9, 5), Color32::from_rgb(128, 0, 0));
        assert_eq!(color_at(std::slice::from_ref(&red), 20, 5), Color32::WHITE);
        let oval = ColorShape { shape: Shape::Oval, bevel: 0, ..red.clone() };
        assert_eq!(color_at(std::slice::from_ref(&oval), 0, 0), Color32::WHITE);
        // A transparent part's color is lighter.
        let faint = ColorShape { color: [0, 0, 255, TRANSPARENT_ALPHA], bevel: 0, ..red };
        let c = color_at(&[faint], 5, 5);
        assert!(c.b() == 255 && c.r() > 0 && c.r() < 255);
    }
}
