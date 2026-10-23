//! Decoder for WOBA ("Wrath Of Bill Atkinson"), the run-length scheme
//! HyperCard 2.x uses to compress card and background pictures.
//!
//! Ported from the decoder in Pierre Lorenzi's HyperCardPreview
//! (BitmapLoading.swift, MIT License, Copyright (c) 2021 Pierre Lorenzi):
//! <https://github.com/PierreLorenzi/HyperCardPreview>

use crate::model::{Picture, Pixel};

/// A rectangle in QuickDraw's (top, left, bottom, right) order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QdRect {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
}

impl QdRect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn is_empty(&self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }
    /// Widen to 32-bit boundaries, as the compressed rows are.
    fn aligned32(&self) -> QdRect {
        QdRect { left: self.left.div_euclid(32) * 32, right: (self.right + 31).div_euclid(32) * 32, ..*self }
    }
}

/// One decoded 1-bit layer.
enum Layer {
    Clear,
    /// No data was stored: the layer is its rectangle, filled.
    Filled(QdRect),
    /// Rows of `rect32.width() / 8` bytes covering `rect32`.
    Bits { rect: QdRect, rect32: QdRect, rows: Vec<u8> },
}

impl Layer {
    fn get(&self, x: i32, y: i32) -> bool {
        match self {
            Layer::Clear => false,
            Layer::Filled(r) => x >= r.left && x < r.right && y >= r.top && y < r.bottom,
            Layer::Bits { rect, rect32, rows } => {
                if x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom {
                    return false;
                }
                let rw = (rect32.width() / 8) as usize;
                let bx = (x - rect32.left) as usize;
                let i = (y - rect32.top) as usize * rw + bx / 8;
                rows.get(i).is_some_and(|b| b & (0x80 >> (bx % 8)) != 0)
            }
        }
    }
}

fn layer(rect: QdRect, data: &[u8]) -> Layer {
    if rect.is_empty() {
        return Layer::Clear;
    }
    if data.is_empty() {
        return Layer::Filled(rect);
    }
    let rect32 = rect.aligned32();
    let rows = decode_rows(rect32, data);
    Layer::Bits { rect, rect32, rows }
}

