//! Classic Mac resource forks: finding them on file systems that have no
//! forks, parsing the resource map, and decoding HyperCard's button icons.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::mac_roman;
use crate::model::{Icon, Picture, Pixel};

const APPLE_SINGLE: u32 = 0x0005_1600;
const APPLE_DOUBLE: u32 = 0x0005_1607;
const MACBINARY_HEADER: usize = 128;

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

/// An entry (1 = data fork, 2 = resource fork) of an AppleSingle or AppleDouble file.
pub fn apple_entry(bytes: &[u8], want: u32) -> Option<&[u8]> {
    let magic = be32(bytes, 0)?;
    if magic != APPLE_SINGLE && magic != APPLE_DOUBLE {
        return None;
    }
    let count = be16(bytes, 24)? as usize;
    for i in 0..count {
        let e = 26 + i * 12;
        if be32(bytes, e)? == want {
            let (off, len) = (be32(bytes, e + 4)? as usize, be32(bytes, e + 8)? as usize);
            return bytes.get(off..off + len);
        }
    }
    None
}

/// The resource fork stored after the data fork in a MacBinary file.
pub fn macbinary_resource_fork(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() <= MACBINARY_HEADER || bytes[0] != 0 {
        return None;
    }
    let data_len = be32(bytes, 83)? as usize;
    let rsrc_len = be32(bytes, 87)? as usize;
    let start = MACBINARY_HEADER + data_len.div_ceil(128) * 128;
    bytes.get(start..start + rsrc_len).filter(|r| !r.is_empty())
}

/// Look for a stack's resource fork in the places other systems keep them.
pub fn find_resource_fork(path: &Path, bytes: &[u8]) -> Option<Vec<u8>> {
    if let Some(r) = macbinary_resource_fork(bytes).filter(|_| super::is_macbinary(bytes)) {
        return Some(r.to_vec());
    }
    if let Some(r) = apple_entry(bytes, 2) {
        return Some(r.to_vec());
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path.file_name()?.to_string_lossy().to_string();
    let mut candidates: Vec<PathBuf> = vec![
        // macOS: the fork itself.
        path.join("..namedfork").join("rsrc"),
        // A raw fork beside the file: "Home.rsrc" or "Home.stack.rsrc".
        PathBuf::from(format!("{}.rsrc", path.display())),
        // AppleDouble, as written by macOS to non-Mac volumes and zip files.
        dir.join(format!("._{name}")),
        dir.join("__MACOSX").join(format!("._{name}")),
        // Netatalk, and the SheepShaver/Basilisk II shared folder.
        dir.join(".AppleDouble").join(&name),
        dir.join(".rsrc").join(&name),
    ];
    if path.extension().is_some() {
        candidates.insert(2, path.with_extension("rsrc"));
    }
    for c in candidates {
        let Ok(data) = std::fs::read(&c) else { continue };
        if data.is_empty() {
            continue;
        }
        if let Some(r) = apple_entry(&data, 2) {
            return Some(r.to_vec());
        }
        return Some(data);
    }
    None
}

/// Resource forks further along HyperCard's resource chain: a Home stack or
/// the HyperCard application next to the stack, then any resource files the
/// user keeps in `~/.config/randomcard/resources/` (for example HyperCard's
/// own fork, which holds the standard icons).
pub fn chain_forks(path: &Path) -> Vec<Vec<u8>> {
    let mut forks = Vec::new();
    let dir = path.parent().unwrap_or(Path::new("."));
    for name in ["Home", "HyperCard", "HyperCard Player"] {
        let sibling = dir.join(name);
        if sibling == path {
            continue;
        }
        let bytes = std::fs::read(&sibling).unwrap_or_default();
        if let Some(f) = find_resource_fork(&sibling, &bytes) {
            forks.push(f);
        }
    }
    if let Some(config) = config_resources_dir()
        && let Ok(entries) = std::fs::read_dir(config)
    {
        let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
        files.sort();
        for f in files {
            let Ok(bytes) = std::fs::read(&f) else { continue };
            // A bare resource fork, or a file carrying one.
            let fork = apple_entry(&bytes, 2)
                .or_else(|| macbinary_resource_fork(&bytes))
                .map(<[u8]>::to_vec)
                .unwrap_or(bytes);
            forks.push(fork);
        }
    }
    forks
}

fn config_resources_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("randomcard").join("resources"))
}

pub struct Resource<'a> {
    pub kind: [u8; 4],
    pub id: i16,
    pub name: String,
    pub data: &'a [u8],
}

/// Parse a resource fork's map. Damaged entries are skipped.
pub fn parse(fork: &[u8]) -> Vec<Resource<'_>> {
    parse_inner(fork).unwrap_or_default()
}

