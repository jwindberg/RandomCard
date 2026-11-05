//! Picture files for Import Paint and Export Paint. Import reads PNG, JPEG,
//! GIF and BMP, MacPaint documents (HyperCard's own export format) and
//! QuickDraw PICT files; Export Paint writes a PNG.

use crate::model::{Picture, Pixel};

/// MacPaint's page: 576 by 720 pixels, 72 bytes a row.
const MACPAINT_WIDTH: u32 = 576;
const MACPAINT_HEIGHT: u32 = 720;
const MACPAINT_HEADER: usize = 512;
const MACBINARY_HEADER: usize = 128;

/// A picture from a file's bytes, whatever its format.
pub fn decode(data: &[u8]) -> Result<Picture, String> {
    if let Ok(format) = image::guess_format(data) {
        let img = image::load_from_memory_with_format(data, format).map_err(|e| e.to_string())?.into_rgba8();
        return Ok(from_rgba(img.width(), img.height(), img.pixels().map(|p| p.0)));
    }
    // A MacPaint document with its Mac file type, in MacBinary.
    if is_macbinary(data, b"PNTG")
        && let Some(pic) = macpaint(&data[MACBINARY_HEADER..])
    {
        return Ok(pic);
    }
    // A PICT file starts with a 512-byte header; a PICT resource doesn't.
    let pict = data.get(MACPAINT_HEADER..).and_then(crate::import::pict::decode).or_else(|| crate::import::pict::decode(data));
    if let Some(p) = pict {
        return Ok(from_rgba(p.width, p.height, p.rgba.into_iter()));
    }
    macpaint(data).ok_or_else(|| "Not a picture RandomCard can read.".into())
}

fn from_rgba(width: u32, height: u32, pixels: impl Iterator<Item = [u8; 4]>) -> Picture {
    let mut pic = Picture::new(width, height);
    for (i, [r, g, b, a]) in pixels.enumerate() {
        pic.set((i as u32 % width.max(1)) as i32, (i as u32 / width.max(1)) as i32, Pixel::rgba(r, g, b, a));
    }
    pic
}

/// Whether `data` is a MacBinary file of the given Mac file type.
fn is_macbinary(data: &[u8], file_type: &[u8; 4]) -> bool {
    data.len() > MACBINARY_HEADER && data[0] == 0 && (1..=63).contains(&data[1]) && &data[65..69] == file_type
}

/// A MacPaint document: a 512-byte header, then 720 rows packed with
/// PackBits. Set bits are black and the rest white.
fn macpaint(data: &[u8]) -> Option<Picture> {
    let mut bytes = data.get(MACPAINT_HEADER..)?.iter().copied();
    let row_bytes = (MACPAINT_WIDTH / 8) as usize;
    let mut pic = Picture::new(MACPAINT_WIDTH, MACPAINT_HEIGHT);
    for y in 0..MACPAINT_HEIGHT as i32 {
        let mut row = Vec::with_capacity(row_bytes);
        while row.len() < row_bytes {
            let n = bytes.next()? as i8;
            if n >= 0 {
                for _ in 0..=n {
                    row.push(bytes.next()?);
                }
            } else if n != -128 {
                let b = bytes.next()?;
                row.extend(std::iter::repeat_n(b, 1 + (-n) as usize));
            }
        }
        // A run may not cross into the next row.
        if row.len() != row_bytes {
            return None;
        }
        for (i, b) in row.iter().enumerate() {
            for bit in 0..8 {
                let black = b & (0x80 >> bit) != 0;
                pic.set(i as i32 * 8 + bit, y, if black { Pixel::BLACK } else { Pixel::WHITE });
            }
        }
    }
    Some(pic)
}

/// A picture from RGBA bytes, row by row (as the system clipboard gives).
pub fn from_rgba_bytes(width: u32, height: u32, bytes: &[u8]) -> Picture {
    from_rgba(width, height, bytes.as_chunks::<4>().0.iter().copied())
}

/// The picture's pixels as RGBA bytes, row by row.
pub fn rgba_bytes(pic: &Picture) -> Vec<u8> {
    pic.iter().flat_map(|p| p.0).collect()
}

/// The picture scaled down, keeping its shape, so it fits in `width` by
/// `height`; as it is if it already fits.
pub fn fit_within(pic: Picture, width: u32, height: u32) -> Picture {
    if pic.width <= width && pic.height <= height || pic.width == 0 || pic.height == 0 {
        return pic;
    }
    let scale = (width as f64 / pic.width as f64).min(height as f64 / pic.height as f64);
    let (w, h) = (((pic.width as f64 * scale).round() as u32).max(1), ((pic.height as f64 * scale).round() as u32).max(1));
    let Some(img) = image::RgbaImage::from_raw(pic.width, pic.height, rgba_bytes(&pic)) else { return pic };
    let small = image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle);
    from_rgba(w, h, small.pixels().map(|p| p.0))
}

/// RGBA pixels, row by row, as a PNG.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(rgba).map_err(|e| e.to_string())?;
    w.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A MacPaint document whose first row starts with one black byte.
    fn macpaint_doc() -> Vec<u8> {
        let mut doc = vec![0u8; MACPAINT_HEADER];
        // Row 0: 0xFF then 71 zeros; the other rows all zeros.
        doc.extend([0x00, 0xFF, (-70i8) as u8, 0x00]);
        for _ in 1..MACPAINT_HEIGHT {
            doc.extend([(-71i8) as u8, 0x00]);
        }
        doc
    }

    #[test]
    fn reads_macpaint() {
        let pic = decode(&macpaint_doc()).unwrap();
        assert_eq!((pic.width, pic.height), (576, 720));
        assert_eq!(pic.get(7, 0), Pixel::BLACK);
        assert_eq!(pic.get(8, 0), Pixel::WHITE);
        assert_eq!(pic.get(0, 719), Pixel::WHITE);
        // In MacBinary too.
        let mut mb = vec![0u8; MACBINARY_HEADER];
        mb[1] = 5;
        mb[65..69].copy_from_slice(b"PNTG");
        mb.extend(macpaint_doc());
        assert_eq!(decode(&mb).unwrap().get(0, 0), Pixel::BLACK);
        // Cut short, it isn't one.
        assert!(decode(&macpaint_doc()[..600]).is_err());
    }

    #[test]
    fn fitting() {
        let mut big = Picture::new(1000, 500);
        big.set(999, 499, Pixel::BLACK);
        let small = fit_within(big, 512, 342);
        // Half the size, the same shape.
        assert_eq!((small.width, small.height), (512, 256));
        let tiny = Picture::new(20, 10);
        assert_eq!(fit_within(tiny.clone(), 512, 342), tiny);
        let bytes = rgba_bytes(&small);
        assert_eq!(from_rgba_bytes(512, 256, &bytes), small);
    }

    #[test]
    fn png_round_trip() {
        let rgba = [255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255];
        let png = encode_png(2, 2, &rgba).unwrap();
        let pic = decode(&png).unwrap();
        assert_eq!((pic.width, pic.height), (2, 2));
        assert_eq!(pic.get(0, 0), Pixel::rgba(255, 0, 0, 255));
        assert_eq!(pic.get(1, 0), Pixel::CLEAR);
        assert_eq!(pic.get(0, 1), Pixel::BLACK);
        assert_eq!(pic.get(1, 1), Pixel::WHITE);
    }
}
