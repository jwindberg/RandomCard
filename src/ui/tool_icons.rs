//! The Tools palette's icons: SVG drawings in `tool_icons/`, drawn at the
//! size the palette needs and kept until they change. The line, shape and
//! polygon tools show their setting: the line size, Draw Filled with the
//! pattern, and the number of sides.

use std::collections::HashMap;

use eframe::egui::{self, ColorImage, TextureHandle, TextureId, TextureOptions};
use resvg::{tiny_skia, usvg};

use crate::runtime::world::Tool;

/// Each drawing is on a 32×32 grid.
const GRID: f32 = 32.0;

fn source(t: Tool) -> &'static str {
    match t {
        Tool::Browse => include_str!("tool_icons/browse.svg"),
        Tool::Button => include_str!("tool_icons/button.svg"),
        Tool::Field => include_str!("tool_icons/field.svg"),
        Tool::Select => include_str!("tool_icons/select.svg"),
        Tool::Lasso => include_str!("tool_icons/lasso.svg"),
        Tool::Pencil => include_str!("tool_icons/pencil.svg"),
        Tool::Brush => include_str!("tool_icons/brush.svg"),
        Tool::Eraser => include_str!("tool_icons/eraser.svg"),
        Tool::Line => include_str!("tool_icons/line.svg"),
        Tool::Spray => include_str!("tool_icons/spray.svg"),
        Tool::Rect => include_str!("tool_icons/rect.svg"),
        Tool::RoundRect => include_str!("tool_icons/round_rect.svg"),
        Tool::Bucket => include_str!("tool_icons/bucket.svg"),
        Tool::Oval => include_str!("tool_icons/oval.svg"),
        Tool::Curve => include_str!("tool_icons/curve.svg"),
        Tool::Text => include_str!("tool_icons/text.svg"),
        Tool::RegularPolygon => include_str!("tool_icons/regular_polygon.svg"),
        Tool::Polygon => include_str!("tool_icons/polygon.svg"),
    }
}

/// What a tool's icon shows of the paint settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Look {
    pub line_size: u32,
    pub sides: u32,
    /// Draw Filled's pattern and its color, when it's on.
    pub filled: Option<([u8; 8], [u8; 4])>,
}

impl Default for Look {
    fn default() -> Look {
        Look { line_size: 3, sides: 6, filled: None }
    }
}

impl Look {
    /// Only the settings the tool's icon shows, so the others changing
    /// don't draw it again.
    fn for_tool(self, t: Tool) -> Look {
        let plain = Look::default();
        match t {
            Tool::Line => Look { line_size: self.line_size, ..plain },
            Tool::RegularPolygon => Look { sides: self.sides, filled: self.filled, ..plain },
            Tool::Rect | Tool::RoundRect | Tool::Oval | Tool::Polygon => Look { filled: self.filled, ..plain },
            _ => plain,
        }
    }
}

/// The tool's drawing with its setting filled in.
fn svg(t: Tool, look: Look) -> String {
    let (fill, defs) = match look.filled {
        Some((bits, [r, g, b, _])) => {
            // The pattern at twice its size, so it reads in a small icon.
            let mut cells = String::new();
            for (y, row) in bits.iter().enumerate() {
                for x in 0..8 {
                    if row & (0x80 >> x) != 0 {
                        cells.push_str(&format!("M{x} {y}h1v1h-1z"));
                    }
                }
            }
            let defs = format!(
                r##"<pattern id="pat" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="scale(0.5)"><rect width="8" height="8" fill="#FFFFFF"/><path d="{cells}" fill="#{r:02X}{g:02X}{b:02X}"/></pattern>"##
            );
            ("url(#pat)".to_string(), defs)
        }
        None => ("none".to_string(), String::new()),
    };
    source(t)
        .replace("{width}", &format!("{:.1}", line_width(look.line_size)))
        .replace("{points}", &polygon_points(look.sides))
        .replace("{fill}", &fill)
        .replace("{defs}", &defs)
}

/// The Line tool's stroke for a line size, thicker as the size grows.
fn line_width(size: u32) -> f32 {
    1.2 + (size.clamp(1, 8) - 1) as f32 * 0.65
}

/// A regular polygon filling the grid, standing on a side when it can.
fn polygon_points(sides: u32) -> String {
    let n = sides.clamp(3, 50) as f32;
    let tau = std::f32::consts::TAU;
    let start = -tau / 4.0 + if sides.is_multiple_of(4) { tau / (2.0 * n) } else { 0.0 };
    let r = 11.5;
    // An odd polygon's point is up and its flat side down: center it.
    let cy = 16.0 + if !sides.is_multiple_of(2) { r * (1.0 - (tau / (2.0 * n)).cos()) / 2.0 } else { 0.0 };
    (0..sides.clamp(3, 50))
        .map(|i| {
            let a = start + i as f32 * tau / n;
            format!("{:.2},{:.2}", 16.0 + r * a.cos(), cy + r * a.sin())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Draw SVG text as an image `px` pixels square.
fn render(svg: &str, px: u32) -> Option<ColorImage> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / GRID;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Some(ColorImage::from_rgba_premultiplied([px as usize, px as usize], pixmap.data()))
}

/// The drawn icons, by tool, setting and size in pixels.
#[derive(Default)]
pub struct ToolIcons {
    drawn: HashMap<(Tool, Look, u32), TextureHandle>,
}

impl ToolIcons {
    /// The tool's icon at `points` across, drawn for the screen's pixels.
    pub fn get(&mut self, ctx: &egui::Context, t: Tool, look: Look, points: f32) -> Option<TextureId> {
        let px = (points * ctx.pixels_per_point()).round().max(1.0) as u32;
        let key = (t, look.for_tool(t), px);
        if let Some(tex) = self.drawn.get(&key) {
            return Some(tex.id());
        }
        // Settings come and go; don't keep every look ever shown.
        if self.drawn.len() > 256 {
            self.drawn.clear();
        }
        let image = render(&svg(t, key.1), px)?;
        let tex = ctx.load_texture(format!("tool-icon-{t:?}"), image, TextureOptions::LINEAR);
        let id = tex.id();
        self.drawn.insert(key, tex);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every tool's drawing reads as SVG, with each setting filled in.
    #[test]
    fn every_icon_draws() {
        let filled = Look { line_size: 8, sides: 12, filled: Some(([0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55], [200, 40, 40, 255])) };
        for t in Tool::ALL.iter().copied() {
            for look in [Look::default(), filled] {
                let text = svg(t, look);
                assert!(!text.contains('{'), "{t:?} left a setting unfilled");
                let image = render(&text, 64).unwrap_or_else(|| panic!("{t:?} didn't draw"));
                assert!(image.pixels.iter().any(|p| p.a() > 0), "{t:?} drew nothing");
            }
        }
    }

    #[test]
    fn polygon_has_its_sides() {
        assert_eq!(polygon_points(3).split(' ').count(), 3);
        assert_eq!(polygon_points(12).split(' ').count(), 12);
    }
}
