//! Bitmap fonts a stack carries: classic Mac `FONT` and `NFNT` resources,
//! as HyperCard stacks shipped them (Blah Blob!'s "Sprites" draws its
//! levels as text). Each is one size of a family: per character, a strip
//! of the font's bit image, its advance and its offset from the pen.
//!
//! Layout (Inside Macintosh: Text, "The Font Resource"): a 26-byte header
//! (fontType, firstChar, lastChar, widMax, kernMax, nDescent, fRectWidth,
//! fRectHeight, owTLoc, ascent, descent, leading, rowWords), the bit image
//! (rowWords × 2 bytes a row, fRectHeight rows), the location table and the
//! offset/width table, each (lastChar - firstChar + 3) words.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// One character's look.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    /// How far the pen moves, in pixels.
    pub advance: u32,
    /// Where its image starts, from the pen (kernMax plus its offset).
    pub left: i32,
    /// Its image: `width` pixels a row, the font's height in rows, from the
    /// font's top (ascent above the baseline).
    pub width: u32,
    pub pixels: Vec<bool>,
}

/// One size of a bitmap font family.
#[derive(Clone, Debug, PartialEq)]
pub struct BitmapFont {
    pub family: String,
    pub size: u32,
    pub ascent: u32,
    pub descent: u32,
    pub leading: u32,
    pub height: u32,
    /// By character, as the text holds it (from Mac Roman).
    pub glyphs: HashMap<char, Glyph>,
    /// For characters the font lacks.
    pub missing: Option<Glyph>,
}

fn be16(d: &[u8], at: usize) -> Option<i16> {
    d.get(at..at + 2).map(|b| i16::from_be_bytes([b[0], b[1]]))
}

impl BitmapFont {
    /// A `FONT` or `NFNT` resource's data, for this family name and size.
    pub fn parse(family: &str, size: u32, d: &[u8]) -> Option<BitmapFont> {
        let first = be16(d, 2)? as i32;
        let last = be16(d, 4)? as i32;
        let kern_max = be16(d, 8)? as i32;
        let height = be16(d, 14)?.max(0) as u32;
        let ow_loc = be16(d, 16)? as usize;
        let ascent = be16(d, 18)?.max(0) as u32;
        let descent = be16(d, 20)?.max(0) as u32;
        let leading = be16(d, 22)?.max(0) as u32;
        let row_words = be16(d, 24)?.max(0) as usize;
        if last < first || height == 0 || row_words == 0 {
            return None;
        }
        let count = (last - first + 3) as usize;
        let image = 26;
        let row_bytes = row_words * 2;
        let loc_table = image + row_bytes * height as usize;
        // owTLoc counts words from its own place in the header (byte 16).
        let ow_table = 16 + ow_loc * 2;
        let bit = |x: usize, y: usize| -> bool {
            d.get(image + y * row_bytes + x / 8).is_some_and(|b| b & (0x80 >> (x % 8)) != 0)
        };
        let glyph = |i: usize| -> Option<Glyph> {
            let ow = be16(d, ow_table + i * 2)?;
            if ow == -1 {
                return None;
            }
            let (offset, advance) = ((ow as u16 >> 8) as i32, (ow as u16 & 0xff) as u32);
            let (from, to) = (be16(d, loc_table + i * 2)? as usize, be16(d, loc_table + (i + 1) * 2)? as usize);
            let width = to.saturating_sub(from) as u32;
            let mut pixels = Vec::with_capacity((width * height) as usize);
            for y in 0..height as usize {
                for x in from..to {
                    pixels.push(bit(x, y));
                }
            }
            Some(Glyph { advance, left: kern_max + offset, width, pixels })
        };
        let mut glyphs = HashMap::new();
        for code in first..=last {
            if let Some(g) = glyph((code - first) as usize) {
                let ch = crate::import::mac_roman(&[code as u8]).chars().next().unwrap_or('\0');
                glyphs.insert(ch, g);
            }
        }
        let missing = glyph(count - 2);
        Some(BitmapFont { family: family.to_string(), size, ascent, descent, leading, height, glyphs, missing })
    }

    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs.get(&c).or(self.missing.as_ref())
    }

    /// How wide text is in this font, in pixels.
    #[cfg(test)]
    pub fn text_width(&self, text: &str) -> u32 {
        text.chars().filter(|c| *c != '\n').filter_map(|c| self.glyph(c)).map(|g| g.advance).sum()
    }
}

