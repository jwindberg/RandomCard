//! Paint layers. A picture holds any color with transparency, and card and
//! background layers need "no paint here" (clear) so lower layers show
//! through. Most pictures are HyperCard's black and white, so a picture
//! keeps one byte a pixel until a color goes into it, and is saved in the
//! same compact form; a picture with color is saved as a PNG.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A pixel's color, as red, green, blue and alpha (not premultiplied).
/// Every fully transparent pixel is `CLEAR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pixel(pub [u8; 4]);

impl Pixel {
    pub const CLEAR: Pixel = Pixel([0, 0, 0, 0]);
    pub const WHITE: Pixel = Pixel([255, 255, 255, 255]);
    pub const BLACK: Pixel = Pixel([0, 0, 0, 255]);

    /// Any color, with transparency made `CLEAR`.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Pixel {
        if a == 0 { Pixel::CLEAR } else { Pixel([r, g, b, a]) }
    }

    pub fn is_clear(self) -> bool {
        self.0[3] == 0
    }

    /// The black-and-white code: 0 clear, 1 white, 2 black.
    fn mono(self) -> Option<u8> {
        match self {
            Pixel::CLEAR => Some(0),
            Pixel::WHITE => Some(1),
            Pixel::BLACK => Some(2),
            _ => None,
        }
    }

    fn from_mono(v: u8) -> Pixel {
        match v {
            1 => Pixel::WHITE,
            2 => Pixel::BLACK,
            _ => Pixel::CLEAR,
        }
    }
}

/// How a picture keeps its pixels: a black-and-white code each, until a
/// color goes in.
#[derive(Clone, Debug, PartialEq)]
enum Pixels {
    Mono(Vec<u8>),
    Color(Vec<Pixel>),
}

impl Default for Pixels {
    fn default() -> Pixels {
        Pixels::Mono(Vec::new())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pixels: Pixels,
}

/// Pictures are equal when their pixels are, however they're kept.
impl PartialEq for Picture {
    fn eq(&self, other: &Picture) -> bool {
        self.width == other.width
            && self.height == other.height
            && match (&self.pixels, &other.pixels) {
                (Pixels::Mono(a), Pixels::Mono(b)) => a == b,
                _ => self.iter().eq(other.iter()),
            }
    }
}

impl Picture {
    pub fn new(width: u32, height: u32) -> Picture {
        Picture { width, height, pixels: Pixels::Mono(vec![0; (width * height) as usize]) }
    }

    fn len(&self) -> usize {
        match &self.pixels {
            Pixels::Mono(v) => v.len(),
            Pixels::Color(v) => v.len(),
        }
    }

    pub fn ensure_size(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height || self.len() != (width * height) as usize {
            let mut resized = Picture::new(width, height);
            for y in 0..height.min(self.height) {
                for x in 0..width.min(self.width) {
                    resized.set(x as i32, y as i32, self.get(x as i32, y as i32));
                }
            }
            *self = resized;
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some((y as u32 * self.width + x as u32) as usize)
    }

    pub fn get(&self, x: i32, y: i32) -> Pixel {
        let Some(i) = self.index(x, y) else { return Pixel::CLEAR };
        match &self.pixels {
            Pixels::Mono(v) => v.get(i).map_or(Pixel::CLEAR, |&m| Pixel::from_mono(m)),
            Pixels::Color(v) => v.get(i).copied().unwrap_or(Pixel::CLEAR),
        }
    }

    pub fn set(&mut self, x: i32, y: i32, p: Pixel) {
        let Some(i) = self.index(x, y) else { return };
        let p = if p.is_clear() { Pixel::CLEAR } else { p };
        if let (Pixels::Mono(v), Some(m)) = (&mut self.pixels, p.mono()) {
            if let Some(px) = v.get_mut(i) {
                *px = m;
            }
            return;
        }
        if let Some(px) = self.color_mut().get_mut(i) {
            *px = p;
        }
    }

    /// The pixels as colors, kept that way from now on.
    fn color_mut(&mut self) -> &mut Vec<Pixel> {
        if let Pixels::Mono(v) = &self.pixels {
            self.pixels = Pixels::Color(v.iter().map(|&m| Pixel::from_mono(m)).collect());
        }
        match &mut self.pixels {
            Pixels::Color(v) => v,
            Pixels::Mono(_) => unreachable!(),
        }
    }

    /// Every pixel, row by row from the top left.
    pub fn iter(&self) -> Box<dyn Iterator<Item = Pixel> + '_> {
        match &self.pixels {
            Pixels::Mono(v) => Box::new(v.iter().map(|&m| Pixel::from_mono(m))),
            Pixels::Color(v) => Box::new(v.iter().copied()),
        }
    }