fn parse_inner(fork: &[u8]) -> Option<Vec<Resource<'_>>> {
    let data_off = be32(fork, 0)? as usize;
    let map_off = be32(fork, 4)? as usize;
    let map_len = be32(fork, 12)? as usize;
    let map = fork.get(map_off..map_off + map_len)?;
    let type_list = be16(map, 24)? as usize;
    let name_list = be16(map, 26)? as usize;
    let type_count = be16(map, type_list)?.wrapping_add(1) as usize;
    let mut out = Vec::new();
    for t in 0..type_count {
        let te = type_list + 2 + t * 8;
        let kind: [u8; 4] = map.get(te..te + 4)?.try_into().ok()?;
        let count = be16(map, te + 4)? as usize + 1;
        let refs = type_list + be16(map, te + 6)? as usize;
        for r in 0..count {
            let re = refs + r * 12;
            let Some(id) = be16(map, re).map(|v| v as i16) else { break };
            let name_off = be16(map, re + 2)? as i16;
            let offset = (be32(map, re + 4)? & 0x00ff_ffff) as usize;
            let name = if name_off < 0 {
                String::new()
            } else {
                let n = name_list + name_off as usize;
                let len = *map.get(n)? as usize;
                mac_roman(map.get(n + 1..n + 1 + len)?)
            };
            let at = data_off + offset;
            let Some(len) = be32(fork, at) else { continue };
            let Some(data) = fork.get(at + 4..at + 4 + len as usize) else { continue };
            out.push(Resource { kind, id, name, data });
        }
    }
    Some(out)
}

/// Decode a 32×32 'ICON'. HyperCard draws icons with a mask made by
/// flood-filling white from the edges: white enclosed by black stays opaque.
pub fn decode_icon(data: &[u8], name: &str) -> Option<Icon> {
    let bits = data.get(..128)?;
    let black = |x: i32, y: i32| bits[(y * 4 + x / 8) as usize] & (0x80 >> (x % 8)) != 0;
    let mut pic = Picture::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            pic.set(x, y, if black(x, y) { Pixel::Black } else { Pixel::White });
        }
    }
    let mut stack: Vec<(i32, i32)> = (0..32).flat_map(|i| [(i, 0), (i, 31), (0, i), (31, i)]).collect();
    while let Some((x, y)) = stack.pop() {
        if !(0..32).contains(&x) || !(0..32).contains(&y) || pic.get(x, y) != Pixel::White {
            continue;
        }
        pic.set(x, y, Pixel::Clear);
        stack.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
    }
    Some(Icon { name: name.to_string(), picture: pic })
}

/// All the 'ICON' resources in a resource fork.
pub fn icons(fork: &[u8]) -> BTreeMap<i32, Icon> {
    parse(fork)
        .into_iter()
        .filter(|r| &r.kind == b"ICON")
        .filter_map(|r| Some((r.id as i32, decode_icon(r.data, &r.name)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal resource fork holding one ICON.
    fn fork_with_icon(id: i16, name: &str, icon: &[u8; 128]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&128u32.to_be_bytes());
        data.extend_from_slice(icon);
        let mut map = vec![0u8; 24];
        map.extend_from_slice(&28u16.to_be_bytes()); // type list offset
        let name_list_pos = map.len();
        map.extend_from_slice(&0u16.to_be_bytes()); // name list offset, patched below
        map.extend_from_slice(&0u16.to_be_bytes()); // type count - 1
        map.extend_from_slice(b"ICON");
        map.extend_from_slice(&0u16.to_be_bytes()); // ref count - 1
        map.extend_from_slice(&10u16.to_be_bytes()); // ref list, from type list
        map.extend_from_slice(&id.to_be_bytes());
        map.extend_from_slice(&0u16.to_be_bytes()); // name offset
        map.extend_from_slice(&0u32.to_be_bytes()); // attrs + data offset
        map.extend_from_slice(&0u32.to_be_bytes()); // handle
        let names = map.len() as u16;
        map[name_list_pos..name_list_pos + 2].copy_from_slice(&names.to_be_bytes());
        map.push(name.len() as u8);
        map.extend_from_slice(name.as_bytes());
        let mut fork = Vec::new();
        let (data_off, map_off) = (256u32, 256 + data.len() as u32);
        for v in [data_off, map_off, data.len() as u32, map.len() as u32] {
            fork.extend_from_slice(&v.to_be_bytes());
        }
        fork.resize(256, 0);
        fork.extend_from_slice(&data);
        fork.extend_from_slice(&map);
        fork
    }

    #[test]
    fn reads_icons() {
        // A hollow square: the outside is transparent, the inside opaque white.
        let mut bits = [0u8; 128];
        for y in 8..24 {
            for x in 8..24 {
                if y == 8 || y == 23 || x == 8 || x == 23 {
                    bits[y * 4 + x / 8] |= 0x80 >> (x % 8);
                }
            }
        }
        let fork = fork_with_icon(128, "Box", &bits);
        let icons = icons(&fork);
        let icon = &icons[&128];
        assert_eq!(icon.name, "Box");
        assert_eq!(icon.picture.get(0, 0), Pixel::Clear);
        assert_eq!(icon.picture.get(8, 8), Pixel::Black);
        assert_eq!(icon.picture.get(15, 15), Pixel::White);
    }

    #[test]
    fn apple_double() {
        let mut ad = Vec::new();
        ad.extend_from_slice(&APPLE_DOUBLE.to_be_bytes());
        ad.extend_from_slice(&0x0002_0000u32.to_be_bytes());
        ad.extend_from_slice(&[0; 16]);
        ad.extend_from_slice(&1u16.to_be_bytes());
        for v in [2u32, 38, 3] {
            ad.extend_from_slice(&v.to_be_bytes());
        }
        ad.extend_from_slice(b"abc");
        assert_eq!(apple_entry(&ad, 2), Some(&b"abc"[..]));
        assert_eq!(apple_entry(&ad, 1), None);
    }
}
