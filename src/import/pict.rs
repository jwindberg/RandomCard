//! QuickDraw pictures ('PICT'), as far as their bitmaps go: the bit images
//! and pixel maps that HyperCard palettes, Picture windows and pasted art are
//! made of. Drawing opcodes (lines, shapes, text) are skipped; `drawn_only`
//! in the result says whether any were.

/// A decoded picture: RGBA pixels, row by row, the size of its frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Pict {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<[u8; 4]>,
    /// The frame's top left, in the picture's own coordinates.
    pub left: i32,
    pub top: i32,
    /// False if the picture had drawing opcodes RandomCard doesn't draw.
    pub complete: bool,
}

impl Pict {
    /// The picture in black and white, as HyperCard's paint layers hold it:
    /// dark pixels black, the rest white.
    pub fn to_picture(&self) -> crate::model::Picture {
        use crate::model::Pixel;
        let mut p = crate::model::Picture::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let [r, g, b, _] = self.rgba[(y * self.width + x) as usize];
                let luma = 299 * r as u32 + 587 * g as u32 + 114 * b as u32;
                p.set(x as i32, y as i32, if luma < 128_000 { Pixel::BLACK } else { Pixel::WHITE });
            }
        }
        p
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.at)?;
        self.at += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let v = u16::from_be_bytes(self.b.get(self.at..self.at + 2)?.try_into().ok()?);
        self.at += 2;
        Some(v)
    }
    fn i16(&mut self) -> Option<i32> {
        self.u16().map(|v| v as i16 as i32)
    }
    fn u32(&mut self) -> Option<u32> {
        let v = u32::from_be_bytes(self.b.get(self.at..self.at + 4)?.try_into().ok()?);
        self.at += 4;
        Some(v)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        if self.at + n > self.b.len() {
            return None;
        }
        self.at += n;
        Some(())
    }
    fn bytes(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }
    /// top, left, bottom, right
    fn rect(&mut self) -> Option<(i32, i32, i32, i32)> {
        Some((self.i16()?, self.i16()?, self.i16()?, self.i16()?))
    }
    /// A region or polygon: its size word counts itself.
    fn skip_sized(&mut self) -> Option<()> {
        let n = self.u16()? as usize;
        self.skip(n.saturating_sub(2))
    }
}

/// Unpack one PackBits run-length row into `out`.
fn unpack_bits(src: &[u8], out: &mut Vec<u8>, want: usize) {
    let mut i = 0;
    while i < src.len() && out.len() < want {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            let len = n as usize + 1;
            out.extend_from_slice(&src[i..(i + len).min(src.len())]);
            i += len;
        } else if n != -128 {
            let len = (1 - n as i32) as usize;
            if let Some(&v) = src.get(i) {
                out.extend(std::iter::repeat_n(v, len));
            }
            i += 1;
        }
    }
    out.resize(want, 0);
}

/// PackBits with 16-bit units (16-bit direct pixels, pack type 3).
fn unpack_words(src: &[u8], out: &mut Vec<u8>, want: usize) {
    let mut i = 0;
    while i < src.len() && out.len() < want {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            let len = (n as usize + 1) * 2;
            out.extend_from_slice(&src[i..(i + len).min(src.len())]);
            i += len;
        } else if n != -128 {
            let len = (1 - n as i32) as usize;
            if let Some(w) = src.get(i..i + 2) {
                for _ in 0..len {
                    out.extend_from_slice(w);
                }
            }
            i += 2;
        }
    }
    out.resize(want, 0);
}

/// Read `rows` rows of `row_bytes`, packed or not.
fn read_rows(r: &mut Reader, row_bytes: usize, rows: usize, packed: bool, words: bool) -> Option<Vec<u8>> {
    if !packed {
        return Some(r.bytes(row_bytes * rows)?.to_vec());
    }
    let mut data = Vec::with_capacity(row_bytes * rows);
    for _ in 0..rows {
        let n = if row_bytes > 250 { r.u16()? as usize } else { r.u8()? as usize };
        let src = r.bytes(n)?;
        let mut row = Vec::with_capacity(row_bytes);
        if words {
            unpack_words(src, &mut row, row_bytes);
        } else {
            unpack_bits(src, &mut row, row_bytes);
        }
        data.extend_from_slice(&row);
    }
    Some(data)
}

