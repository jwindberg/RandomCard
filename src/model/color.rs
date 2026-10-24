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