/// Decode compressed rows covering the 32-bit-aligned `rect`.
fn decode_rows(rect: QdRect, data: &[u8]) -> Vec<u8> {
    let rw = (rect.width() / 8) as usize;
    let height = rect.height() as usize;
    let mut px = vec![0u8; rw * height];
    let mut off = 0usize;
    let (mut dx, mut dy) = (0usize, 0usize);
    let mut repeated: [u8; 8] = [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55];
    let mut y = 0usize;
    let next = |off: &mut usize| -> Option<u8> {
        let b = data.get(*off).copied();
        *off += 1;
        b
    };
    // Writes outside the buffer (from damaged data) are ignored.
    let put = |px: &mut Vec<u8>, i: usize, v: u8| {
        if let Some(p) = px.get_mut(i) {
            *p = v;
        }
    };
    let fill_row = |px: &mut Vec<u8>, row: usize, v: u8| {
        if let Some(r) = px.get_mut(row * rw..(row + 1) * rw) {
            r.fill(v);
        }
    };

    'rows: while y < height {
        let base = y * rw;
        let mut x = 0usize; // in bytes
        let mut repeat = 1usize;
        while x < rw {
            let Some(op) = next(&mut off) else { break 'rows };
            match op {
                0x00..=0x7f => {
                    // z zero bytes followed by d data bytes, repeated.
                    let z = (op & 0x0f) as usize;
                    let d = (op >> 4) as usize;
                    let total = z + d;
                    for i in 0..d {
                        let Some(v) = next(&mut off) else { break 'rows };
                        for r in 0..repeat {
                            if x + z + i + r * total < rw {
                                put(&mut px, base + x + z + i + r * total, v);
                            }
                        }
                    }
                    x += total * repeat;
                    repeat = 1;
                }
                0x80 => {
                    // One row of uncompressed data, repeated.
                    let Some(row) = data.get(off..off + rw) else { break 'rows };
                    for r in 0..repeat {
                        if let Some(dst) = px.get_mut((y + r) * rw..(y + r + 1) * rw) {
                            dst.copy_from_slice(row);
                        }
                    }
                    off += rw;
                    y += repeat;
                    continue 'rows;
                }
                0x81 => {
                    // White rows.
                    y += repeat;
                    continue 'rows;
                }
                0x82 => {
                    for r in 0..repeat {
                        fill_row(&mut px, y + r, 0xff);
                    }
                    y += repeat;
                    continue 'rows;
                }
                0x83 => {
                    // Rows of one repeated byte, remembered for 0x84.
                    let Some(v) = next(&mut off) else { break 'rows };
                    repeated[(rect.top as usize + y) % 8] = v;
                    for r in 0..repeat {
                        fill_row(&mut px, y + r, v);
                    }
                    y += repeat;
                    continue 'rows;
                }
                0x84 => {
                    for _ in 0..repeat {
                        fill_row(&mut px, y, repeated[(rect.top as usize + y) % 8]);
                        y += 1;
                    }
                    continue 'rows;
                }
                0x85..=0x87 => {
                    // Copy the row 1, 2 or 3 rows above.
                    let back = (op - 0x84) as usize;
                    for _ in 0..repeat {
                        if y >= back && y < height {
                            px.copy_within((y - back) * rw..(y - back + 1) * rw, y * rw);
                        }
                        y += 1;
                    }
                    continue 'rows;
                }
                0x88 => (dx, dy) = (16, 0),
                0x89 => (dx, dy) = (0, 0),
                0x8a => (dx, dy) = (0, 1),
                0x8b => (dx, dy) = (0, 2),
                0x8c => (dx, dy) = (1, 0),
                0x8d => (dx, dy) = (1, 1),
                0x8e => (dx, dy) = (2, 2),
                0x8f => (dx, dy) = (8, 0),
                0xa0..=0xbf => repeat = (op & 0x1f) as usize,
                0xc0..=0xdf => {
                    // (n * 8) bytes of data, repeated.
                    let d = ((op & 0x1f) as usize) * 8;
                    for i in 0..d {
                        let Some(v) = next(&mut off) else { break 'rows };
                        for r in 0..repeat {
                            if x + i + r * d < rw {
                                put(&mut px, base + x + i + r * d, v);
                            }
                        }
                    }
                    x += d * repeat;
                    repeat = 1;
                }
                0xe0..=0xff => {
                    // (n * 16) zero bytes, repeated.
                    x += ((op & 0x1f) as usize) * 16 * repeat;
                    repeat = 1;
                }
                // Anything else means the data is over.
                _ => break 'rows,
            }
        }
        // A row built from byte runs gets the current delta transforms.
        let row = &mut px[base..base + rw];
        if dx != 0 {
            // Each bit is XORed with the (already transformed) bit dx to its left.
            for bit in dx..rw * 8 {
                let src = bit - dx;
                if row[src / 8] & (0x80 >> (src % 8)) != 0 {
                    row[bit / 8] ^= 0x80 >> (bit % 8);
                }
            }
        }
        if dy != 0 && dy <= y {
            for i in 0..rw {
                px[base + i] ^= px[base + i - dy * rw];
            }
        }
        y += 1;
    }
    px
}

/// Decode a picture from its card, mask and image rectangles and compressed data.
/// Black where the image bit is set, white where only the mask is, else clear.
pub fn decode(card: QdRect, mask_rect: QdRect, mask: &[u8], image_rect: QdRect, image: &[u8]) -> Option<Picture> {
    if card.width() <= 0 || card.height() <= 0 || card.width() > 4096 || card.height() > 4096 {
        return None;
    }
    let image = layer(image_rect, image);
    let mask = layer(mask_rect, mask);
    let mut pic = Picture::new(card.width() as u32, card.height() as u32);
    for y in 0..card.height() {
        for x in 0..card.width() {
            let (cx, cy) = (card.left + x, card.top + y);
            if image.get(cx, cy) {
                pic.set(x, y, Pixel::Black);
            } else if mask.get(cx, cy) {
                pic.set(x, y, Pixel::White);
            }
        }
    }
    Some(pic)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: QdRect = QdRect { top: 0, left: 0, bottom: 4, right: 32 };

    fn black_row(pic: &Picture, y: i32) -> Vec<bool> {
        (0..32).map(|x| pic.get(x, y) == Pixel::Black).collect()
    }

    #[test]
    fn literal_rows() {
        // 0x80: one row of literal bytes; 0x82: a black row; 0x81: a white row.
        let data = [0x80, 0xF0, 0x00, 0x00, 0x0F, 0x82, 0x81, 0x80, 0x01, 0x02, 0x03, 0x04];
        let pic = decode(CARD, QdRect::default(), &[], CARD, &data).unwrap();
        assert_eq!(pic.get(0, 0), Pixel::Black);
        assert_eq!(pic.get(4, 0), Pixel::Clear);
        assert_eq!(pic.get(31, 0), Pixel::Black);
        assert!(black_row(&pic, 1).iter().all(|b| *b));
        assert!(black_row(&pic, 2).iter().all(|b| !*b));
        assert_eq!(pic.get(7, 3), Pixel::Black);
    }

    #[test]
    fn zeros_and_data_with_repeat() {
        // 0xA2 repeats the next op twice; 0x11 = one zero byte then one data byte.
        let data = [0xA2, 0x11, 0xFF];
        let pic = decode(CARD, QdRect::default(), &[], CARD, &data).unwrap();
        let row = black_row(&pic, 0);
        assert!(!row[0] && row[8] && !row[16] && row[24]);
    }

    #[test]
    fn vertical_delta() {
        // dy = 1: a row of zero bytes repeats the row above.
        let data = [0x8A, 0x40, 0xF0, 0x0F, 0xF0, 0x0F, 0x40, 0, 0, 0, 0, 0x40, 0, 0, 0, 0];
        let pic = decode(CARD, QdRect::default(), &[], CARD, &data).unwrap();
        assert_eq!(black_row(&pic, 0), black_row(&pic, 1));
        assert_eq!(black_row(&pic, 1), black_row(&pic, 2));
        assert!(black_row(&pic, 0)[0]);
    }

    #[test]
    fn mask_without_data_is_a_filled_rect() {
        let mask = QdRect { top: 1, left: 2, bottom: 3, right: 6 };
        let pic = decode(CARD, mask, &[], QdRect::default(), &[]).unwrap();
        assert_eq!(pic.get(2, 1), Pixel::White);
        assert_eq!(pic.get(5, 2), Pixel::White);
        assert_eq!(pic.get(6, 2), Pixel::Clear);
        assert_eq!(pic.get(0, 0), Pixel::Clear);
    }
}
