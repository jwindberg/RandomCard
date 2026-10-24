//! HyperCard's text metrics: the line heights and character widths of the
//! classic Macintosh bitmap fonts. RandomCard draws text with its own fonts
//! but spaces it by these, so a field holds the same words on each line and
//! the same number of lines as it did in HyperCard.

mod data;

use super::styles;

pub struct FontMetrics {
    pub family: &'static str,
    pub size: u16,
    pub ascent: u8,
    pub descent: u8,
    pub leading: u8,
    /// Advance widths of MacRoman characters 32 to 255.
    pub widths: &'static [u8; 224],
}

/// A family's extra width per style, as a fraction of the text size.
pub struct FamilyStyles {
    pub family: &'static str,
    /// Bold, italic, underline, outline, shadow, condense, extend, in 4096ths.
    pub extra: [i16; 7],
}

/// Metrics for one font, size and style.
#[derive(Clone, Copy)]
pub struct Metrics {
    base: &'static FontMetrics,
    /// How much the base size is scaled, when there's no bitmap of this size.
    scale: f32,
    /// Pixels the style adds to every character.
    extra: i32,
    pub ascent: u32,
    pub descent: u32,
    pub leading: u32,
}

/// The family to measure a font by: unknown fonts were drawn in Geneva.
fn family_of(font: &str) -> &'static str {
    data::FONTS
        .iter()
        .map(|f| f.family)
        .find(|f| f.eq_ignore_ascii_case(font.trim()))
        .unwrap_or("Geneva")
}

/// The bitmap QuickDraw would use: this size, or the nearest one scaled.
fn base_font(family: &str, size: u32) -> &'static FontMetrics {
    data::FONTS
        .iter()
        .filter(|f| f.family == family)
        .min_by_key(|f| ((f.size as i64 - size as i64).abs(), f.size < size as u16))
        .expect("every family has a size")
}

/// QuickDraw's rounding of style extras: halves round down.
fn extra_width(fraction: i16, size: u32) -> i32 {
    let v = fraction as f32 / 4096.0 * size as f32;
    if v - v.floor() == 0.5 { v.floor() as i32 } else { v.round() as i32 }
}

pub fn metrics(font: &str, size: u32, style: &str) -> Metrics {
    let family = family_of(font);
    let size = size.clamp(1, 255);
    let base = base_font(family, size);
    let scale = size as f32 / base.size as f32;
    let fractions = data::STYLES.iter().find(|s| s.family == family).map(|s| s.extra);
    let has = |s| styles::has_style(style, s);
    // Bold, italic, underline, outline, shadow, condense, extend.
    let extra_for = |i: usize, default: i32| fractions.map_or(default, |f| extra_width(f[i], size));
    let mut extra = 0;
    if has("bold") {
        extra += extra_for(0, 1);
    }
    if has("outline") || has("shadow") {
        extra += extra_for(3, 1);
    }
    if has("shadow") {
        extra += if has("outline") { 2 } else { 1 } * extra_for(4, 1);
    }
    if has("condense") {
        extra += extra_for(5, -1);
    }
    if has("extend") {
        extra += extra_for(6, 1);
    }
    let scaled = |v: u8| (v as f32 * scale).round() as u32;
    Metrics {
        base,
        scale,
        extra,
        ascent: scaled(base.ascent),
        descent: scaled(base.descent),
        leading: scaled(base.leading),
    }
}

impl Metrics {
    /// The height of a line in this font.
    pub fn line_height(&self) -> u32 {
        self.ascent + self.descent + self.leading
    }

    /// How far a character advances the pen.
    pub fn width(&self, c: char) -> f32 {
        let code = mac_roman_code(c).unwrap_or(b'?');
        let w = match code {
            32.. => self.base.widths[(code - 32) as usize],
            b'\t' => self.base.widths[0],
            _ => 0,
        };
        ((w as f32 * self.scale).round() as i32 + self.extra).max(0) as f32
    }

    pub fn text_width(&self, text: &str) -> f32 {
        text.chars().map(|c| self.width(c)).sum()
    }
}

/// HyperCard's automatic textHeight for a text size.
pub fn default_text_height(size: u32) -> u32 {
    (size * 4).div_ceil(3)
}

fn mac_roman_code(c: char) -> Option<u8> {
    if (c as u32) < 0x80 {
        return Some(c as u8);
    }
    crate::import::MAC_ROMAN_HIGH.iter().position(|h| *h == c).map(|i| 0x80 + i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hypercard_metrics() {
        let g = metrics("Geneva", 12, "plain");
        assert_eq!((g.ascent, g.descent, g.leading, g.line_height()), (12, 3, 1, 16));
        assert_eq!(g.width(' '), 4.0);
        // Bold adds a pixel to every character in Geneva, which has no style table.
        assert_eq!(metrics("Geneva", 12, "bold").width(' '), 5.0);
        // Courier is fixed width, and stays so when bold.
        let c = metrics("Courier", 12, "bold");
        assert_eq!(c.width('i'), c.width('W'));
        // Unknown fonts measure as Geneva; missing sizes are scaled.
        assert_eq!(metrics("Boston", 12, "plain").width('m'), g.width('m'));
        assert!(metrics("Geneva", 36, "plain").line_height() > metrics("Geneva", 24, "plain").line_height());
        assert!(metrics("Geneva", 12, "plain").width('é') > 0.0);
    }
}
