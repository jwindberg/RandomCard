//! Classic Mac resource forks: finding them on file systems that have no
//! forks, parsing the resource map, and decoding HyperCard's button icons.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::mac_roman;
use crate::model::{Icon, Picture, Pixel, Sound};

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

/// All the 'snd ' resources in a resource fork that can be decoded.
pub fn sounds(fork: &[u8]) -> BTreeMap<i32, Sound> {
    parse(fork)
        .into_iter()
        .filter(|r| &r.kind == b"snd ")
        .filter_map(|r| Some((r.id as i32, decode_snd(r.data, &r.name)?)))
        .collect()
}

const SOUND_CMD: u16 = 80;
const BUFFER_CMD: u16 = 81;

/// Decode a 'snd ' resource (format 1 or 2) holding a sampled sound.
/// Standard, extended and IMA4-compressed sound headers are supported.
pub fn decode_snd(data: &[u8], name: &str) -> Option<Sound> {
    let format = be16(data, 0)?;
    let mut pos = match format {
        1 => {
            let mods = be16(data, 2)? as usize;
            4 + mods * 6
        }
        2 => 4,
        _ => return None,
    };
    let commands = be16(data, pos)? as usize;
    pos += 2;
    // The header comes from the first sound or buffer command with a data offset.
    let header = (0..commands).find_map(|i| {
        let c = pos + i * 8;
        let cmd = be16(data, c)?;
        let kind = cmd & 0x7fff;
        (cmd & 0x8000 != 0 && (kind == SOUND_CMD || kind == BUFFER_CMD)).then(|| be32(data, c + 4)).flatten()
    })? as usize;
    decode_sound_header(data, header, name)
}

fn decode_sound_header(data: &[u8], h: usize, name: &str) -> Option<Sound> {
    let rate = be32(data, h + 8)? as f32 / 65536.0;
    let loop_start = be32(data, h + 12)?;
    let loop_end = be32(data, h + 16)?;
    let encode = *data.get(h + 20)?;
    let base_note = match *data.get(h + 21)? {
        0 => 60,
        n => n,
    };
    let samples = match encode {
        // Standard: 8-bit unsigned mono; the length field counts bytes.
        0x00 => {
            let len = be32(data, h + 4)? as usize;
            let bytes = data.get(h + 22..)?;
            bytes[..len.min(bytes.len())].iter().map(|b| ((*b as i16) - 128) << 8).collect()
        }
        // Extended: 8- or 16-bit, any number of channels, interleaved.
        0xff => {
            let channels = be32(data, h + 4)?.max(1) as usize;
            let frames = be32(data, h + 22)? as usize;
            let bits = be16(data, h + 48)?;
            let body = data.get(h + 64..)?;
            let width = if bits == 16 { 2 } else { 1 };
            let frames = frames.min(body.len() / (width * channels));
            (0..frames)
                .map(|f| {
                    let sum: i32 = (0..channels)
                        .map(|c| {
                            let i = (f * channels + c) * width;
                            if width == 2 {
                                i16::from_be_bytes([body[i], body[i + 1]]) as i32
                            } else {
                                ((body[i] as i32) - 128) << 8
                            }
                        })
                        .sum();
                    (sum / channels as i32) as i16
                })
                .collect()
        }
        // Compressed: only IMA4 is decoded (MACE isn't).
        0xfe => {
            let channels = be32(data, h + 4)?.max(1) as usize;
            if data.get(h + 40..h + 44)? != b"ima4" {
                return None;
            }
            decode_ima4(data.get(h + 64..)?, channels)
        }
        _ => return None,
    };
    if samples.is_empty() || !(1000.0..=200_000.0).contains(&rate) {
        return None;
    }
    Some(Sound { name: name.to_string(), rate, base_note, loop_start, loop_end, samples })
}