    /// Change every pixel.
    pub fn map(&mut self, f: impl Fn(Pixel) -> Pixel) {
        match &mut self.pixels {
            Pixels::Mono(v) => {
                let mut colors = None;
                for (i, m) in v.iter_mut().enumerate() {
                    let p = f(Pixel::from_mono(*m));
                    match p.mono() {
                        Some(n) => *m = n,
                        None => {
                            colors = Some(i);
                            break;
                        }
                    }
                }
                // A color came out: go on in color from that pixel.
                if let Some(from) = colors {
                    let v = self.color_mut();
                    for p in &mut v[from..] {
                        *p = f(*p);
                    }
                    self.normalize_clear();
                }
            }
            Pixels::Color(v) => {
                for p in v.iter_mut() {
                    *p = f(*p);
                }
                self.normalize_clear();
            }
        }
    }

    fn normalize_clear(&mut self) {
        if let Pixels::Color(v) = &mut self.pixels {
            for p in v.iter_mut().filter(|p| p.is_clear()) {
                *p = Pixel::CLEAR;
            }
        }
    }

    /// Whether any pixel is a color other than black, white or clear.
    pub fn has_color(&self) -> bool {
        match &self.pixels {
            Pixels::Mono(_) => false,
            Pixels::Color(v) => v.iter().any(|p| p.mono().is_none()),
        }
    }

    pub fn clear(&mut self) {
        self.pixels = Pixels::Mono(vec![0; self.len()]);
    }

    pub fn is_blank(&self) -> bool {
        match &self.pixels {
            Pixels::Mono(v) => v.iter().all(|&m| m == 0),
            Pixels::Color(v) => v.iter().all(|p| p.is_clear()),
        }
    }

    /// Black and white: run-length encode the codes as (code, LEB128 run
    /// length) pairs, then base64.
    fn encode(&self) -> String {
        if self.is_blank() {
            return String::new();
        }
        let codes: Vec<u8> = self.iter().map(|p| p.mono().unwrap_or(2)).collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < codes.len() {
            let v = codes[i];
            let mut run = 1usize;
            while i + run < codes.len() && codes[i + run] == v {
                run += 1;
            }
            out.push(v);
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
        let mut codes = vec![0u8; (width * height) as usize];
        if !data.is_empty() {
            let bytes = STANDARD.decode(data).map_err(|e| e.to_string())?;
            let mut i = 0;
            let mut pos = 0usize;
            while i < bytes.len() {
                let v = bytes[i].min(2);
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
                let end = (pos + run).min(codes.len());
                codes[pos..end].fill(v);
                pos = end;
            }
        }
        Ok(Picture { width, height, pixels: Pixels::Mono(codes) })
    }

    /// The picture as a PNG, base64-encoded.
    fn encode_png(&self) -> Result<String, String> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width, self.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(|e| e.to_string())?;
            let bytes: Vec<u8> = self.iter().flat_map(|p| p.0).collect();
            w.write_image_data(&bytes).map_err(|e| e.to_string())?;
        }
        Ok(STANDARD.encode(out))
    }

