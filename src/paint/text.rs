//! Painted text for the text paint tool and `type`: glyphs rasterised
//! straight into the 1-bit picture.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};

use crate::model::{Picture, Pixel};

const INK_COVERAGE: f32 = 0.3;

/// Draw text with its first line's top-left at (x, y); returns where the
/// next character would go, for typing more. Lines are separated by "\n".
pub fn draw_text(pic: &mut Picture, x: i32, y: i32, text: &str, size: u32, font: &str, style: &str) -> (i32, i32) {
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
            // A 1-bit picture: ink any pixel the glyph covers noticeably, so
            // the thin strokes of a light font stay connected.
            outline.draw(|gx, gy, coverage| {
                if coverage >= INK_COVERAGE {
                    let (px, py) = (bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32);
                    pic.set(px, py, Pixel::Black);
                    if bold {
                        pic.set(px + 1, py, Pixel::Black);
                    }
                }
            });
        }
        if underline {
            let uy = caret.y as i32 + 2;
            for ux in start_x as i32..caret.x as i32 {
                pic.set(ux, uy, Pixel::Black);
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
        let (x, y) = draw_text(&mut p, 5, 5, "Hi", 24, "Geneva", "plain");
        assert!(x > 20 && y == 5);
        assert!(p.pixels.contains(&Pixel::Black));
        // A second line moves down.
        let mut q = Picture::new(120, 80);
        let (_, y2) = draw_text(&mut q, 5, 5, "a\nb", 18, "Geneva", "plain");
        assert!(y2 > 20);
    }
}