const IMA_STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107,
    118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876,
    963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358,
    5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086,
    29794, 32767,
];
const IMA_INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// Apple IMA4: 34-byte packets of 64 samples, one packet per channel in turn.
fn decode_ima4(body: &[u8], channels: usize) -> Vec<i16> {
    let packets = body.len() / 34 / channels;
    let mut per_channel: Vec<Vec<i32>> = vec![Vec::with_capacity(packets * 64); channels];
    for p in 0..packets {
        for (c, out) in per_channel.iter_mut().enumerate() {
            let packet = &body[(p * channels + c) * 34..][..34];
            let preamble = u16::from_be_bytes([packet[0], packet[1]]);
            let mut predictor = (preamble & 0xff80) as i16 as i32;
            let mut index = (preamble & 0x7f).min(88) as i32;
            for byte in &packet[2..] {
                for nibble in [byte & 0x0f, byte >> 4] {
                    let step = IMA_STEPS[index as usize];
                    let mut diff = step >> 3;
                    if nibble & 1 != 0 {
                        diff += step >> 2;
                    }
                    if nibble & 2 != 0 {
                        diff += step >> 1;
                    }
                    if nibble & 4 != 0 {
                        diff += step;
                    }
                    if nibble & 8 != 0 {
                        diff = -diff;
                    }
                    predictor = (predictor + diff).clamp(-32768, 32767);
                    index = (index + IMA_INDEX[nibble as usize]).clamp(0, 88);
                    out.push(predictor);
                }
            }
        }
    }
    (0..packets * 64).map(|i| (per_channel.iter().map(|ch| ch[i]).sum::<i32>() / channels as i32) as i16).collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A format 1 'snd ' with one bufferCmd pointing at a standard sound header.
    pub fn standard_snd(samples: &[u8], rate: u32, base: u8, loop_points: (u32, u32)) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&1u16.to_be_bytes()); // format 1
        d.extend_from_slice(&1u16.to_be_bytes()); // one modifier: sampled synth
        d.extend_from_slice(&5u16.to_be_bytes());
        d.extend_from_slice(&0u32.to_be_bytes());
        d.extend_from_slice(&1u16.to_be_bytes()); // one command
        d.extend_from_slice(&(0x8000 | BUFFER_CMD).to_be_bytes());
        d.extend_from_slice(&0u16.to_be_bytes());
        let header_at = d.len() as u32 + 4;
        d.extend_from_slice(&header_at.to_be_bytes());
        d.extend_from_slice(&0u32.to_be_bytes()); // samplePtr
        d.extend_from_slice(&(samples.len() as u32).to_be_bytes());
        d.extend_from_slice(&(rate << 16).to_be_bytes());
        d.extend_from_slice(&loop_points.0.to_be_bytes());
        d.extend_from_slice(&loop_points.1.to_be_bytes());
        d.push(0); // standard encoding
        d.push(base);
        d.extend_from_slice(samples);
        d
    }

    #[test]
    fn standard_sound_header() {
        let snd = standard_snd(&[0x80, 0xff, 0x00, 0x80], 22050, 62, (1, 3));
        let s = decode_snd(&snd, "Boing").unwrap();
        assert_eq!(s.name, "Boing");
        assert_eq!(s.rate, 22050.0);
        assert_eq!(s.base_note, 62);
        assert_eq!((s.loop_start, s.loop_end), (1, 3));
        assert_eq!(s.samples, vec![0, 127 << 8, -128 << 8, 0]);
    }

    #[test]
    fn sounds_in_a_fork() {
        let snd = standard_snd(&[0x80; 100], 11025, 60, (0, 0));
        let fork = make_fork(&[(b"ICON", 1, "", vec![0; 128]), (b"snd ", 9, "Ding", snd)]);
        let sounds = sounds(&fork);
        assert_eq!(sounds[&9].name, "Ding");
        assert_eq!(sounds[&9].samples.len(), 100);
        assert_eq!(icons(&fork).len(), 1);
    }

    #[test]
    fn extended_16_bit_stereo() {
        let mut d = vec![0, 2, 0, 0, 0, 1];
        d.extend_from_slice(&(0x8000 | SOUND_CMD).to_be_bytes());
        d.extend_from_slice(&[0, 0]);
        d.extend_from_slice(&14u32.to_be_bytes());
        let mut h = vec![0u8; 64];
        h[4..8].copy_from_slice(&2u32.to_be_bytes());
        h[8..12].copy_from_slice(&(44100u32 << 16).to_be_bytes());
        h[20] = 0xff;
        h[21] = 60;
        h[22..26].copy_from_slice(&2u32.to_be_bytes());
        h[48..50].copy_from_slice(&16u16.to_be_bytes());
        d.extend_from_slice(&h);
        for v in [1000i16, 3000, -2000, -4000] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        let s = decode_snd(&d, "x").unwrap();
        assert_eq!(s.samples, vec![2000, -3000]);
        assert_eq!(s.rate, 44100.0);
    }

    #[test]
    fn ima4_packet() {
        // A packet starting at 0 whose nibbles all mean "step up" grows steadily.
        let mut body = vec![0u8, 0];
        body.extend(std::iter::repeat_n(0x44u8, 32));
        let out = decode_ima4(&body, 1);
        assert_eq!(out.len(), 64);
        assert!(out.windows(2).all(|w| w[1] >= w[0]));
        assert!(out[1] > out[0] && out[63] == 32767);
    }

    /// Build a resource fork holding the given (type, id, name, data) resources.
    pub fn make_fork(entries: &[(&[u8; 4], i16, &str, Vec<u8>)]) -> Vec<u8> {
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for (_, _, _, d) in entries {
            offsets.push(data.len() as u32);
            data.extend_from_slice(&(d.len() as u32).to_be_bytes());
            data.extend_from_slice(d);
        }
        // Map: 24-byte header copy, then type list offset (28) and name list offset.
        let n = entries.len();
        let type_list = 28usize;
        let refs_at = 2 + n * 8; // reference lists start after the type entries
        let names_at = type_list + refs_at + n * 12;
        let mut map = vec![0u8; 24];
        map.extend_from_slice(&(type_list as u16).to_be_bytes());
        map.extend_from_slice(&(names_at as u16).to_be_bytes());
        map.extend_from_slice(&((n as u16).wrapping_sub(1)).to_be_bytes());
        for (i, (kind, ..)) in entries.iter().enumerate() {
            map.extend_from_slice(*kind);
            map.extend_from_slice(&0u16.to_be_bytes());
            map.extend_from_slice(&((refs_at + i * 12) as u16).to_be_bytes());
        }
        let mut names = Vec::new();
        for (i, (_, id, name, _)) in entries.iter().enumerate() {
            map.extend_from_slice(&id.to_be_bytes());
            map.extend_from_slice(&(names.len() as u16).to_be_bytes());
            map.extend_from_slice(&offsets[i].to_be_bytes());
            map.extend_from_slice(&0u32.to_be_bytes());
            names.push(name.len() as u8);
            names.extend_from_slice(name.as_bytes());
        }
        map.extend_from_slice(&names);
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
        let fork = make_fork(&[(b"ICON", 128, "Box", bits.to_vec())]);
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
