//! Writes RandomCard's icon as PNGs for packaging: icon_<size>.png for 16
//! to 1024 pixels.
//!
//! Usage: cargo run --example make_icons -- <output folder>

#[path = "../src/icon.rs"]
mod icon;

/// A minimal PNG: one IHDR, one zlib-compressed IDAT of RGBA rows, IEND.
fn png(size: u32, rgba: &[u8]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    }
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { 0xedb8_8320 ^ (crc >> 1) } else { crc >> 1 };
            }
        }
        !crc
    }
    let mut header = Vec::new();
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    let mut rows = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks(size as usize * 4) {
        rows.push(0); // no filter
        rows.extend_from_slice(row);
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &miniz_oxide::deflate::compress_to_vec_zlib(&rows, 9));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn main() {
    let folder = std::env::args().nth(1).expect("an output folder");
    std::fs::create_dir_all(&folder).expect("creating the folder");
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        let path = format!("{folder}/icon_{size}.png");
        std::fs::write(&path, png(size, &icon::rgba(size as usize))).expect("writing an icon");
    }
}
