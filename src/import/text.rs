//! The text encodings of HyperCard stacks. A stack's text is in the Mac
//! OS script of the system it was made on: MacRoman for most, MacJapanese
//! (Shift-JIS with Apple's additions) for stacks made with Japanese
//! HyperCard under KanjiTalk. Everything comes into RandomCard as Unicode.

use super::MAC_ROMAN_HIGH;

/// Mac OS text encodings, as Apple named them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextEncoding {
    #[default]
    MacRoman,
    MacJapanese,
}

/// Japanese fonts' ids: the Mac's script code 1 gave them 16384–16895.
const JAPANESE_FONTS: std::ops::RangeInclusive<i16> = 16384..=16895;

impl TextEncoding {
    /// The encoding a stack's text is in, from the ids of the fonts in its
    /// font table: Japanese stacks use Japanese fonts (Osaka, 細明朝体…).
    pub fn of_fonts(ids: impl IntoIterator<Item = i16>) -> Self {
        if ids.into_iter().any(|id| JAPANESE_FONTS.contains(&id)) { Self::MacJapanese } else { Self::MacRoman }
    }

    /// Decode text, turning classic Mac carriage returns into newlines.
    pub fn decode(self, bytes: &[u8]) -> String {
        self.decode_with_positions(bytes).0
    }

    /// Decode a script. HyperTalk's line continuation, option-return, is
    /// the byte MacRoman shows as ¬; MacJapanese shows it as the katakana
    /// ﾂ, and HyperCard read it as a continuation there too.
    pub fn decode_script(self, bytes: &[u8]) -> String {
        let text = self.decode(bytes);
        match self {
            Self::MacRoman => text,
            Self::MacJapanese => text.replace("ﾂ\n", "¬\n"),
        }
    }

    /// The encoding of text in a font: its script's. The system and
    /// application fonts (0 and 1) were the system's own, which was the
    /// stack's.
    pub fn for_font(self, id: i16) -> Self {
        match id {
            0 | 1 => self,
            id if JAPANESE_FONTS.contains(&id) => Self::MacJapanese,
            _ => Self::MacRoman,
        }
    }

    /// Decode text, with the character position each byte position starts
    /// in (one more entry than bytes, for the end), as style runs count in
    /// bytes.
    pub fn decode_with_positions(self, bytes: &[u8]) -> (String, Vec<usize>) {
        decode_mixed(bytes, |_| self)
    }
}

/// Decode text whose encoding changes along it, as a field's does with
/// its fonts: `enc_at` gives the encoding at a byte position. Returns the
/// text and, as `decode_with_positions` does, each byte's character.
pub fn decode_mixed(bytes: &[u8], enc_at: impl Fn(usize) -> TextEncoding) -> (String, Vec<usize>) {
    let mut text = String::with_capacity(bytes.len());
    let mut positions = Vec::with_capacity(bytes.len() + 1);
    let mut chars = 0;
    let mut i = 0;
    while i < bytes.len() {
        let (c, len) = match enc_at(i) {
            TextEncoding::MacRoman => (mac_roman_char(bytes[i]), 1),
            TextEncoding::MacJapanese => mac_japanese_char(&bytes[i..]),
        };
        positions.extend(std::iter::repeat_n(chars, len));
        text.push(c);
        chars += 1;
        i += len;
    }
    positions.push(chars);
    (text, positions)
}

fn mac_roman_char(b: u8) -> char {
    match b {
        b'\r' => '\n',
        b if b < 0x80 => b as char,
        b => MAC_ROMAN_HIGH[(b - 0x80) as usize],
    }
}

/// One character of MacJapanese and its length in bytes, by Apple's
/// mapping (JAPANESE.TXT): Shift-JIS, with ¥ at 0x5C, the backslash at
/// 0x80, and ©, ™ and … at 0xFD–0xFF.
fn mac_japanese_char(bytes: &[u8]) -> (char, usize) {
    match bytes[0] {
        b'\r' => ('\n', 1),
        0x5c => ('¥', 1),
        b if b < 0x80 => (b as char, 1),
        0x80 => ('\\', 1),
        0xa0 => ('\u{a0}', 1),
        // Half-width katakana.
        b @ 0xa1..=0xdf => (char::from_u32(0xff61 + (b - 0xa1) as u32).unwrap_or('\u{fffd}'), 1),
        0xfd => ('©', 1),
        0xfe => ('™', 1),
        0xff => ('…', 1),
        // A lead byte and the one after.
        _ => match bytes.get(..2) {
            Some(pair) => {
                let (s, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(pair);
                let mut cs = s.chars();
                match (cs.next(), cs.next()) {
                    (Some(c), None) if c != '\u{fffd}' => (c, 2),
                    _ => ('\u{fffd}', 1),
                }
            }
            None => ('\u{fffd}', 1),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::TextEncoding::*;
    use super::*;

    #[test]
    fn mac_japanese() {
        // 花札スタック, the Hanafuda stack's name.
        let bytes = [0x89, 0xd4, 0x8e, 0x44, 0x83, 0x58, 0x83, 0x5e, 0x83, 0x62, 0x83, 0x4e];
        assert_eq!(MacJapanese.decode(&bytes), "花札スタック");
        assert_eq!(MacJapanese.decode(b"a\rb\x5c\xb1\xfe"), "a\nb¥ｱ™");
        let (text, positions) = MacJapanese.decode_with_positions(b"a\x89\xd4b");
        assert_eq!(text, "a花b");
        assert_eq!(positions, vec![0, 1, 1, 2, 3]);
        // A lead byte with nothing after it.
        assert_eq!(MacJapanese.decode(b"a\x89"), "a\u{fffd}");
        // Option-return continues a script's line.
        assert_eq!(MacJapanese.decode_script(b"put 1 +\xc2\r2\r"), "put 1 +¬\n2\n");
        assert_eq!(MacJapanese.decode(b"\xc2\r"), "ﾂ\n");
    }

    #[test]
    fn of_fonts() {
        assert_eq!(TextEncoding::of_fonts([3, 16384]), MacJapanese);
        assert_eq!(TextEncoding::of_fonts([0, 3, 21]), MacRoman);
        // Geneva is Roman even in a Japanese stack; the system font is the stack's.
        assert_eq!(MacJapanese.for_font(3), MacRoman);
        assert_eq!(MacJapanese.for_font(0), MacJapanese);
        assert_eq!(MacRoman.for_font(16384), MacJapanese);
    }

    #[test]
    fn mixed() {
        // ©1990 in Geneva, then 花 in Osaka.
        let (text, positions) = decode_mixed(b"\xa91990\x89\xd4", |i| if i < 5 { MacRoman } else { MacJapanese });
        assert_eq!(text, "©1990花");
        assert_eq!(positions, vec![0, 1, 2, 3, 4, 5, 5, 6]);
    }
}
