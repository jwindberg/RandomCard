//! AddColor's HCcd (card) and HCbg (background) resources, after
//! HyperCardPreview's LayerColorLoading. Each is a list of items: a type
//! byte (high bit set when the item is turned off), then the item.

use crate::model::color::{ColorItem, ColorTarget, Rgb};
use crate::model::{Rect, Stack};

use super::TextEncoding;
use super::resources::Resource;

/// Give the stack's cards and backgrounds their AddColor items.
pub fn apply(stack: &mut Stack, resources: &[Resource], enc: TextEncoding) {
    for r in resources {
        let id = r.id as u16 as u32;
        match &r.kind {
            b"HCcd" => {
                if let Some(c) = stack.cards.iter_mut().find(|c| c.id == id) {
                    c.colors = items(r.data, enc);
                }
            }
            b"HCbg" => {
                if let Some(b) = stack.backgrounds.iter_mut().find(|b| b.id == id) {
                    b.colors = items(r.data, enc);
                }
            }
            _ => {}
        }
    }
}

/// Keep the color pictures the stack's AddColor items show, and those its
/// scripts name in quotes (`addColor addPict, cd, "Sky", …`): PICT
/// resources from its fork, and PICT files beside it.
pub fn pictures(stack: &mut Stack, resources: &[Resource], path: &std::path::Path) {
    let items = stack.cards.iter().flat_map(|c| &c.colors).chain(stack.backgrounds.iter().flat_map(|b| &b.colors));
    let mut wanted: Vec<(String, bool)> = items
        .filter_map(|i| match &i.target {
            ColorTarget::Picture { name, file, .. } => Some((name.clone(), *file)),
            _ => None,
        })
        .collect();
    let scripts = super::all_scripts(stack).to_lowercase();
    for r in resources.iter().filter(|r| &r.kind == b"PICT" && !r.name.is_empty()) {
        let named = scripts.contains(&format!("\"{}\"", r.name.to_lowercase()));
        if named || wanted.iter().any(|(n, f)| !f && n.eq_ignore_ascii_case(&r.name)) {
            wanted.retain(|(n, f)| *f || !n.eq_ignore_ascii_case(&r.name));
            if let Some(p) = super::pict::decode(r.data) {
                stack.color_pictures.insert(crate::model::color_picture_key(&r.name, false), std::sync::Arc::new(p.to_color_picture()));
            }
        }
    }
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    for (name, _) in wanted.into_iter().filter(|(_, f)| *f) {
        if let Some(p) = picture_file(dir, &name) {
            stack.color_pictures.insert(crate::model::color_picture_key(&name, true), std::sync::Arc::new(p));
        }
    }
}

/// A picture file AddColor names, beside the stack: a name, or a Mac path
/// from the stack's folder (":Pictures:Sky") or from its disk, of which
/// the end is tried beside the stack.
pub fn picture_file(dir: &std::path::Path, name: &str) -> Option<crate::model::Picture> {
    let parts: Vec<&str> = name.split(':').filter(|p| !p.is_empty()).collect();
    let tries = (0..parts.len()).map(|from| parts[from..].iter().fold(dir.to_path_buf(), |p, part| p.join(part.replace('/', ":"))));
    tries.chain(std::iter::once(std::path::PathBuf::from(name))).find_map(|p| crate::paint::image_file::decode(&std::fs::read(&p).ok()?).ok())
}

/// The items in one resource; a damaged or unknown item ends the list.
pub fn items(data: &[u8], enc: TextEncoding) -> Vec<ColorItem> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some((item, len)) = item(&data[at..], enc) {
        out.push(item);
        at += len;
    }
    out
}

fn item(d: &[u8], enc: TextEncoding) -> Option<(ColorItem, usize)> {
    let u16_at = |i: usize| d.get(i..i + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let rect_at = |i: usize| -> Option<Rect> {
        let v = |k| u16_at(i + k).map(|n| n as i16 as i32);
        Some(Rect::new(v(2)?, v(0)?, v(6)?, v(4)?))
    };
    // 16-bit channels, kept to 8.
    let color_at = |i: usize| -> Option<Rgb> { Some([d.get(i).copied()?, d.get(i + 2).copied()?, d.get(i + 4).copied()?]) };
    let first = *d.first()?;
    let enabled = first & 0x80 == 0;
    let (target, len) = match first & 0x7f {
        1 | 2 => {
            let (id, bevel, color) = (u16_at(1)? as u32, u16_at(3)?, color_at(5)?);
            let target = if first & 0x7f == 1 {
                ColorTarget::Button { id, color, bevel }
            } else {
                ColorTarget::Field { id, color, bevel }
            };
            (target, 11)
        }
        3 => (ColorTarget::Rect { rect: rect_at(1)?, bevel: u16_at(9)?, color: color_at(11)? }, 17),
        kind @ (4 | 5) => {
            let len = *d.get(10)? as usize;
            let name = enc.decode(d.get(11..11 + len)?);
            let target = ColorTarget::Picture { rect: rect_at(1)?, name, transparent: *d.get(9)? != 0, file: kind == 5 };
            (target, 11 + len)
        }
        _ => return None,
    };
    Some((ColorItem { target, enabled }, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_items() {
        let mut d = vec![1, 0, 7, 0, 2, 0xff, 0xff, 0x80, 0x00, 0, 0]; // button 7, bevel 2, orange-ish
        d.extend([0x83, 0, 10, 0, 20, 0, 30, 0, 40, 0, 0, 0, 0, 0xff, 0xff, 0, 0]); // rect, turned off
        d.extend([4, 0, 0, 0, 0, 0, 100, 0, 50, 1, 3, b'S', b'k', b'y']);
        let items = items(&d, TextEncoding::MacRoman);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], ColorItem { target: ColorTarget::Button { id: 7, color: [255, 128, 0], bevel: 2 }, enabled: true });
        assert_eq!(
            items[1],
            ColorItem { target: ColorTarget::Rect { rect: Rect::new(20, 10, 40, 30), color: [0, 255, 0], bevel: 0 }, enabled: false }
        );
        assert!(matches!(&items[2].target, ColorTarget::Picture { name, transparent: true, file: false, .. } if name == "Sky"));
    }
}
