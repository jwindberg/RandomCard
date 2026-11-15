//! Raster painting on pictures, shared by the paint tools and by scripts
//! (`drag from … to …`, `click at`, `type`). As in QuickDraw, a pattern is
//! a mask: its black bits paint in a color (the fill color) and its white
//! bits in white. Lines, borders, the pencil and text use the line color.
//! With both colors black, painting is HyperCard's black and white.

pub mod image_file;
mod text;

use crate::model::{Picture, Pixel};
pub use text::{PaintText, draw_text};

/// What paint is laid down with: a solid pixel value, or an 8×8 pattern
/// whose black bits are in `color` and white bits white.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Solid(Pixel),
    Pattern { rows: [u8; 8], color: Pixel },
}

impl Ink {
    /// A black-and-white pattern.
    #[cfg(test)]
    pub fn pattern(rows: [u8; 8]) -> Ink {
        Ink::Pattern { rows, color: Pixel::BLACK }
    }

    /// Whether the pattern's bit at (x, y) is set (always, for solid ink).
    pub fn covers(&self, x: i32, y: i32) -> bool {
        match self {
            Ink::Solid(_) => true,
            Ink::Pattern { rows, .. } => rows[y.rem_euclid(8) as usize] & (0x80 >> x.rem_euclid(8)) != 0,
        }
    }

    pub fn at(&self, x: i32, y: i32) -> Pixel {
        match self {
            Ink::Solid(p) => *p,
            Ink::Pattern { color, .. } => {
                if self.covers(x, y) {
                    *color
                } else {
                    Pixel::WHITE
                }
            }
        }
    }
}

/// The lines a shape is drawn with: their width and color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub size: u32,
    pub color: Pixel,
}

impl Outline {
    /// HyperCard's: black, `size` pixels wide.
    #[cfg(test)]
    pub fn black(size: u32) -> Outline {
        Outline { size, color: Pixel::BLACK }
    }
}

/// RandomCard's default pattern palette (black first, white second). These
/// are original designs; imported stacks bring their own patterns.
pub const DEFAULT_PATTERNS: [[u8; 8]; 40] = [
    [0xff; 8],                                        // black
    [0x00; 8],                                        // white
    [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55], // 50% gray
    [0x88, 0x22, 0x88, 0x22, 0x88, 0x22, 0x88, 0x22], // 25% gray
    [0x77, 0xdd, 0x77, 0xdd, 0x77, 0xdd, 0x77, 0xdd], // 75% gray
    [0x80, 0x00, 0x08, 0x00, 0x80, 0x00, 0x08, 0x00], // 12% gray
    [0x80, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00], // light dots
    [0xee, 0xff, 0xbb, 0xff, 0xee, 0xff, 0xbb, 0xff], // dark dots
    [0xff, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00], // horizontal lines
    [0x88; 8],                                        // vertical lines
    [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80], // diagonal /
    [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01], // diagonal \
    [0xff, 0x88, 0x88, 0x88, 0xff, 0x88, 0x88, 0x88], // grid
    [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81], // crosshatch
    [0xf0, 0xf0, 0xf0, 0xf0, 0x0f, 0x0f, 0x0f, 0x0f], // checkerboard
    [0xcc, 0xcc, 0x33, 0x33, 0xcc, 0xcc, 0x33, 0x33], // small checks
    [0xff, 0x80, 0x80, 0x80, 0xff, 0x08, 0x08, 0x08], // bricks
    [0x80, 0x41, 0x22, 0x14, 0x08, 0x10, 0x20, 0x40], // zigzag
    [0xff, 0xff, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00], // wide stripes
    [0xcc; 8],                                        // wide vertical stripes
    [0x03, 0x06, 0x0c, 0x18, 0x30, 0x60, 0xc0, 0x81], // thick diagonal /
    [0xc0, 0x60, 0x30, 0x18, 0x0c, 0x06, 0x03, 0x81], // thick diagonal \
    [0x10, 0x38, 0x7c, 0xfe, 0x7c, 0x38, 0x10, 0x00], // diamonds
    [0x00, 0x18, 0x3c, 0x7e, 0x7e, 0x3c, 0x18, 0x00], // circles
    [0x18, 0x18, 0xff, 0xff, 0x18, 0x18, 0x18, 0x18], // plus
    [0x81, 0x42, 0x24, 0x18, 0x00, 0x00, 0x00, 0x00], // chevrons
    [0xf8, 0x74, 0x22, 0x47, 0x8f, 0x17, 0x22, 0x71], // weave
    [0xaa, 0x00, 0xaa, 0x00, 0xaa, 0x00, 0xaa, 0x00], // fine dots
    [0xff, 0x55, 0xff, 0x55, 0xff, 0x55, 0xff, 0x55], // dark stripes
    [0x11, 0x22, 0x44, 0x88, 0x11, 0x22, 0x44, 0x88], // fine diagonal
    [0x88, 0x44, 0x22, 0x11, 0x88, 0x44, 0x22, 0x11], // fine diagonal \
    [0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0xff], // wide lines
    [0x10; 8],                                        // wide vertical lines
    [0x01, 0x01, 0x01, 0xff, 0x10, 0x10, 0x10, 0xff], // offset bricks
    [0xe0, 0xd1, 0xbb, 0x1d, 0x0e, 0x17, 0xbb, 0x71], // basket
    [0x55, 0xaa, 0x55, 0xaa, 0x00, 0x00, 0x00, 0x00], // half gray band
    [0x7f, 0xbf, 0xdf, 0xef, 0xf7, 0xfb, 0xfd, 0xfe], // inverse diagonal
    [0xff, 0x81, 0x81, 0x81, 0x81, 0x81, 0x81, 0xff], // boxes
    [0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00], // sparse squares
    [0xaa, 0x44, 0xaa, 0x11, 0xaa, 0x44, 0xaa, 0x11], // textured gray
];

/// A pixel as it shows on paper: clear is white.
pub fn paper(p: Pixel) -> Pixel {
    if p.is_clear() { Pixel::WHITE } else { p }
}

/// The color a pencil stroke starting at (x, y) draws in: the line color,
/// or white when it starts on that color, as MacPaint's pencil erased by
/// starting on black.
pub fn pencil_color(pic: &Picture, x: i32, y: i32, line: Pixel) -> Pixel {
    if paper(pic.get(x, y)) == paper(line) && paper(line) != Pixel::WHITE { Pixel::WHITE } else { line }
}

/// A square pen of `size` pixels centered on (x, y).
pub fn dot(pic: &mut Picture, x: i32, y: i32, size: u32, color: Pixel) {
    let size = size.max(1) as i32;
    let off = (size - 1) / 2;
    for dy in 0..size {
        for dx in 0..size {
            pic.set(x - off + dx, y - off + dy, color);
        }
    }
}