/// The bitmap fonts of the stacks open now, by family (lowercase), every
/// size of each. RandomCard's own fonts come from its metrics tables; these
/// come first.
fn registry() -> &'static RwLock<HashMap<String, Vec<Arc<BitmapFont>>>> {
    static FONTS: OnceLock<RwLock<HashMap<String, Vec<Arc<BitmapFont>>>>> = OnceLock::new();
    FONTS.get_or_init(Default::default)
}

/// Make these fonts the ones in use, in place of any before.
pub fn register(fonts: Vec<BitmapFont>) {
    let mut map: HashMap<String, Vec<Arc<BitmapFont>>> = HashMap::new();
    for f in fonts {
        map.entry(f.family.to_lowercase()).or_default().push(Arc::new(f));
    }
    for sizes in map.values_mut() {
        sizes.sort_by_key(|f| f.size);
    }
    *registry().write().unwrap_or_else(|p| p.into_inner()) = map;
}

/// Whether a family is a stack's bitmap font.
#[cfg(test)]
pub fn has_family(family: &str) -> bool {
    registry().read().unwrap_or_else(|p| p.into_inner()).contains_key(&family.to_lowercase())
}

/// The size of a family nearest to `size`: an exact one, else the nearest
/// below, else the smallest.
pub fn font(family: &str, size: u32) -> Option<Arc<BitmapFont>> {
    let map = registry().read().unwrap_or_else(|p| p.into_inner());
    let sizes = map.get(&family.to_lowercase())?;
    sizes.iter().rev().find(|f| f.size <= size).or_else(|| sizes.first()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny FONT: characters 'A' and 'B', 3 rows high, in one 16-pixel
    /// row of bit image. 'A' is 2 pixels wide (the left column set), 'B' 3
    /// (all set); B advances 4 and is offset 1.
    fn tiny() -> Vec<u8> {
        let mut d = Vec::new();
        let (first, last, rows, row_words) = (b'A' as i16, b'B' as i16, 3i16, 1i16);
        let count = (last - first + 3) as usize;
        // ow table after the image and loc table, counted from byte 16 in words.
        let ow_loc = ((26 + 2 * rows as usize + 2 * count) - 16) / 2;
        for v in [0i16, first, last, 4, -1, 2, 4, rows, ow_loc as i16, 2, 1, 1, row_words] {
            d.extend(v.to_be_bytes());
        }
        // Rows: A is bits 0-1 with only bit 0 set; B bits 2-4 all set.
        for _ in 0..rows {
            d.extend([0b1011_1000u8, 0]);
        }
        for v in [0i16, 2, 5, 5] {
            d.extend(v.to_be_bytes()); // loc: A at 0, B at 2, missing glyph empty, end
        }
        for v in [(0u8, 2u8), (1, 4), (0, 1), (0, 0)] {
            d.extend([v.0, v.1]);
        }
        d
    }

    #[test]
    fn reads_a_font() {
        let f = BitmapFont::parse("Tiny", 3, &tiny()).unwrap();
        assert_eq!((f.ascent, f.descent, f.height), (2, 1, 3));
        let a = f.glyph('A').unwrap();
        assert_eq!((a.advance, a.left, a.width), (2, -1, 2));
        assert_eq!(a.pixels, vec![true, false, true, false, true, false]);
        let b = f.glyph('B').unwrap();
        assert_eq!((b.advance, b.left, b.width), (4, 0, 3));
        assert!(b.pixels.iter().all(|p| *p));
        assert_eq!(f.text_width("AB"), 6);
        // A character it lacks: the missing-character glyph.
        assert_eq!(f.glyph('Z').unwrap().advance, 1);
    }

    #[test]
    fn registry_finds_sizes() {
        let mut small = BitmapFont::parse("Tiny", 9, &tiny()).unwrap();
        small.size = 9;
        let mut big = small.clone();
        big.size = 18;
        register(vec![big, small]);
        assert!(has_family("TINY"));
        assert_eq!(font("tiny", 12).unwrap().size, 9);
        assert_eq!(font("tiny", 18).unwrap().size, 18);
        assert_eq!(font("tiny", 4).unwrap().size, 9);
        register(Vec::new());
        assert!(!has_family("tiny"));
    }
}