struct Canvas {
    pict: Pict,
}

impl Canvas {
    /// Copy `src` (w×h, with src rect `s` in its bounds `b`) to dest rect `d`.
    fn blit(&mut self, img: &[[u8; 4]], b: (i32, i32, i32, i32), s: (i32, i32, i32, i32), d: (i32, i32, i32, i32), mode: u16) {
        let bw = b.3 - b.1;
        let (sw, sh) = (s.3 - s.1, s.2 - s.0);
        let (dw, dh) = (d.3 - d.1, d.2 - d.0);
        if sw <= 0 || sh <= 0 || dw <= 0 || dh <= 0 {
            return;
        }
        let p = &mut self.pict;
        for dy in 0..dh {
            let y = d.0 + dy - p.top;
            if y < 0 || y >= p.height as i32 {
                continue;
            }
            let sy = s.0 + dy * sh / dh - b.0;
            for dx in 0..dw {
                let x = d.1 + dx - p.left;
                if x < 0 || x >= p.width as i32 {
                    continue;
                }
                let sx = s.1 + dx * sw / dw - b.1;
                let Some(&px) = img.get((sy * bw + sx) as usize) else { continue };
                // srcOr and transparent leave white source pixels unpainted.
                if ((mode & 0x3f) == 1 || (mode & 0x3f) == 36) && px == [255, 255, 255, 255] {
                    continue;
                }
                p.rgba[(y * p.width as i32 + x) as usize] = px;
            }
        }
    }
}

/// A bit image or pixel map opcode (0x90, 0x91, 0x98, 0x99, 0x9A, 0x9B).
fn bits(r: &mut Reader, canvas: &mut Canvas, op: u16) -> Option<()> {
    let direct = op == 0x9a || op == 0x9b;
    let region = op & 1 == 1;
    if direct {
        r.skip(4)?; // baseAddr
    }
    let rb = r.u16()?;
    let pixmap = direct || rb & 0x8000 != 0;
    let row_bytes = (rb & 0x3fff) as usize;
    let bounds = r.rect()?;
    let (w, h) = ((bounds.3 - bounds.1).max(0) as usize, (bounds.2 - bounds.0).max(0) as usize);
    let mut pixel_size = 1;
    let mut pack_type = 0;
    let mut cmp_count = 1;
    let mut clut: Vec<[u8; 4]> = vec![[255, 255, 255, 255], [0, 0, 0, 255]];
    if pixmap {
        r.skip(2)?; // version
        pack_type = r.u16()?;
        r.skip(4 + 4 + 4 + 2)?; // packSize, hRes, vRes, pixelType
        pixel_size = r.u16()?;
        cmp_count = r.u16()?;
        r.skip(2 + 4 + 4 + 4)?; // cmpSize, planeBytes, pmTable, pmReserved
        if !direct {
            r.skip(4)?; // ctSeed
            let flags = r.u16()?;
            let n = r.u16()? as usize + 1;
            clut = vec![[0, 0, 0, 255]; 1 << pixel_size.min(8)];
            for i in 0..n {
                let v = r.u16()? as usize;
                let (red, green, blue) = (r.u16()?, r.u16()?, r.u16()?);
                // Device color tables number entries by position.
                let at = if flags & 0x8000 != 0 { i } else { v };
                if let Some(c) = clut.get_mut(at) {
                    *c = [(red >> 8) as u8, (green >> 8) as u8, (blue >> 8) as u8, 255];
                }
            }
        }
    }
    let src = r.rect()?;
    let dst = r.rect()?;
    let mode = r.u16()?;
    if region {
        r.skip_sized()?;
    }
    let mut img = vec![[255, 255, 255, 255]; w * h];
    if direct && pixel_size == 32 {
        // Pack type 4 stores each row's components in planes; 1 and 2 don't pack.
        let planes = if pack_type == 2 { 3 } else { cmp_count.max(3) as usize };
        let stored = if pack_type == 4 { w * planes } else if pack_type == 2 { w * 3 } else { row_bytes };
        let data = read_rows(r, stored, h, pack_type == 4 || (pack_type == 0 && row_bytes >= 8), false)?;
        for y in 0..h {
            let row = &data[y * stored..(y + 1) * stored];
            for x in 0..w {
                let c = match pack_type {
                    4 => {
                        let o = planes - 3;
                        [row[o * w + x], row[(o + 1) * w + x], row[(o + 2) * w + x], 255]
                    }
                    2 => [row[x * 3], row[x * 3 + 1], row[x * 3 + 2], 255],
                    _ => [row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3], 255],
                };
                img[y * w + x] = c;
            }
        }
    } else if direct && pixel_size == 16 {
        let data = read_rows(r, row_bytes, h, row_bytes >= 8 && pack_type != 1, true)?;
        for y in 0..h {
            for x in 0..w {
                let at = y * row_bytes + x * 2;
                let v = u16::from_be_bytes([data[at], data[at + 1]]);
                let five = |s: u16| (((v >> s) & 31) * 255 / 31) as u8;
                img[y * w + x] = [five(10), five(5), five(0), 255];
            }
        }
    } else if !direct {
        let data = read_rows(r, row_bytes, h, row_bytes >= 8, false)?;
        let bits = pixel_size.clamp(1, 8) as usize;
        for y in 0..h {
            for x in 0..w {
                let bit = x * bits;
                let byte = data.get(y * row_bytes + bit / 8).copied().unwrap_or(0);
                let v = (byte >> (8 - bits - bit % 8)) & ((1u16 << bits) - 1) as u8;
                img[y * w + x] = clut.get(v as usize).copied().unwrap_or([0, 0, 0, 255]);
            }
        }
    } else {
        return None;
    }
    canvas.blit(&img, bounds, src, dst, mode);
    Some(())
}