/// Brush shapes, numbered from 1 as `the brush`; numbers wrap around.
pub const BRUSH_NAMES: [&str; 16] = [
    "Round 2", "Round 4", "Round 6", "Round 8", "Round 12", "Square 2", "Square 4", "Square 6", "Square 8",
    "Horizontal", "Vertical", "Slash", "Backslash", "Short horizontal", "Short vertical", "Dots",
];

/// The pixel offsets a brush covers.
pub fn brush_shape(brush: u32) -> Vec<(i32, i32)> {
    let n = (brush.max(1) - 1) % BRUSH_NAMES.len() as u32 + 1;
    let round = |d: f32| {
        let r = d / 2.0;
        let ri = r.ceil() as i32;
        let mut v = Vec::new();
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f32 <= r * r {
                    v.push((dx, dy));
                }
            }
        }
        v
    };
    let square = |s: i32| {
        let off = (s - 1) / 2;
        (0..s).flat_map(|y| (0..s).map(move |x| (x - off, y - off))).collect::<Vec<_>>()
    };
    match n {
        1 => round(2.0),
        2 => round(4.0),
        3 => round(6.0),
        4 => round(8.0),
        5 => round(12.0),
        6 => square(2),
        7 => square(4),
        8 => square(6),
        9 => square(8),
        10 => (-4..4).map(|x| (x, 0)).collect(),
        11 => (-4..4).map(|y| (0, y)).collect(),
        12 => (-4..4).map(|i| (i, -i)).collect(),
        13 => (-4..4).map(|i| (i, i)).collect(),
        14 => (-2..2).map(|x| (x, 0)).collect(),
        15 => (-2..2).map(|y| (0, y)).collect(),
        _ => vec![(-2, -2), (1, -2), (-1, 0), (2, 0), (-2, 2), (1, 2)],
    }
}

pub fn stamp(pic: &mut Picture, x: i32, y: i32, shape: &[(i32, i32)], ink: Ink) {
    for (dx, dy) in shape {
        pic.set(x + dx, y + dy, ink.at(x + dx, y + dy));
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

/// Paint every pixel of a bounding box for which `inside` holds: the border
/// band in the outline's color, and the interior with `fill` when given.
fn shape(
    pic: &mut Picture,
    (l, t, r, b): (i32, i32, i32, i32),
    outline: Outline,
    fill: Option<Ink>,
    inside: impl Fn(i32, i32, i32) -> bool,
) {
    let s = outline.size.max(1) as i32;
    for y in t..=b {
        for x in l..=r {
            if !inside(x, y, 0) {
                continue;
            }
            if !inside(x, y, s) {
                pic.set(x, y, outline.color);
            } else if let Some(ink) = fill {
                pic.set(x, y, ink.at(x, y));
            }
        }
    }
}

pub fn rect(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, outline: Outline, fill: Option<Ink>) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    shape(pic, (l, t, r, b), outline, fill, |x, y, s| x >= l + s && x <= r - s && y >= t + s && y <= b - s);
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

pub fn round_rect(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, outline: Outline, fill: Option<Ink>) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    shape(pic, (l, t, r, b), outline, fill, |x, y, s| in_round_rect(x, y, l + s, t + s, r - s, b - s, 10 - s));
}

/// Whether pixel (x, y) lies inside the ellipse filling pixels l..=r, t..=b.
fn in_ellipse(x: i32, y: i32, l: f32, t: f32, r: f32, b: f32) -> bool {
    let (cx, cy) = ((l + r + 1.0) / 2.0, (t + b + 1.0) / 2.0);
    let (rx, ry) = (((r - l + 1.0) / 2.0).max(0.5), ((b - t + 1.0) / 2.0).max(0.5));
    let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
    dx * dx + dy * dy <= 1.0
}

pub fn oval(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, outline: Outline, fill: Option<Ink>) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    let (lf, tf, rf, bf) = (l as f32, t as f32, r as f32, b as f32);
    shape(pic, (l, t, r, b), outline, fill, |x, y, s| {
        let s = s as f32;
        in_ellipse(x, y, lf + s, tf + s, rf - s, bf - s)
    });
}

/// Even-odd point-in-polygon test at the pixel's center.
pub fn in_polygon(points: &[(i32, i32)], x: i32, y: i32) -> bool {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let mut inside = false;
    let n = points.len();
    for i in 0..n {
        let (x1, y1) = (points[i].0 as f32, points[i].1 as f32);
        let (x2, y2) = (points[(i + 1) % n].0 as f32, points[(i + 1) % n].1 as f32);
        if (y1 > py) != (y2 > py) && px < (x2 - x1) * (py - y1) / (y2 - y1) + x1 {
            inside = !inside;
        }
    }
    inside
}

