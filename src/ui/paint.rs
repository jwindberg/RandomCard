//! Raster operations for the paint tools, on 1-bit-plus-transparency pictures.

use crate::model::{Picture, Pixel};

pub fn is_dark(p: Pixel) -> bool {
    p == Pixel::Black
}

/// A square pen of `size` pixels centred on (x, y).
pub fn dot(pic: &mut Picture, x: i32, y: i32, size: u32, color: Pixel) {
    let size = size.max(1) as i32;
    let off = (size - 1) / 2;
    for dy in 0..size {
        for dx in 0..size {
            pic.set(x - off + dx, y - off + dy, color);
        }
    }
}

/// A round brush of the given diameter.
pub fn disc(pic: &mut Picture, x: i32, y: i32, diameter: u32, color: Pixel) {
    let r = diameter as f32 / 2.0;
    let ri = r.ceil() as i32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if (dx * dx + dy * dy) as f32 <= r * r {
                pic.set(x + dx, y + dy, color);
            }
        }
    }
}

/// Bresenham line, stamping `stamp` at every point.
pub fn line_with(x0: i32, y0: i32, x1: i32, y1: i32, mut stamp: impl FnMut(i32, i32)) {
    let (mut x, mut y) = (x0, y0);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        stamp(x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub fn line(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, size: u32, color: Pixel) {
    line_with(x0, y0, x1, y1, |x, y| dot(pic, x, y, size, color));
}

fn norm(x0: i32, y0: i32, x1: i32, y1: i32) -> (i32, i32, i32, i32) {
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}

pub fn rect(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, size: u32, filled: bool) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    if filled {
        for y in t..=b {
            for x in l..=r {
                pic.set(x, y, Pixel::Black);
            }
        }
        return;
    }
    let s = size.max(1) as i32;
    for i in 0..s {
        for x in l..=r {
            pic.set(x, t + i, Pixel::Black);
            pic.set(x, b - i, Pixel::Black);
        }
        for y in t..=b {
            pic.set(l + i, y, Pixel::Black);
            pic.set(r - i, y, Pixel::Black);
        }
    }
}

/// Whether (x, y) lies inside a rounded rectangle.
fn in_round_rect(x: i32, y: i32, l: i32, t: i32, r: i32, b: i32, radius: i32) -> bool {
    if x < l || x > r || y < t || y > b {
        return false;
    }
    let rad = radius.min((r - l) / 2).min((b - t) / 2).max(0);
    let cx = if x < l + rad { l + rad } else if x > r - rad { r - rad } else { x };
    let cy = if y < t + rad { t + rad } else if y > b - rad { b - rad } else { y };
    let (dx, dy) = (x - cx, y - cy);
    dx * dx + dy * dy <= rad * rad
}

pub fn round_rect(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, size: u32, filled: bool) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    let s = size.max(1) as i32;
    for y in t..=b {
        for x in l..=r {
            let outer = in_round_rect(x, y, l, t, r, b, 10);
            let inner = in_round_rect(x, y, l + s, t + s, r - s, b - s, 10 - s);
            if outer && (filled || !inner) {
                pic.set(x, y, Pixel::Black);
            }
        }
    }
}

/// Whether pixel (x, y) lies inside the ellipse filling pixels l..=r, t..=b.
fn in_ellipse(x: i32, y: i32, l: f32, t: f32, r: f32, b: f32) -> bool {
    let (cx, cy) = ((l + r + 1.0) / 2.0, (t + b + 1.0) / 2.0);
    let (rx, ry) = (((r - l + 1.0) / 2.0).max(0.5), ((b - t + 1.0) / 2.0).max(0.5));
    let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
    dx * dx + dy * dy <= 1.0
}

pub fn oval(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, size: u32, filled: bool) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    let s = size.max(1) as f32;
    let (lf, tf, rf, bf) = (l as f32, t as f32, r as f32, b as f32);
    for y in t..=b {
        for x in l..=r {
            let outer = in_ellipse(x, y, lf, tf, rf, bf);
            let inner = in_ellipse(x, y, lf + s, tf + s, rf - s, bf - s);
            if outer && (filled || !inner) {
                pic.set(x, y, Pixel::Black);
            }
        }
    }
}

/// Flood fill the region of same-coloured pixels (clear counts as white).
pub fn fill(pic: &mut Picture, x: i32, y: i32, color: Pixel) {
    let target = is_dark(pic.get(x, y));
    if x < 0 || y < 0 || x >= pic.width as i32 || y >= pic.height as i32 {
        return;
    }
    if target == is_dark(color) {
        return;
    }
    let mut stack = vec![(x, y)];
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= pic.width as i32 || y >= pic.height as i32 {
            continue;
        }
        if is_dark(pic.get(x, y)) != target {
            continue;
        }
        // Fill a horizontal span, then queue the rows above and below.
        let mut l = x;
        while l > 0 && is_dark(pic.get(l - 1, y)) == target {
            l -= 1;
        }
        let mut r = x;
        while r + 1 < pic.width as i32 && is_dark(pic.get(r + 1, y)) == target {
            r += 1;
        }
        for i in l..=r {
            pic.set(i, y, color);
            stack.push((i, y - 1));
            stack.push((i, y + 1));
        }
    }
}

pub fn spray(pic: &mut Picture, x: i32, y: i32) {
    for _ in 0..12 {
        let dx: i32 = rand::random_range(-8..=8);
        let dy: i32 = rand::random_range(-8..=8);
        if dx * dx + dy * dy <= 64 {
            pic.set(x + dx, y + dy, Pixel::Black);
        }
    }
}

/// Copy a rectangular region out of a picture.
pub fn copy_region(pic: &Picture, l: i32, t: i32, r: i32, b: i32) -> Picture {
    let (w, h) = ((r - l).max(0) as u32, (b - t).max(0) as u32);
    let mut out = Picture::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            out.set(x, y, pic.get(l + x, t + y));
        }
    }
    out
}

pub fn clear_region(pic: &mut Picture, l: i32, t: i32, r: i32, b: i32) {
    for y in t..b {
        for x in l..r {
            pic.set(x, y, Pixel::Clear);
        }
    }
}

/// Paste non-clear pixels of `src` at (x, y).
pub fn paste(pic: &mut Picture, src: &Picture, x: i32, y: i32) {
    for sy in 0..src.height as i32 {
        for sx in 0..src.width as i32 {
            let p = src.get(sx, sy);
            if p != Pixel::Clear {
                pic.set(x + sx, y + sy, p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_bounded() {
        let mut p = Picture::new(20, 20);
        rect(&mut p, 2, 2, 10, 10, 1, false);
        fill(&mut p, 5, 5, Pixel::Black);
        assert_eq!(p.get(5, 5), Pixel::Black);
        assert_eq!(p.get(15, 15), Pixel::Clear);
        fill(&mut p, 15, 15, Pixel::Black);
        assert_eq!(p.get(0, 0), Pixel::Black);
    }

    #[test]
    fn shapes() {
        let mut p = Picture::new(30, 30);
        oval(&mut p, 0, 0, 29, 29, 1, false);
        assert_eq!(p.get(15, 0), Pixel::Black);
        assert_eq!(p.get(15, 15), Pixel::Clear);
        line(&mut p, 0, 0, 29, 29, 1, Pixel::Black);
        assert_eq!(p.get(10, 10), Pixel::Black);
    }
}
