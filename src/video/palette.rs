//! Classic QuickTime movies in 256 colors (Apple's Graphics codec, for one)
//! can carry their own color table. GStreamer decodes them with the
//! standard Mac palette instead, so their colors come out wrong. Both are
//! tables of 256 colors, so each standard color names a color number, and
//! the movie's table says what that number really is: frames are put right
//! color by color.

use std::collections::HashMap;
use std::path::Path;

/// The standard Mac 256-color palette's color `i`: a 6×6×6 cube from
/// white down, then ramps of red, green, blue and gray, then black.
pub fn standard(i: u8) -> [u8; 3] {
    let i = i as u32;
    if i < 215 {
        let c = |n: u32| (255 - 51 * n) as u8;
        return [c(i / 36), c(i / 6 % 6), c(i % 6)];
    }
    if i == 255 {
        return [0, 0, 0];
    }
    // The ramps skip the cube's steps: EE, DD, BB, AA, 88, 77, 55, 44, 22, 11.
    const RAMP: [u8; 10] = [0xEE, 0xDD, 0xBB, 0xAA, 0x88, 0x77, 0x55, 0x44, 0x22, 0x11];
    let (ramp, step) = ((i - 215) / 10, RAMP[((i - 215) % 10) as usize]);
    match ramp {
        0 => [step, 0, 0],
        1 => [0, step, 0],
        2 => [0, 0, step],
        _ => [step, step, step],
    }
}

/// Standard colors to the movie's own, for a movie whose first video track
/// is in 256 colors with a color table of its own. None for any other.
pub fn remap_for(path: &Path) -> Option<HashMap<[u8; 3], [u8; 3]>> {
    let data = std::fs::read(path).ok()?;
    let table = color_table(&data)?;
    let map: HashMap<[u8; 3], [u8; 3]> = table.into_iter().map(|(i, c)| (standard(i), c)).filter(|(from, to)| from != to).collect();
    (!map.is_empty()).then_some(map)
}

/// Put a frame's colors right, in place: RGBA pixels.
pub fn remap(rgba: &mut [u8], map: &HashMap<[u8; 3], [u8; 3]>) {
    for px in rgba.as_chunks_mut::<4>().0 {
        if let Some(c) = map.get(&[px[0], px[1], px[2]]) {
            px[..3].copy_from_slice(c);
        }
    }
}

/// The color table in the first 8-bit video track's sample description:
/// color numbers and their colors.
fn color_table(data: &[u8]) -> Option<Vec<(u8, [u8; 3])>> {
    let moov = find_atom(data, 0, data.len(), b"moov")?;
    for trak in atoms(data, moov.0, moov.1).filter(|a| &a.0 == b"trak") {
        let Some(mdia) = find_atom(data, trak.1, trak.2, b"mdia") else { continue };
        let video = find_atom(data, mdia.0, mdia.1, b"hdlr").is_some_and(|(s, _)| data.get(s + 8..s + 12) == Some(b"vide"));
        if !video {
            continue;
        }
        let minf = find_atom(data, mdia.0, mdia.1, b"minf")?;
        let stbl = find_atom(data, minf.0, minf.1, b"stbl")?;
        let (s, e) = find_atom(data, stbl.0, stbl.1, b"stsd")?;
        // The first sample description, after the stsd's version, flags
        // and count: size, format, then the video fields.
        let entry = s + 8;
        let size = be32(data, entry)? as usize;
        if size < 86 || entry + size > e {
            return None;
        }
        let depth = be16(data, entry + 82)?;
        let table_id = be16(data, entry + 84)? as i16;
        if depth != 8 || table_id != 0 || size < 94 {
            return None;
        }
        let count = be16(data, entry + 92)? as usize + 1;
        let mut out = Vec::new();
        for k in 0..count.min(256) {
            let at = entry + 94 + k * 8;
            if at + 8 > entry + size {
                break;
            }
            let (index, r, g, b) = (be16(data, at)?, be16(data, at + 2)?, be16(data, at + 4)?, be16(data, at + 6)?);
            if index < 256 {
                out.push((index as u8, [(r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8]));
            }
        }
        return Some(out);
    }
    None
}

fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// The atoms from `start` to `end`: each one's type, and where its
/// contents start and end.
fn atoms(d: &[u8], start: usize, end: usize) -> impl Iterator<Item = ([u8; 4], usize, usize)> + '_ {
    let mut at = start;
    std::iter::from_fn(move || {
        let size = be32(d, at)? as usize;
        let kind: [u8; 4] = d.get(at + 4..at + 8)?.try_into().ok()?;
        if size < 8 || at + size > end {
            return None;
        }
        let atom = (kind, at + 8, at + size);
        at += size;
        Some(atom)
    })
}

fn find_atom(d: &[u8], start: usize, end: usize, kind: &[u8; 4]) -> Option<(usize, usize)> {
    atoms(d, start, end).find(|a| &a.0 == kind).map(|a| (a.1, a.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_palette() {
        assert_eq!(standard(0), [255, 255, 255]);
        assert_eq!(standard(125), [102, 153, 0]);
        assert_eq!(standard(214), [0, 0, 51]);
        assert_eq!(standard(215), [0xEE, 0, 0]);
        assert_eq!(standard(234), [0, 0x11, 0]);
        assert_eq!(standard(254), [0x11, 0x11, 0x11]);
        assert_eq!(standard(255), [0, 0, 0]);
        // Every color once, so a standard color names one color number.
        let all: std::collections::HashSet<[u8; 3]> = (0..=255).map(standard).collect();
        assert_eq!(all.len(), 256);
    }

    #[test]
    fn a_movies_own_colors() {
        // A movie with no color table of its own (the Tour's, in H.264).
        assert!(remap_for(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/model/tour_movie/Card Trick.mp4"))).is_none());
        // HyperCard 2.4's Simple Movie: its black Q is standard color 125.
        let simple = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/apple/hypercard_241_stacks/HyperCard Stacks/Simple Movie"));
        if simple.exists() {
            let map = remap_for(simple).expect("a color table");
            assert_eq!(map.get(&[102, 153, 0]), Some(&[0, 0, 0]));
            let mut px = [102, 153, 0, 255, 1, 2, 3, 255];
            remap(&mut px, &map);
            assert_eq!(px, [0, 0, 0, 255, 1, 2, 3, 255]);
        }
    }
}