/// A closed polygon: filled with `fill` if given, and outlined.
pub fn polygon(pic: &mut Picture, points: &[(i32, i32)], outline: Outline, fill: Option<Ink>) {
    if points.len() < 2 {
        return;
    }
    if let Some(ink) = fill
        && points.len() >= 3
    {
        let (l, t) = points.iter().fold((i32::MAX, i32::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
        let (r, b) = points.iter().fold((i32::MIN, i32::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
        for y in t..=b {
            for x in l..=r {
                if in_polygon(points, x, y) {
                    pic.set(x, y, ink.at(x, y));
                }
            }
        }
    }
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        line(pic, a.0, a.1, b.0, b.1, outline.size, outline.color);
    }
}

/// A regular polygon dragged out from its center to a corner.
pub fn regular_polygon_points(center: (i32, i32), corner: (i32, i32), sides: u32) -> Vec<(i32, i32)> {
    let sides = sides.clamp(3, 50);
    let (dx, dy) = ((corner.0 - center.0) as f32, (corner.1 - center.1) as f32);
    let radius = (dx * dx + dy * dy).sqrt();
    let start = dy.atan2(dx);
    (0..sides)
        .map(|i| {
            let a = start + i as f32 * std::f32::consts::TAU / sides as f32;
            (center.0 + (radius * a.cos()).round() as i32, center.1 + (radius * a.sin()).round() as i32)
        })
        .collect()
}

/// Shift with the line tool: `b` turned to the nearest multiple of 15°
/// around `a`, keeping its length. The polygon tools use it too.
pub fn snap_15(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
    let length = (dx * dx + dy * dy).sqrt();
    let step = 15f32.to_radians();
    let angle = (dy.atan2(dx) / step).round() * step;
    (a.0 + (length * angle.cos()).round() as i32, a.1 + (length * angle.sin()).round() as i32)
}

/// Shift with a shape tool: the corner moved so the shape is a square or
/// a circle, as big as the longer side of the drag.
pub fn square(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let side = dx.abs().max(dy.abs());
    let sign = |d: i32| if d < 0 { -1 } else { 1 };
    (a.0 + side * sign(dx), a.1 + side * sign(dy))
}

/// Which way a Shift-drag goes: horizontally or vertically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// The way a move from `a` to `b` mostly goes, once it has moved at all.
    pub fn of(a: (i32, i32), b: (i32, i32)) -> Option<Axis> {
        let (dx, dy) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
        if dx == 0 && dy == 0 {
            None
        } else if dx >= dy {
            Some(Axis::Horizontal)
        } else {
            Some(Axis::Vertical)
        }
    }

    /// `b` kept on the line through `a` that goes this way.
    pub fn keep(self, a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
        match self {
            Axis::Horizontal => (b.0, a.1),
            Axis::Vertical => (a.0, b.1),
        }
    }
}

/// With Draw Centered, where a line or shape really starts: as far before
/// the drag's start as its end is after it. A regular polygon always grows
/// from its center, and freehand tools aren't affected.
pub fn centered_start(tool: DragTool, a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    match tool {
        DragTool::Line | DragTool::Rect | DragTool::RoundRect | DragTool::Oval => (2 * a.0 - b.0, 2 * a.1 - b.1),
        _ => a,
    }
}

/// HyperCard's Grid: points snap to an invisible grid 8 pixels apart.
pub const GRID: i32 = 8;

pub fn snap_grid(p: (i32, i32)) -> (i32, i32) {
    let snap = |v: i32| (v + GRID / 2).div_euclid(GRID) * GRID;
    (snap(p.0), snap(p.1))
}

/// The tools Grid, Draw Centered and Draw Multiple work with: the line and
/// the shapes.
pub fn shape_tool(tool: DragTool) -> bool {
    matches!(tool, DragTool::Line | DragTool::Rect | DragTool::RoundRect | DragTool::Oval | DragTool::RegularPolygon)
}

/// With Draw Multiple, whether a drag that last left a copy at `last` and is
/// now at `p` has gone far enough to leave another: `the multiSpace` pixels.
pub fn multiple_due(last: (i32, i32), p: (i32, i32), space: u32) -> bool {
    (p.0 - last.0).abs().max((p.1 - last.1).abs()) >= space.max(1) as i32
}

/// Where a Shift-drag with `tool` from `a` to `b` really ends. The
/// freehand tools keep their own Axis, since it's chosen at the start.
pub fn constrain(tool: DragTool, a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    match tool {
        DragTool::Line | DragTool::RegularPolygon => snap_15(a, b),
        DragTool::Rect | DragTool::RoundRect | DragTool::Oval => square(a, b),
        DragTool::Pencil | DragTool::Brush | DragTool::Eraser | DragTool::Spray => {
            Axis::of(a, b).map_or(b, |axis| axis.keep(a, b))
        }
        DragTool::Curve => b,
    }
}

/// Bucket fill: the region of same-colored pixels (clear counts as white)
/// around (x, y), painted with `ink`. Colors must match exactly: painting
/// is never smoothed, so a shape's edge holds the paint in.
pub fn fill(pic: &mut Picture, x: i32, y: i32, ink: Ink) {
    let (w, h) = (pic.width as i32, pic.height as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let target = paper(pic.get(x, y));
    // Find the whole region before painting, since a pattern adds both colors.
    let mut seen = vec![false; (w * h) as usize];
    let mut region = Vec::new();
    let mut stack = vec![(x, y)];
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= w || y >= h {
            continue;
        }
        let i = (y * w + x) as usize;
        if seen[i] || paper(pic.get(x, y)) != target {
            continue;
        }
        seen[i] = true;
        region.push((x, y));
        stack.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
    }
    // Filling a solid area with its own color does nothing.
    if ink == Ink::Solid(target) {
        return;
    }
    for (x, y) in region {
        pic.set(x, y, ink.at(x, y));
    }
}

/// HyperCard's spray: a circle 16 pixels across, 12 dots at a time.
pub const SPRAY_SIZE: u32 = 16;
pub const SPRAY_DENSITY: u32 = 12;
/// The spray sizes and densities the palette offers.
pub const SPRAY_SIZES: [u32; 4] = [8, 16, 24, 32];
pub const SPRAY_DENSITIES: [(u32, &str); 3] = [(4, "Light"), (12, "Medium"), (32, "Heavy")];

/// Spray `density` dots at random in a circle `size` pixels across.
pub fn spray(pic: &mut Picture, x: i32, y: i32, ink: Ink, size: u32, density: u32) {
    let r = (size.clamp(2, 64) / 2) as i32;
    for _ in 0..density.clamp(1, 200) {
        let dx: i32 = rand::random_range(-r..=r);
        let dy: i32 = rand::random_range(-r..=r);
        if dx * dx + dy * dy <= r * r {
            // Spraying a pattern reveals its set bits dot by dot.
            if ink.covers(x + dx, y + dy) {
                pic.set(x + dx, y + dy, ink.at(x + dx, y + dy));
            }
        }
    }
}

/// Paste non-clear pixels of `src` at (x, y).
pub fn paste(pic: &mut Picture, src: &Picture, x: i32, y: i32) {
    for sy in 0..src.height as i32 {
        for sx in 0..src.width as i32 {
            let p = src.get(sx, sy);
            if p != Pixel::CLEAR {
                pic.set(x + sx, y + sy, p);
            }
        }
    }
}

/// A selected area: a rectangle, optionally narrowed by a lasso outline
/// or to just the pixels of a mask.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub rect: (i32, i32, i32, i32),
    pub lasso: Option<Vec<(i32, i32)>>,
    /// Which of the rectangle's pixels are in, row by row: a selection
    /// that hugs what's painted, as the lasso and Select make.
    pub mask: Option<std::sync::Arc<Vec<bool>>>,
}

impl Region {
    pub fn new(rect: (i32, i32, i32, i32)) -> Region {
        Region { rect, lasso: None, mask: None }
    }

    pub fn all(pic: &Picture) -> Region {
        Region::new((0, 0, pic.width as i32, pic.height as i32))
    }

    pub fn width(&self) -> i32 {
        (self.rect.2 - self.rect.0).max(0)
    }

    pub fn height(&self) -> i32 {
        (self.rect.3 - self.rect.1).max(0)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (l, t, r, b) = self.rect;
        x >= l
            && x < r
            && y >= t
            && y < b
            && self.lasso.as_ref().is_none_or(|pts| in_polygon(pts, x, y))
            && self.mask.as_ref().is_none_or(|m| m[((y - t) * (r - l) + x - l) as usize])
    }

    /// The same region moved by (dx, dy).
    pub fn translated(&self, dx: i32, dy: i32) -> Region {
        let (l, t, r, b) = self.rect;
        Region {
            rect: (l + dx, t + dy, r + dx, b + dy),
            lasso: self.lasso.as_ref().map(|pts| pts.iter().map(|(x, y)| (x + dx, y + dy)).collect()),
            mask: self.mask.clone(),
        }
    }

    /// A region of just these pixels, in a rectangle that fits them.
    pub fn from_pixels(pixels: &[(i32, i32)]) -> Option<Region> {
        let l = pixels.iter().map(|p| p.0).min()?;
        let t = pixels.iter().map(|p| p.1).min()?;
        let r = pixels.iter().map(|p| p.0).max()? + 1;
        let b = pixels.iter().map(|p| p.1).max()? + 1;
        let mut mask = vec![false; ((r - l) * (b - t)) as usize];
        for (x, y) in pixels {
            mask[((y - t) * (r - l) + x - l) as usize] = true;
        }
        Some(Region { rect: (l, t, r, b), lasso: None, mask: Some(std::sync::Arc::new(mask)) })
    }

    /// The region's edges, for its moving dashed line: the outline of a
    /// mask as runs between pixel corners, across and then down.
    pub fn mask_edges(&self) -> Vec<((i32, i32), (i32, i32))> {
        let Some(mask) = &self.mask else { return Vec::new() };
        let (l, t, r, b) = self.rect;
        let (w, h) = (r - l, b - t);
        let at = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && mask[(y * w + x) as usize];
        let mut edges = Vec::new();
        // Across: between row y-1 and row y.
        for y in 0..=h {
            let mut run: Option<i32> = None;
            for x in 0..=w {
                let edge = x < w && at(x, y - 1) != at(x, y);
                match (edge, run) {
                    (true, None) => run = Some(x),
                    (false, Some(x0)) => {
                        edges.push(((l + x0, t + y), (l + x, t + y)));
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        // Down: between column x-1 and column x.
        for x in 0..=w {
            let mut run: Option<i32> = None;
            for y in 0..=h {
                let edge = y < h && at(x - 1, y) != at(x, y);
                match (edge, run) {
                    (true, None) => run = Some(y),
                    (false, Some(y0)) => {
                        edges.push(((l + x, t + y0), (l + x, t + y)));
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        edges
    }

    pub fn pixels(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (l, t, r, b) = self.rect;
        (t..b).flat_map(move |y| (l..r).map(move |x| (x, y))).filter(|(x, y)| self.contains(*x, *y))
    }

    /// Lift the selection out of the picture, leaving it clear.
    pub fn lift(&self, pic: &mut Picture) -> Picture {
        let (l, t, r, b) = self.rect;
        let mut out = Picture::new((r - l).max(0) as u32, (b - t).max(0) as u32);
        for (x, y) in self.pixels().collect::<Vec<_>>() {
            out.set(x - l, y - t, pic.get(x, y));
            pic.set(x, y, Pixel::CLEAR);
        }
        out
    }

    /// A copy of the selection's pixels, clear outside it.
    pub fn copy(&self, pic: &Picture) -> Picture {
        let (l, t, _, _) = self.rect;
        let mut out = Picture::new(self.width() as u32, self.height() as u32);
        for (x, y) in self.pixels() {
            out.set(x - l, y - t, pic.get(x, y));
        }
        out
    }
}

/// Whether a pixel is blank paper: white or clear.
fn blank(p: Pixel) -> bool {
    paper(p) == Pixel::WHITE
}

/// Tighten a selection around what's painted in it, as HyperCard's lasso
/// does: blank pixels reached from the selection's edge without crossing
/// paint drop out, and blank pixels inside a shape stay in. None when
/// nothing is painted there.
pub fn tighten(pic: &Picture, region: &Region) -> Option<Region> {
    let (l, t, r, b) = region.rect;
    let (w, h) = (r - l, b - t);
    if w <= 0 || h <= 0 {
        return None;
    }
    let idx = |x: i32, y: i32| (y * w + x) as usize;
    let inside = |x: i32, y: i32| region.contains(l + x, t + y);
    // Fill the blank surroundings from every blank pixel on the edge, or
    // next to a pixel outside the selection.
    let mut outside = vec![false; (w * h) as usize];
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !inside(x, y) {
                outside[idx(x, y)] = true;
                continue;
            }
            let at_edge = x == 0
                || y == 0
                || x == w - 1
                || y == h - 1
                || [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| !inside(x + dx, y + dy));
            if at_edge && blank(pic.get(l + x, t + y)) {
                stack.push((x, y));
            }
        }
    }
    let mut filled = vec![false; (w * h) as usize];
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= w || y >= h || filled[idx(x, y)] || !inside(x, y) || !blank(pic.get(l + x, t + y)) {
            continue;
        }
        filled[idx(x, y)] = true;
        outside[idx(x, y)] = true;
        stack.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
    }
    let kept: Vec<(i32, i32)> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| !outside[idx(x, y)]).map(|(x, y)| (l + x, t + y)).collect();
    Region::from_pixels(&kept)
}

/// What was painted since `before`: the pixels that differ, with what
/// they enclose, as a lasso would take an outlined shape. None when
/// nothing changed.
pub fn changed(before: &Picture, after: &Picture) -> Option<Region> {
    let (w, h) = (after.width.min(before.width) as i32, after.height.min(before.height) as i32);
    let pixels: Vec<(i32, i32)> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| before.get(x, y) != after.get(x, y)).collect();
    let region = Region::from_pixels(&pixels)?;
    // Fill in from the edges through what didn't change: what's left is
    // the change and the holes it closes.
    let (l, t, r, b) = region.rect;
    let (rw, rh) = (r - l, b - t);
    let mut outside = vec![false; (rw * rh) as usize];
    let mut stack: Vec<(i32, i32)> = (0..rw).flat_map(|x| [(x, 0), (x, rh - 1)]).chain((0..rh).flat_map(|y| [(0, y), (rw - 1, y)])).collect();
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= rw || y >= rh {
            continue;
        }
        let i = (y * rw + x) as usize;
        if outside[i] || region.contains(l + x, t + y) {
            continue;
        }
        outside[i] = true;
        stack.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
    }
    let mask: Vec<bool> = outside.iter().map(|o| !o).collect();
    Some(Region { rect: region.rect, lasso: None, mask: Some(std::sync::Arc::new(mask)) })
}

/// The smallest rectangle around what's painted in a rectangle: the
/// Select tool's Command-drag.
pub fn painted_bounds(pic: &Picture, rect: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
    let (l, t, r, b) = rect;
    let painted: Vec<(i32, i32)> = (t..b).flat_map(|y| (l..r).map(move |x| (x, y))).filter(|&(x, y)| !blank(pic.get(x, y))).collect();
    let region = Region::from_pixels(&painted)?;
    Some(region.rect)
}

/// The connected shape at (x, y): the lasso's Command-click. Paint
/// pixels joined side by side or corner to corner, with the blank pixels
/// they enclose.
pub fn shape_at(pic: &Picture, x: i32, y: i32) -> Option<Region> {
    let (w, h) = (pic.width as i32, pic.height as i32);
    if x < 0 || y < 0 || x >= w || y >= h || blank(pic.get(x, y)) {
        return None;
    }
    let mut seen = vec![false; (w * h) as usize];
    let mut stack = vec![(x, y)];
    let mut pixels = Vec::new();
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= w || y >= h || seen[(y * w + x) as usize] || blank(pic.get(x, y)) {
            continue;
        }
        seen[(y * w + x) as usize] = true;
        pixels.push((x, y));
        for dy in -1..=1 {
            for dx in -1..=1 {
                stack.push((x + dx, y + dy));
            }
        }
    }
    let shape = Region::from_pixels(&pixels)?;
    // Take in the holes: tighten the shape's rectangle, minus other paint.
    let mut whole = tighten(pic, &Region::new(shape.rect))?;
    let (l, t, r, _) = whole.rect;
    let mask: Vec<bool> = whole
        .mask
        .as_ref()?
        .iter()
        .enumerate()
        .map(|(i, &on)| {
            let (px, py) = (l + i as i32 % (r - l), t + i as i32 / (r - l));
            on && (blank(pic.get(px, py)) || shape.contains(px, py))
        })
        .collect();
    whole.mask = Some(std::sync::Arc::new(mask));
    Some(whole)
}

/// Options' Rotate, Slant, Distort and Perspective: ways of dragging a
/// rectangular selection's corners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reshape {
    Rotate,
    Slant,
    Distort,
    Perspective,
}

impl Reshape {
    pub const ALL: [Reshape; 4] = [Reshape::Rotate, Reshape::Slant, Reshape::Distort, Reshape::Perspective];

    pub fn name(self) -> &'static str {
        match self {
            Reshape::Rotate => "Rotate",
            Reshape::Slant => "Slant",
            Reshape::Distort => "Distort",
            Reshape::Perspective => "Perspective",
        }
    }

    pub fn parse(s: &str) -> Option<Reshape> {
        Reshape::ALL.into_iter().find(|r| r.name().eq_ignore_ascii_case(s.trim()))
    }

    /// The corners after dragging corner `i` (top left, top right, bottom
    /// right, bottom left) by (dx, dy) from where they were. `snap` turns
    /// a rotation in steps of 15°.
    pub fn drag(self, from: [(f32, f32); 4], i: usize, dx: f32, dy: f32, snap: bool) -> [(f32, f32); 4] {
        let mut c = from;
        // The corner beside it across (top or bottom edge), and up or down.
        let across = [1, 0, 3, 2][i];
        let down = [3, 2, 1, 0][i];
        let sideways = dx.abs() >= dy.abs();
        match self {
            Reshape::Rotate => {
                let (cx, cy) = (from.iter().map(|p| p.0).sum::<f32>() / 4.0, from.iter().map(|p| p.1).sum::<f32>() / 4.0);
                let (px, py) = from[i];
                let mut turn = (py + dy - cy).atan2(px + dx - cx) - (py - cy).atan2(px - cx);
                if snap {
                    let step = std::f32::consts::PI / 12.0;
                    turn = (turn / step).round() * step;
                }
                let (sin, cos) = turn.sin_cos();
                for (k, p) in c.iter_mut().enumerate() {
                    let (x, y) = (from[k].0 - cx, from[k].1 - cy);
                    *p = (cx + x * cos - y * sin, cy + x * sin + y * cos);
                }
            }
            // An edge slides along itself, leaning the sides.
            Reshape::Slant if sideways => {
                c[i].0 += dx;
                c[across].0 += dx;
            }
            Reshape::Slant => {
                c[i].1 += dy;
                c[down].1 += dy;
            }
            Reshape::Distort => c[i] = (c[i].0 + dx, c[i].1 + dy),
            // An edge widens or narrows about its middle.
            Reshape::Perspective if sideways => {
                c[i].0 += dx;
                c[across].0 -= dx;
            }
            Reshape::Perspective => {
                c[i].1 += dy;
                c[down].1 -= dy;
            }
        }
        c
    }
}

