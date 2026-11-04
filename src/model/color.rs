//! AddColor: the colors HyperCard 2.x's AddColor XCMD laid over a card or
//! background. The picture and parts stay black and white; each item colors
//! the white of one part, a rectangle, or shows a color picture.

use serde::{Deserialize, Serialize};

use super::{Id, Rect};

/// An RGB color, 8 bits per channel.
pub type Rgb = [u8; 3];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorItem {
    #[serde(flatten)]
    pub target: ColorTarget,
    /// AddColor could keep an item but turn it off.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ColorTarget {
    /// A button or field of the same layer, by id. `bevel` is the width of
    /// a 3D-looking edge.
    Button { id: Id, color: Rgb, bevel: u16 },
    Field { id: Id, color: Rgb, bevel: u16 },
    Rect { rect: Rect, color: Rgb, bevel: u16 },
    /// A color picture from a PICT resource or a file beside the stack,
    /// stretched to `rect`; `transparent` makes its white see-through.
    Picture { rect: Rect, name: String, transparent: bool, file: bool },
}

/// Colors by name, for scripts: the basic CSS names.
const NAMES: [(&str, [u8; 3]); 21] = [
    ("black", [0, 0, 0]),
    ("white", [255, 255, 255]),
    ("gray", [128, 128, 128]),
    ("grey", [128, 128, 128]),
    ("silver", [192, 192, 192]),
    ("red", [255, 0, 0]),
    ("maroon", [128, 0, 0]),
    ("orange", [255, 165, 0]),
    ("yellow", [255, 255, 0]),
    ("olive", [128, 128, 0]),
    ("lime", [0, 255, 0]),
    ("green", [0, 128, 0]),
    ("teal", [0, 128, 128]),
    ("cyan", [0, 255, 255]),
    ("aqua", [0, 255, 255]),
    ("blue", [0, 0, 255]),
    ("navy", [0, 0, 128]),
    ("purple", [128, 0, 128]),
    ("magenta", [255, 0, 255]),
    ("pink", [255, 192, 203]),
    ("brown", [165, 42, 42]),
];

/// A color as scripts write it: a name ("red"), red, green and blue from 0
/// to 255 ("255,128,0"), or hex ("#FF8000").
pub fn parse_color(s: &str) -> Option<Rgb> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(hex, 16).ok()?;
        return Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    }
    if s.contains(',') {
        let parts: Vec<&str> = s.split(',').map(str::trim).collect();
        let [r, g, b] = parts.as_slice() else { return None };
        let c = |x: &str| x.parse::<f64>().ok().filter(|v| (0.0..=255.0).contains(v)).map(|v| v.round() as u8);
        return Some([c(r)?, c(g)?, c(b)?]);
    }
    let name = s.to_ascii_lowercase().replace(' ', "");
    NAMES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// A color as scripts get it: "255,128,0".
pub fn color_text(c: Rgb) -> String {
    format!("{},{},{}", c[0], c[1], c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("Red"), Some([255, 0, 0]));
        assert_eq!(parse_color(" 10, 20 ,30 "), Some([10, 20, 30]));
        assert_eq!(parse_color("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_color("300,0,0"), None);
        assert_eq!(parse_color("chartreuse"), None);
        assert_eq!(color_text([1, 2, 3]), "1,2,3");
    }
}
