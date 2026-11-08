//! Drawing the bitmap fonts a stack carries (`model::bitmap_font`). Each
//! size of each family becomes an egui font family, `rc-bitmap:name:size`;
//! a glyph rasterizer asked before any installed font draws its characters
//! from the bitmap, pixel for pixel at its own size and in whole pixels
//! when scaled, and egui lays the text out as it does any other.

use eframe::egui::{self, Color32, ColorImage, FontFamily, Vec2};
use egui::epaint::text::{FontDefinitions, FontPriority, GlyphBitmap, GlyphRasterizer, GlyphRasterizerRequest, RasterizedGlyph};

use crate::model::Stack;
use crate::model::bitmap_font;

const PREFIX: &str = "rc-bitmap:";

/// The egui family that draws `font` at `size`, if it's a stack's bitmap
/// font: named for the size of it that's used.
pub fn family(font: &str, size: u32) -> Option<FontFamily> {
    let f = bitmap_font::font(font, size)?;
    Some(FontFamily::Name(format!("{PREFIX}{}:{}", f.family.to_lowercase(), f.size).into()))
}

/// Make a stack's fonts the ones in use: registered for measuring, and
/// installed as egui families, each falling back to the usual typeface
/// for characters it lacks, and to the system's for writing that needs
/// one (Japanese).
pub fn install(ctx: &egui::Context, stack: &Stack) {
    let fonts: Vec<_> = stack.fonts.iter().filter_map(|f| f.parse()).collect();
    let names: Vec<String> = fonts.iter().map(|f| format!("{PREFIX}{}:{}", f.family.to_lowercase(), f.size)).collect();
    bitmap_font::register(fonts);
    let mut defs = FontDefinitions::default();
    let fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for name in names {
        defs.families.insert(FontFamily::Name(name.into()), fallback.clone());
    }
    super::system_fonts::add_fallbacks(&mut defs, stack);
    ctx.set_fonts(defs);
    ctx.add_glyph_rasterizer(GlyphRasterizer::new("randomcard::bitmap_fonts", rasterize).with_priority(FontPriority::Highest));
}

/// A character in a stack's bitmap font, or None to let egui's fonts draw
/// it.
fn rasterize(req: &GlyphRasterizerRequest<'_>) -> Option<RasterizedGlyph> {
    let FontFamily::Name(name) = req.family else { return None };
    let (family, size) = name.strip_prefix(PREFIX)?.rsplit_once(':')?;
    let font = bitmap_font::font(family, size.parse().ok()?)?;
    let c = req.cluster.chars().next()?;
    let glyph = font.glyph(c)?;
    // Physical pixels for one of the font's pixels.
    let scale = req.font_size_px / font.size.max(1) as f32;
    let (w, h) = ((glyph.width as f32 * scale).round() as usize, (font.height as f32 * scale).round() as usize);
    let mut image = ColorImage::filled([w.max(1), h.max(1)], Color32::TRANSPARENT);
    if glyph.width > 0 {
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = ((x as f32 / scale) as usize, (y as f32 / scale) as usize);
                if glyph.pixels.get(sy * glyph.width as usize + sx).copied().unwrap_or(false) {
                    image[(x, y)] = Color32::WHITE;
                }
            }
        }
    }
    Some(RasterizedGlyph {
        bitmap: GlyphBitmap {
            image,
            // From the pen on the baseline to the image's top left.
            offset_px: Vec2::new(glyph.left as f32 * scale, -(font.ascent as f32) * scale),
            is_color: false,
        },
        advance_px: glyph.advance as f32 * scale,
    })
}