/// A picture drawn into four corners (top left, top right, bottom right,
/// bottom left): what Rotate, Slant, Distort and Perspective do. Each row
/// of the picture runs evenly from the left side to the right, so lines
/// keep their spacing (a bilinear map; for Rotate and Slant, the plain
/// turn or lean). Returns the picture and where its top left goes; pixels
/// outside the shape are clear.
pub fn warp(src: &Picture, corners: [(f32, f32); 4]) -> (Picture, (i32, i32)) {
    let l = corners.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor() as i32;
    let t = corners.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i32;
    let r = corners.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil() as i32;
    let b = corners.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i32;
    let (w, h) = ((r - l).clamp(1, 8192), (b - t).clamp(1, 8192));
    let mut out = Picture::new(w as u32, h as u32);
    let (sw, sh) = (src.width as f32, src.height as f32);
    for y in 0..h {
        for x in 0..w {
            // The pixel's center, back in the source.
            let q = ((l + x) as f32 + 0.5, (t + y) as f32 + 0.5);
            if let Some((u, v)) = inverse_bilinear(q, corners)
                && (0.0..1.0).contains(&u)
                && (0.0..1.0).contains(&v)
            {
                out.set(x, y, src.get((u * sw) as i32, (v * sh) as i32));
            }
        }
    }
    (out, (l, t))
}

