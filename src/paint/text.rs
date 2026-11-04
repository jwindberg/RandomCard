//! Painted text for the text paint tool and `type`: glyphs rasterized
//! straight into the picture, in the line color.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};

use crate::model::{Picture, Pixel};

const INK_COVERAGE: f32 = 0.3;

/// How painted text looks.
#[derive(Clone, Copy)]
pub struct PaintText<'a> {
    pub size: u32,
    pub font: &'a str,
    pub style: &'a str,
    pub color: Pixel,
}

/// Draw text with its first line's top-left at (x, y); returns where the
/// next character would go, for typing more. Lines are separated by "\n".
pub fn draw_text(pic: &mut Picture, x: i32, y: i32, text: &str, how: &PaintText) -> (i32, i32) {
    let PaintText { size, font, style, color } = *how;
    let mono = {
        let f = font.to_ascii_lowercase();
        f.contains("courier") || f.contains("monaco") || f.contains("mono")
    };
    let data = if mono { epaint_default_fonts::HACK_REGULAR } else { epaint_default_fonts::UBUNTU_LIGHT };
    let Ok(font) = FontRef::try_from_slice(data) else { return (x, y) };
    let scale = PxScale::from(size.clamp(6, 128) as f32);
    let scaled = font.as_scaled(scale);
    let style = style.to_ascii_lowercase();
    let bold = style.contains("bold");
    let underline = style.contains("underline");
    let line_height = (scaled.height() + scaled.line_gap()).ceil();
    let mut caret = point(x as f32, y as f32 + scaled.ascent());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            caret = point(x as f32, caret.y + line_height);
        }
        let start_x = caret.x;
        let mut last = None;
        for c in line.chars() {
            let id = font.glyph_id(c);
            if let Some(prev) = last {
                caret.x += scaled.kern(prev, id);
            }
            let glyph = id.with_scale_and_position(scale, caret);
            caret.x += scaled.h_advance(id);
            last = Some(id);
            let Some(outline) = font.outline_glyph(glyph) else { continue };
            let bounds = outline.px_bounds();
            // Painting isn't smoothed: ink any pixel the glyph covers noticeably, so
            // the thin strokes of a light font stay connected.
            outline.draw(|gx, gy, coverage| {
                if coverage >= INK_COVERAGE {
                    let (px, py) = (bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32);
                    pic.set(px, py, color);
                    if bold {
                        pic.set(px + 1, py, color);
                    }
                }
            });
        }
        if underline {
            let uy = caret.y as i32 + 2;
            for ux in start_x as i32..caret.x as i32 {
                pic.set(ux, uy, color);
            }
        }
    }
    (caret.x.round() as i32, (caret.y - scaled.ascent()).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_ink_and_advances() {
        let mut p = Picture::new(120, 60);
        let (x, y) = draw_text(&mut p, 5, 5, "Hi", &PaintText { size: 24, font: "Geneva", style: "plain", color: Pixel::BLACK });
        assert!(x > 20 && y == 5);
        assert!(p.iter().any(|x| x == Pixel::BLACK));
        // A second line moves down.
        let mut q = Picture::new(120, 80);
        let (_, y2) = draw_text(&mut q, 5, 5, "a\nb", &PaintText { size: 18, font: "Geneva", style: "plain", color: Pixel::BLACK });
        assert!(y2 > 20);
    }
}
