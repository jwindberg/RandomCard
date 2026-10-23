//! Paint layers. Like HyperCard, pictures are black and white, but card and
//! background layers also need "no paint here" so lower layers show through.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Pixel {
    Clear = 0,
    White = 1,
    Black = 2,
}

impl Pixel {
    fn from_u8(v: u8) -> Pixel {
        match v {
            1 => Pixel::White,
            2 => Pixel::Black,
            _ => Pixel::Clear,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Pixel>,
}

impl Picture {
    pub fn new(width: u32, height: u32) -> Picture {
        Picture { width, height, pixels: vec![Pixel::Clear; (width * height) as usize] }
    }

    pub fn ensure_size(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height || self.pixels.len() != (width * height) as usize {
            let mut resized = Picture::new(width, height);
            for y in 0..height.min(self.height) {
                for x in 0..width.min(self.width) {
                    resized.set(x as i32, y as i32, self.get(x as i32, y as i32));
                }
            }
            *self = resized;
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Pixel {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Pixel::Clear;
        }
        self.pixels.get((y as u32 * self.width + x as u32) as usize).copied().unwrap_or(Pixel::Clear)
    }

    pub fn set(&mut self, x: i32, y: i32, p: Pixel) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = (y as u32 * self.width + x as u32) as usize;
        if let Some(px) = self.pixels.get_mut(i) {
            *px = p;
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(Pixel::Clear);
    }

    pub fn is_blank(&self) -> bool {
        self.pixels.iter().all(|p| *p == Pixel::Clear)
    }

    /// Run-length encode as (value, LEB128 run length) pairs, then base64.
    fn encode(&self) -> String {
        if self.is_blank() {
            return String::new();
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.pixels.len() {
            let v = self.pixels[i];
            let mut run = 1usize;
            while i + run < self.pixels.len() && self.pixels[i + run] == v {
                run += 1;
            }
            out.push(v as u8);
            let mut n = run;
            loop {
                let byte = (n & 0x7f) as u8;
                n >>= 7;
                if n == 0 {
                    out.push(byte);
                    break;
                }
                out.push(byte | 0x80);
            }
            i += run;
        }
        STANDARD.encode(out)
    }

    fn decode(width: u32, height: u32, data: &str) -> Result<Picture, String> {
        let mut pic = Picture::new(width, height);
        if data.is_empty() {
            return Ok(pic);
        }
        let bytes = STANDARD.decode(data).map_err(|e| e.to_string())?;
        let mut i = 0;
        let mut pos = 0usize;
        while i < bytes.len() {
            let v = Pixel::from_u8(bytes[i]);
            i += 1;
            let mut run = 0usize;
            let mut shift = 0;
            while i < bytes.len() {
                let b = bytes[i];
                i += 1;
                run |= ((b & 0x7f) as usize) << shift;
                shift += 7;
                if b & 0x80 == 0 {
                    break;
                }
            }
            let end = (pos + run).min(pic.pixels.len());
            pic.pixels[pos..end].fill(v);
            pos = end;
        }
        Ok(pic)
    }
}

#[derive(Serialize, Deserialize)]
struct PictureRepr {
    width: u32,
    height: u32,
    data: String,
}

impl Serialize for Picture {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        PictureRepr { width: self.width, height: self.height, data: self.encode() }.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Picture {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Picture, D::Error> {
        let r = PictureRepr::deserialize(d)?;
        Picture::decode(r.width, r.height, &r.data).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut p = Picture::new(40, 30);
        for x in 0..40 {
            p.set(x, 5, Pixel::Black);
            p.set(x, 6, Pixel::White);
        }
        p.set(3, 29, Pixel::Black);
        let q = Picture::decode(40, 30, &p.encode()).unwrap();
        assert_eq!(p, q);
    }
}