/// Where a point falls in a four-cornered shape, as (across, down) from
/// 0 to 1: the inverse of the bilinear map (after Inigo Quilez), in f64
/// and with the stable form of the quadratic, since a turned rectangle
/// leaves its squared term all but zero.
fn inverse_bilinear(p: (f32, f32), corners: [(f32, f32); 4]) -> Option<(f32, f32)> {
    let [a, b, c, d] = corners.map(|(x, y)| (x as f64, y as f64));
    let p = (p.0 as f64, p.1 as f64);
    let cross = |p: (f64, f64), q: (f64, f64)| p.0 * q.1 - p.1 * q.0;
    let e = (b.0 - a.0, b.1 - a.1);
    let f = (d.0 - a.0, d.1 - a.1);
    let g = (a.0 - b.0 + c.0 - d.0, a.1 - b.1 + c.1 - d.1);
    let h = (p.0 - a.0, p.1 - a.1);
    let (k2, k1, k0) = (cross(g, f), cross(e, f) + cross(h, g), cross(h, e));
    // Across, given down, from whichever axis is steadier.
    let across = |v: f64| {
        let (dx, dy) = (e.0 + g.0 * v, e.1 + g.1 * v);
        if dx.abs() >= dy.abs() { (h.0 - f.0 * v) / dx } else { (h.1 - f.1 * v) / dy }
    };
    let inside = |u: f64, v: f64| (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v);
    let disc = k1 * k1 - 4.0 * k0 * k2;
    if disc < 0.0 {
        return None;
    }
    // The roots as q / k2 and k0 / q, so neither subtracts near equals.
    let q = -0.5 * (k1 + k1.signum() * disc.sqrt());
    let roots = [if q != 0.0 { k0 / q } else { f64::NAN }, if k2 != 0.0 { q / k2 } else { f64::NAN }];
    let mut best = None;
    for v in roots.into_iter().filter(|v| v.is_finite()) {
        let u = across(v);
        if inside(u, v) {
            return Some((u as f32, v as f32));
        }
        best.get_or_insert((u as f32, v as f32));
    }
    best
}