/// Decode a picture's bitmaps. None if it isn't a picture.
pub fn decode(data: &[u8]) -> Option<Pict> {
    let mut r = Reader { b: data, at: 2 };
    let (top, left, bottom, right) = r.rect()?;
    let (width, height) = ((right - left).max(0) as u32, (bottom - top).max(0) as u32);
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return None;
    }
    let v2 = data.get(10..12) == Some(&[0x00, 0x11]);
    if !v2 && data.get(10..12) != Some(&[0x11, 0x01]) {
        return None;
    }
    let mut canvas = Canvas {
        pict: Pict { width, height, rgba: vec![[255, 255, 255, 255]; (width * height) as usize], left, top, complete: true },
    };
    loop {
        if v2 && r.at % 2 == 1 {
            r.skip(1)?;
        }
        let Some(op) = (if v2 { r.u16() } else { r.u8().map(u16::from) }) else { break };
        let ok = match op {
            0xff | 0xffff => break,
            0x00 | 0x1c | 0x1e | 0x17..=0x19 | 0x38..=0x3f | 0x48..=0x4f | 0x58..=0x5f | 0x78..=0x7f | 0x88..=0x8f => {
                if op >= 0x38 {
                    canvas.pict.complete = false;
                }
                Some(())
            }
            0x11 => r.skip(if v2 { 2 } else { 1 }),
            0x01 => r.skip_sized(),
            0x02 | 0x09 | 0x0a | 0x10 => r.skip(8),
            0x03 | 0x05 | 0x08 | 0x0d | 0x15 | 0x16 | 0x23 => r.skip(2),
            0x04 => r.skip(1),
            0x06 | 0x07 | 0x0b | 0x0c | 0x0e | 0x0f | 0x21 => r.skip(4),
            0x12..=0x14 => return finish(canvas, false), // pixel patterns: rare; stop here
            0x1a | 0x1b | 0x1d | 0x1f | 0x22 => r.skip(6),
            0x20 | 0x30..=0x37 | 0x40..=0x47 | 0x50..=0x57 => {
                canvas.pict.complete = false;
                r.skip(8)
            }
            0x28 => {
                canvas.pict.complete = false;
                r.skip(4).and_then(|_| r.u8()).and_then(|n| r.skip(n as usize))
            }
            0x29 | 0x2a => {
                canvas.pict.complete = false;
                r.skip(1).and_then(|_| r.u8()).and_then(|n| r.skip(n as usize))
            }
            0x2b => {
                canvas.pict.complete = false;
                r.skip(2).and_then(|_| r.u8()).and_then(|n| r.skip(n as usize))
            }
            0x24..=0x27 | 0x2c..=0x2f | 0x92..=0x97 | 0x9c..=0x9f | 0xa2..=0xaf => {
                r.u16().and_then(|n| r.skip(n as usize))
            }
            0x60..=0x67 => {
                canvas.pict.complete = false;
                r.skip(12)
            }
            0x68..=0x6f => {
                canvas.pict.complete = false;
                r.skip(4)
            }
            0x70..=0x77 | 0x80..=0x87 => {
                canvas.pict.complete = false;
                r.skip_sized()
            }
            0x90 | 0x91 | 0x98 | 0x99 | 0x9a | 0x9b => bits(&mut r, &mut canvas, op),
            0xa0 => r.skip(2),
            0xa1 => r.skip(2).and_then(|_| r.u16()).and_then(|n| r.skip(n as usize)),
            0xb0..=0xcf => Some(()),
            0xd0..=0xfe => r.u32().and_then(|n| r.skip(n as usize)),
            0x0c00 => r.skip(24),
            0x0100..=0x7fff => r.skip((op >> 8) as usize * 2),
            0x8000..=0x80ff => Some(()),
            0x8100..=0xfffe => r.u32().and_then(|n| r.skip(n as usize)),
        };
        if ok.is_none() {
            return finish(canvas, false);
        }
    }
    finish(canvas, true)
}