    /// A picture from a PNG (base64), placed at the top left of a picture
    /// of the given size.
    fn decode_png(width: u32, height: u32, data: &str) -> Result<Picture, String> {
        let bytes = STANDARD.decode(data).map_err(|e| e.to_string())?;
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
        let mut reader = dec.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let channels = info.color_type.samples();
        let mut pic = Picture::new(width, height);
        for y in 0..info.height.min(height) {
            for x in 0..info.width.min(width) {
                let i = (y as usize * info.line_size) + x as usize * channels;
                let p = match channels {
                    4 => Pixel::rgba(buf[i], buf[i + 1], buf[i + 2], buf[i + 3]),
                    2 => Pixel::rgba(buf[i], buf[i], buf[i], buf[i + 1]),
                    _ => continue,
                };
                pic.set(x as i32, y as i32, p);
            }
        }
        Ok(pic)
    }
}

#[derive(Serialize, Deserialize)]
struct PictureRepr {
    width: u32,
    height: u32,
    /// Black and white, run-length encoded.
    #[serde(default)]
    data: String,
    /// In color, as a PNG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    png: Option<String>,
}

impl Serialize for Picture {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let repr = if self.has_color() {
            let png = self.encode_png().map_err(serde::ser::Error::custom)?;
            PictureRepr { width: self.width, height: self.height, data: String::new(), png: Some(png) }
        } else {
            PictureRepr { width: self.width, height: self.height, data: self.encode(), png: None }
        };
        repr.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Picture {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Picture, D::Error> {
        let r = PictureRepr::deserialize(d)?;
        match &r.png {
            Some(png) => Picture::decode_png(r.width, r.height, png),
            None => Picture::decode(r.width, r.height, &r.data),
        }
        .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Picture {
        let mut p = Picture::new(40, 30);
        for x in 0..40 {
            p.set(x, 5, Pixel::BLACK);
            p.set(x, 6, Pixel::WHITE);
        }
        p.set(3, 29, Pixel::BLACK);
        p
    }

    #[test]
    fn round_trip() {
        let p = sample();
        let q = Picture::decode(40, 30, &p.encode()).unwrap();
        assert_eq!(p, q);
    }

    #[test]
    fn black_and_white_stays_compact() {
        let p = sample();
        assert!(matches!(p.pixels, Pixels::Mono(_)));
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("png"), "{json}");
        // Saved exactly as before color: width, height and the codes.
        assert_eq!(json, format!(r#"{{"width":40,"height":30,"data":"{}"}}"#, p.encode()));
    }

    #[test]
    fn color_round_trips_as_png() {
        let mut p = sample();
        p.set(10, 10, Pixel::rgba(200, 30, 40, 255));
        p.set(11, 10, Pixel::rgba(0, 128, 255, 100));
        p.set(12, 10, Pixel::rgba(9, 9, 9, 0));
        assert!(p.has_color());
        assert_eq!(p.get(12, 10), Pixel::CLEAR);
        assert_eq!(p.get(3, 29), Pixel::BLACK);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"png\""), "{json}");
        let q: Picture = serde_json::from_str(&json).unwrap();
        assert_eq!(p, q);
        assert_eq!(q.get(11, 10), Pixel([0, 128, 255, 100]));
    }

    #[test]
    fn color_painted_out_saves_as_black_and_white() {
        let mut p = sample();
        p.set(10, 10, Pixel::rgba(200, 30, 40, 255));
        p.set(10, 10, Pixel::WHITE);
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("png"));
        assert_eq!(serde_json::from_str::<Picture>(&json).unwrap(), p);
    }

    #[test]
    fn map_switches_to_color_midway() {
        let mut p = sample();
        p.map(|px| if px == Pixel::BLACK { Pixel::rgba(1, 2, 3, 255) } else { px });
        assert_eq!(p.get(0, 5), Pixel::rgba(1, 2, 3, 255));
        assert_eq!(p.get(3, 29), Pixel::rgba(1, 2, 3, 255));
        assert_eq!(p.get(0, 6), Pixel::WHITE);
        assert_eq!(p.get(0, 0), Pixel::CLEAR);
    }
}