/// A picture stretched or shrunk to a new size, pixel by pixel.
pub fn scaled(src: &Picture, w: u32, h: u32) -> Picture {
    let mut out = Picture::new(w, h);
    if src.width == 0 || src.height == 0 {
        return out;
    }
    for y in 0..h {
        for x in 0..w {
            let sx = (x as u64 * src.width as u64 / w as u64) as i32;
            let sy = (y as u64 * src.height as u64 / h as u64) as i32;
            out.set(x as i32, y as i32, src.get(sx, sy));
        }
    }
    out
}

/// The Paint menu's transformations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintOp {
    Fill,
    Invert,
    Darken,
    Lighten,
    TraceEdges,
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
    Opaque,
    Transparent,
}

impl PaintOp {
    pub const ALL: [PaintOp; 11] = [
        PaintOp::Fill,
        PaintOp::Invert,
        PaintOp::Darken,
        PaintOp::Lighten,
        PaintOp::TraceEdges,
        PaintOp::RotateLeft,
        PaintOp::RotateRight,
        PaintOp::FlipHorizontal,
        PaintOp::FlipVertical,
        PaintOp::Opaque,
        PaintOp::Transparent,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PaintOp::Fill => "Fill",
            PaintOp::Invert => "Invert",
            PaintOp::Darken => "Darken",
            PaintOp::Lighten => "Lighten",
            PaintOp::TraceEdges => "Trace Edges",
            PaintOp::RotateLeft => "Rotate Left",
            PaintOp::RotateRight => "Rotate Right",
            PaintOp::FlipHorizontal => "Flip Horizontal",
            PaintOp::FlipVertical => "Flip Vertical",
            PaintOp::Opaque => "Opaque",
            PaintOp::Transparent => "Transparent",
        }
    }

    pub fn parse(s: &str) -> Option<PaintOp> {
        PaintOp::ALL.into_iter().find(|op| op.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// Apply a Paint menu operation to a region. Rotations turn the region's
/// rectangle about its top-left corner and return the new rectangle.
pub fn apply_op(pic: &mut Picture, region: &Region, op: PaintOp, ink: Ink) -> (i32, i32, i32, i32) {
    let (l, t, r, b) = region.rect;
    let pixels: Vec<(i32, i32)> = region.pixels().collect();
    // Painted: anything but white or clear.
    let painted = |p: Pixel| paper(p) != Pixel::WHITE;
    match op {
        PaintOp::Fill => pixels.iter().for_each(|&(x, y)| pic.set(x, y, ink.at(x, y))),
        // Each color becomes its opposite: black and white swap, as before.
        PaintOp::Invert => pixels.iter().for_each(|&(x, y)| {
            let Pixel([r, g, b, a]) = paper(pic.get(x, y));
            pic.set(x, y, Pixel([255 - r, 255 - g, 255 - b, a]));
        }),
        PaintOp::Darken | PaintOp::Lighten => {
            // Sprinkle one pixel in eight towards black or white.
            for &(x, y) in &pixels {
                if rand::random_range(0..8) == 0 {
                    pic.set(x, y, if op == PaintOp::Darken { Pixel::BLACK } else { Pixel::WHITE });
                }
            }
        }
        PaintOp::TraceEdges => {
            // Outline shapes: unpainted pixels next to paint take its color,
            // and the paint becomes white.
            let before = pic.clone();
            for &(x, y) in &pixels {
                let near = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .map(|(dx, dy)| before.get(x + dx, y + dy))
                    .find(|p| painted(*p));
                if painted(before.get(x, y)) {
                    pic.set(x, y, Pixel::WHITE);
                } else if let Some(p) = near {
                    pic.set(x, y, p);
                }
            }
        }
        PaintOp::FlipHorizontal | PaintOp::FlipVertical => {
            let lifted = region.lift(pic);
            let (w, h) = (lifted.width as i32, lifted.height as i32);
            for y in 0..h {
                for x in 0..w {
                    let p = lifted.get(x, y);
                    if p != Pixel::CLEAR {
                        let (nx, ny) = if op == PaintOp::FlipHorizontal { (w - 1 - x, y) } else { (x, h - 1 - y) };
                        pic.set(l + nx, t + ny, p);
                    }
                }
            }
        }
        PaintOp::RotateLeft | PaintOp::RotateRight => {
            let lifted = region.lift(pic);
            let (w, h) = (lifted.width as i32, lifted.height as i32);
            for y in 0..h {
                for x in 0..w {
                    let p = lifted.get(x, y);
                    if p != Pixel::CLEAR {
                        let (nx, ny) = if op == PaintOp::RotateRight { (h - 1 - y, x) } else { (y, w - 1 - x) };
                        pic.set(l + nx, t + ny, p);
                    }
                }
            }
            return (l, t, l + (b - t), t + (r - l));
        }
        PaintOp::Opaque => pixels.iter().for_each(|&(x, y)| {
            if pic.get(x, y) == Pixel::CLEAR {
                pic.set(x, y, Pixel::WHITE);
            }
        }),
        PaintOp::Transparent => pixels.iter().for_each(|&(x, y)| {
            if pic.get(x, y) == Pixel::WHITE {
                pic.set(x, y, Pixel::CLEAR);
            }
        }),
    }
    region.rect
}

/// The settings a paint tool draws with.
#[derive(Clone, Debug)]
pub struct Settings {
    pub line_size: u32,
    pub filled: bool,
    pub pattern: [u8; 8],
    pub brush: u32,
    pub poly_sides: u32,
    pub spray_size: u32,
    pub spray_density: u32,
    /// Draw Centered: lines and shapes grow from where the drag started.
    pub centered: bool,
    /// The pencil, lines, shape borders and text (`the penColor`).
    pub line_color: Pixel,
    /// The pattern's set bits: brush, spray, bucket and shape interiors
    /// (`the brushColor`).
    pub fill_color: Pixel,
}

impl Settings {
    pub fn ink(&self) -> Ink {
        Ink::Pattern { rows: self.pattern, color: self.fill_color }
    }

    pub fn outline(&self) -> Outline {
        Outline { size: self.line_size, color: self.line_color }
    }

    fn fill(&self) -> Option<Ink> {
        self.filled.then(|| self.ink())
    }
}

/// The tools that paint by dragging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragTool {
    Pencil,
    Brush,
    Eraser,
    Spray,
    Curve,
    Line,
    Rect,
    RoundRect,
    Oval,
    RegularPolygon,
}

/// Paint a whole drag of a tool from `a` to `b` (straight, for freehand
/// tools). Used for shape tools when the mouse is released, and by scripts.
pub fn apply_drag(pic: &mut Picture, tool: DragTool, a: (i32, i32), b: (i32, i32), s: &Settings) {
    let a = if s.centered { centered_start(tool, a, b) } else { a };
    match tool {
        DragTool::Pencil => {
            let color = pencil_color(pic, a.0, a.1, s.line_color);
            line(pic, a.0, a.1, b.0, b.1, 1, color);
        }
        DragTool::Brush => {
            let shape = brush_shape(s.brush);
            line_with(a.0, a.1, b.0, b.1, |x, y| stamp(pic, x, y, &shape, s.ink()));
        }
        DragTool::Eraser => line(pic, a.0, a.1, b.0, b.1, 16, Pixel::CLEAR),
        DragTool::Spray => line_with(a.0, a.1, b.0, b.1, |x, y| spray(pic, x, y, s.ink(), s.spray_size, s.spray_density)),
        DragTool::Curve | DragTool::Line => line(pic, a.0, a.1, b.0, b.1, s.line_size, s.line_color),
        DragTool::Rect => rect(pic, a.0, a.1, b.0, b.1, s.outline(), s.fill()),
        DragTool::RoundRect => round_rect(pic, a.0, a.1, b.0, b.1, s.outline(), s.fill()),
        DragTool::Oval => oval(pic, a.0, a.1, b.0, b.1, s.outline(), s.fill()),
        DragTool::RegularPolygon => {
            let pts = regular_polygon_points(a, b, s.poly_sides);
            polygon(pic, &pts, s.outline(), s.fill());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_constraints() {
        // Lines snap to multiples of 15°, keeping their length.
        assert_eq!(snap_15((0, 0), (100, 3)), (100, 0));
        assert_eq!(snap_15((0, 0), (98, 102)), (100, 100));
        assert_eq!(snap_15((10, 10), (10, -40)), (10, -40));
        let (x, y) = snap_15((0, 0), (85, 52));
        assert!((x, y) == (86, 50), "30°: {x},{y}");
        // Shapes become squares, the size of the longer side, either way.
        assert_eq!(square((10, 10), (40, 20)), (40, 40));
        assert_eq!(square((10, 10), (0, 50)), (-30, 50));
        // The freehand tools go straight across or straight down.
        assert_eq!(constrain(DragTool::Pencil, (5, 5), (30, 9)), (30, 5));
        assert_eq!(constrain(DragTool::Brush, (5, 5), (7, 40)), (5, 40));
        assert_eq!(Axis::of((1, 1), (1, 1)), None);
        assert_eq!(constrain(DragTool::Oval, (0, 0), (20, 8)), (20, 20));
        assert_eq!(constrain(DragTool::Curve, (0, 0), (20, 8)), (20, 8));
    }

    #[test]
    fn fill_bounded() {
        let mut p = Picture::new(20, 20);
        rect(&mut p, 2, 2, 10, 10, Outline::black(1), None);
        fill(&mut p, 5, 5, Ink::Solid(Pixel::BLACK));
        assert_eq!(p.get(5, 5), Pixel::BLACK);
        assert_eq!(p.get(15, 15), Pixel::CLEAR);
        fill(&mut p, 15, 15, Ink::Solid(Pixel::BLACK));
        assert_eq!(p.get(0, 0), Pixel::BLACK);
    }

    #[test]
    fn pattern_fill_covers_the_whole_region() {
        let mut p = Picture::new(16, 16);
        fill(&mut p, 0, 0, Ink::pattern(DEFAULT_PATTERNS[2]));
        // A 50% gray: alternating black and white everywhere.
        assert_eq!(p.get(0, 0), Pixel::BLACK);
        assert_eq!(p.get(1, 0), Pixel::WHITE);
        assert_eq!(p.get(15, 15), Pixel::BLACK);
        assert_eq!(p.get(14, 15), Pixel::WHITE);
    }

    #[test]
    fn shapes() {
        let mut p = Picture::new(30, 30);
        oval(&mut p, 0, 0, 29, 29, Outline::black(1), None);
        assert_eq!(p.get(15, 0), Pixel::BLACK);
        assert_eq!(p.get(15, 15), Pixel::CLEAR);
        line(&mut p, 0, 0, 29, 29, 1, Pixel::BLACK);
        assert_eq!(p.get(10, 10), Pixel::BLACK);
        let mut q = Picture::new(30, 30);
        rect(&mut q, 0, 0, 29, 29, Outline::black(2), Some(Ink::Solid(Pixel::WHITE)));
        assert_eq!((q.get(1, 15), q.get(15, 15)), (Pixel::BLACK, Pixel::WHITE));
    }

    #[test]
    fn polygons() {
        let tri = [(0, 0), (20, 0), (0, 20)];
        assert!(in_polygon(&tri, 2, 2));
        assert!(!in_polygon(&tri, 18, 18));
        let pts = regular_polygon_points((50, 50), (60, 50), 4);
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], (60, 50));
        let mut p = Picture::new(100, 100);
        polygon(&mut p, &pts, Outline::black(1), Some(Ink::Solid(Pixel::WHITE)));
        assert_eq!(p.get(50, 50), Pixel::WHITE);
        assert_eq!(p.get(60, 50), Pixel::BLACK);
    }

    #[test]
    fn paint_ops() {
        let mut p = Picture::new(4, 2);
        p.set(0, 0, Pixel::BLACK);
        let all = Region::all(&p);
        apply_op(&mut p, &all, PaintOp::Invert, Ink::Solid(Pixel::BLACK));
        assert_eq!((p.get(0, 0), p.get(1, 0)), (Pixel::WHITE, Pixel::BLACK));
        apply_op(&mut p, &all, PaintOp::FlipHorizontal, Ink::Solid(Pixel::BLACK));
        assert_eq!(p.get(3, 0), Pixel::WHITE);
        let mut q = Picture::new(4, 4);
        q.set(3, 0, Pixel::BLACK);
        let rect = apply_op(&mut q, &Region::new((0, 0, 4, 1)), PaintOp::RotateRight, Ink::Solid(Pixel::BLACK));
        assert_eq!(rect, (0, 0, 1, 4));
        assert_eq!(q.get(0, 3), Pixel::BLACK);
        assert_eq!(PaintOp::parse("trace edges"), Some(PaintOp::TraceEdges));
    }

    #[test]
    fn tight_selections() {
        // A 10×10 outlined square with a dot inside, on a 40×40 picture.
        let mut p = Picture::new(40, 40);
        rect(&mut p, 10, 10, 20, 20, Outline::black(1), None);
        p.set(15, 15, Pixel::BLACK);
        // Tightened from the whole picture: the square and what's inside
        // it, but not the blank around it.
        let r = tighten(&p, &Region::all(&p)).unwrap();
        assert_eq!(r.rect, (10, 10, 21, 21));
        assert!(r.contains(12, 12) && r.contains(15, 15) && !r.contains(5, 5));
        // Nothing painted: nothing to select.
        assert!(tighten(&Picture::new(8, 8), &Region::new((0, 0, 8, 8))).is_none());
        // The rectangle around what's painted.
        assert_eq!(painted_bounds(&p, (0, 0, 40, 40)), Some((10, 10, 21, 21)));
        // A shape on its own: the square and the blank inside it, but not
        // the separate dot in it, nor a second shape beside it.
        rect(&mut p, 25, 25, 30, 30, Outline::black(1), None);
        let shape = shape_at(&p, 10, 12).unwrap();
        assert_eq!(shape.rect, (10, 10, 21, 21));
        assert!(shape.contains(12, 12) && !shape.contains(15, 15) && !shape.contains(26, 25));
        assert!(shape_at(&p, 5, 5).is_none());
        // Its outline is the square's four sides.
        let edges = shape.mask_edges();
        assert!(edges.contains(&((10, 10), (21, 10))) && edges.contains(&((21, 10), (21, 21))), "{edges:?}");
    }

    #[test]
    fn new_element_and_scaling() {
        let before = Picture::new(20, 20);
        let mut after = before.clone();
        line(&mut after, 2, 3, 8, 3, 1, Pixel::BLACK);
        let r = changed(&before, &after).unwrap();
        assert_eq!(r.rect, (2, 3, 9, 4));
        // An outlined square's inside comes with it.
        let mut boxed = before.clone();
        rect(&mut boxed, 5, 5, 10, 10, Outline::black(1), None);
        assert!(changed(&before, &boxed).unwrap().contains(7, 7));
        assert!(changed(&before, &before).is_none());
        // Moving the region keeps its mask.
        assert!(r.translated(10, 10).contains(14, 13));
        // Twice the size, and back.
        let mut q = Picture::new(2, 1);
        q.set(0, 0, Pixel::BLACK);
        let big = scaled(&q, 4, 2);
        assert_eq!((big.get(1, 1), big.get(2, 0)), (Pixel::BLACK, Pixel::CLEAR));
        assert_eq!(scaled(&big, 2, 1), q);
    }

    #[test]
    fn reshaping() {
        // A 4×2 picture, black on the left half.
        let mut p = Picture::new(4, 2);
        for y in 0..2 {
            for x in 0..2 {
                p.set(x, y, Pixel::BLACK);
            }
        }
        let rect = [(10.0, 10.0), (14.0, 10.0), (14.0, 12.0), (10.0, 12.0)];
        // Into its own rectangle: unchanged.
        let (same, at) = warp(&p, rect);
        assert_eq!((same, at), (p.clone(), (10, 10)));
        // Twice as wide, as a distortion of the right-hand corners.
        let wide = Reshape::Distort.drag(Reshape::Distort.drag(rect, 1, 4.0, 0.0, false), 2, 4.0, 0.0, false);
        let (out, _) = warp(&p, wide);
        assert_eq!((out.width, out.get(3, 0), out.get(4, 0)), (8, Pixel::BLACK, Pixel::CLEAR));
        // A little turn keeps nearly everything (a near-zero squared term).
        let tilted = Reshape::Rotate.drag(rect, 1, 0.0, 1.0, false);
        let (out, _) = warp(&p, tilted);
        let inked = (0..out.height as i32).flat_map(|y| (0..out.width as i32).map(move |x| (x, y))).filter(|&(x, y)| out.get(x, y) == Pixel::BLACK).count();
        assert!(inked >= 3, "{inked}");
        // Rotated half a turn: black on the right.
        let turned = Reshape::Rotate.drag(rect, 0, 4.0, 2.0, false);
        let (out, _) = warp(&p, turned);
        assert_eq!((out.get(0, 0), out.get(3, 1)), (Pixel::CLEAR, Pixel::BLACK));
        // Slant moves the edge; perspective widens it both ways.
        let s = Reshape::Slant.drag(rect, 0, 3.0, 0.0, false);
        assert_eq!((s[0].0, s[1].0, s[3].0), (13.0, 17.0, 10.0));
        let q = Reshape::Perspective.drag(rect, 1, 2.0, 0.0, false);
        assert_eq!((q[0].0, q[1].0), (8.0, 16.0));
        assert_eq!(Reshape::parse("perspective"), Some(Reshape::Perspective));
    }

    #[test]
    fn brushes() {
        assert_eq!(brush_shape(6).len(), 4);
        assert_eq!(brush_shape(17), brush_shape(1));
        let mut p = Picture::new(20, 20);
        let s = Settings { line_size: 1, filled: false, pattern: [0xff; 8], brush: 10, poly_sides: 4, spray_size: SPRAY_SIZE, spray_density: SPRAY_DENSITY, centered: false, line_color: Pixel::BLACK, fill_color: Pixel::BLACK };
        apply_drag(&mut p, DragTool::Brush, (10, 10), (10, 10), &s);
        assert_eq!((p.get(6, 10), p.get(10, 11)), (Pixel::BLACK, Pixel::CLEAR));
    }

    #[test]
    fn painting_in_color() {
        let red = Pixel::rgba(220, 30, 30, 255);
        let blue = Pixel::rgba(30, 60, 220, 255);
        let s = Settings { line_size: 2, filled: true, pattern: DEFAULT_PATTERNS[2], brush: 1, poly_sides: 4, spray_size: SPRAY_SIZE, spray_density: SPRAY_DENSITY, centered: false, line_color: blue, fill_color: red };
        let mut p = Picture::new(40, 40);
        apply_drag(&mut p, DragTool::Rect, (0, 0), (39, 39), &s);
        // A blue border, and the gray pattern in red and white inside.
        assert_eq!(p.get(0, 20), blue);
        assert_eq!((p.get(10, 10), p.get(11, 10)), (red, Pixel::WHITE));
        // The bucket fills exactly one color's region.
        fill(&mut p, 11, 10, Ink::Solid(blue));
        assert_eq!((p.get(11, 10), p.get(10, 10)), (blue, red));
        // The pencil draws its color, or white when it starts on it.
        assert_eq!(pencil_color(&p, 0, 20, blue), Pixel::WHITE);
        assert_eq!(pencil_color(&p, 10, 10, blue), blue);
        // Black-and-white settings paint exactly as before.
        let mut q = Picture::new(4, 4);
        fill(&mut q, 0, 0, Ink::pattern(DEFAULT_PATTERNS[2]));
        assert!(!q.has_color());
    }

    #[test]
    fn color_paint_ops() {
        let mut p = Picture::new(3, 1);
        p.set(1, 0, Pixel::rgba(200, 100, 0, 255));
        let all = Region::all(&p);
        apply_op(&mut p, &all, PaintOp::TraceEdges, Ink::Solid(Pixel::BLACK));
        assert_eq!((p.get(0, 0), p.get(1, 0)), (Pixel::rgba(200, 100, 0, 255), Pixel::WHITE));
        apply_op(&mut p, &all, PaintOp::Invert, Ink::Solid(Pixel::BLACK));
        assert_eq!((p.get(0, 0), p.get(1, 0)), (Pixel::rgba(55, 155, 255, 255), Pixel::BLACK));
    }
}