fn finish(mut canvas: Canvas, ended: bool) -> Option<Pict> {
    if !ended {
        canvas.pict.complete = false;
    }
    Some(canvas.pict)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version 1 picture with one PackBitsRect: 16×2, rows "F0 0F" and "0F F0".
    fn v1_bits() -> Vec<u8> {
        let mut p = vec![0, 0, 0, 10, 0, 20, 0, 12, 0, 36, 0x11, 0x01, 0xa0, 0, 0x82, 0x90];
        p.extend([0, 2]); // rowBytes 2: under 8, so not packed
        p.extend([0, 10, 0, 20, 0, 12, 0, 36]); // bounds
        p.extend([0, 10, 0, 20, 0, 12, 0, 36]); // src
        p.extend([0, 10, 0, 20, 0, 12, 0, 36]); // dst
        p.extend([0, 0]); // srcCopy
        p.extend([0xf0, 0x0f, 0x0f, 0xf0]);
        p.push(0xff);
        p
    }

    #[test]
    fn version_1_bit_image() {
        let pict = decode(&v1_bits()).unwrap();
        assert_eq!((pict.width, pict.height, pict.left, pict.top), (16, 2, 20, 10));
        assert!(pict.complete);
        let black = |x: u32, y: u32| pict.rgba[(y * 16 + x) as usize] == [0, 0, 0, 255];
        assert!(black(0, 0) && black(3, 0) && !black(4, 0) && black(12, 0));
        assert!(!black(0, 1) && black(4, 1) && black(8, 1) && !black(12, 1));
    }

    #[test]
    fn pack_bits() {
        let mut out = Vec::new();
        unpack_bits(&[0x02, 1, 2, 3, 0xfd, 9], &mut out, 7);
        assert_eq!(out, [1, 2, 3, 9, 9, 9, 9]);
    }

    #[test]
    fn version_2_pixel_map() {
        // 2×1, 8 bits a pixel, with a two-color table: red, then blue.
        let mut p = vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 2];
        p.extend([0x00, 0x11, 0x02, 0xff, 0x0c, 0x00]);
        p.extend([0; 24]);
        p.extend([0x00, 0x98, 0x80, 0x02]); // PackBitsRect, a pixmap with rowBytes 2
        p.extend([0, 0, 0, 0, 0, 1, 0, 2]); // bounds
        p.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // version..pixelType
        p.extend([0, 8, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // pixelSize..pmReserved
        p.extend([0, 0, 0, 0, 0, 0, 0, 1]); // ctSeed, flags, ctSize
        p.extend([0, 0, 0xff, 0xff, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0xff, 0xff]);
        p.extend([0, 0, 0, 0, 0, 1, 0, 2, 0, 0, 0, 0, 0, 1, 0, 2, 0, 0]); // src, dst, mode
        p.extend([0, 1]); // the pixels: rowBytes under 8, so not packed
        p.extend([0x00, 0xff]); // end
        let pict = decode(&p).unwrap();
        assert_eq!(pict.rgba, vec![[255, 0, 0, 255], [0, 0, 255, 255]]);
        assert!(pict.complete);
    }
}
