//! Raster painting on 1-bit-plus-transparency pictures, shared by the paint
//! tools and by scripts (`drag from … to …`, `click at`, `type`).

mod text;

use crate::model::{Picture, Pixel};
pub use text::draw_text;

/// What paint is laid down with: a solid pixel value or an 8×8 pattern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Solid(Pixel),
    Pattern([u8; 8]),
}

impl Ink {
    pub fn at(&self, x: i32, y: i32) -> Pixel {
        match self {
            Ink::Solid(p) => *p,
            Ink::Pattern(rows) => {
                if rows[y.rem_euclid(8) as usize] & (0x80 >> x.rem_euclid(8)) != 0 {
                    Pixel::Black
                } else {
                    Pixel::White
                }
            }
        }
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
/// band black, and the interior with `fill` when given.
fn shape(
    pic: &mut Picture,
    (l, t, r, b): (i32, i32, i32, i32),
    size: u32,
    fill: Option<Ink>,
    inside: impl Fn(i32, i32, i32) -> bool,
) {
    let s = size.max(1) as i32;
    for y in t..=b {
        for x in l..=r {
            if !inside(x, y, 0) {
                continue;
            }
            if !inside(x, y, s) {
                pic.set(x, y, Pixel::Black);
            } else if let Some(ink) = fill {
                pic.set(x, y, ink.at(x, y));
            }
        }
    }
}

pub fn rect(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, size: u32, fill: Option<Ink>) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    shape(pic, (l, t, r, b), size, fill, |x, y, s| x >= l + s && x <= r - s && y >= t + s && y <= b - s);
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

pub fn round_rect(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, size: u32, fill: Option<Ink>) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    shape(pic, (l, t, r, b), size, fill, |x, y, s| in_round_rect(x, y, l + s, t + s, r - s, b - s, 10 - s));
}

/// Whether pixel (x, y) lies inside the ellipse filling pixels l..=r, t..=b.
fn in_ellipse(x: i32, y: i32, l: f32, t: f32, r: f32, b: f32) -> bool {
    let (cx, cy) = ((l + r + 1.0) / 2.0, (t + b + 1.0) / 2.0);
    let (rx, ry) = (((r - l + 1.0) / 2.0).max(0.5), ((b - t + 1.0) / 2.0).max(0.5));
    let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
    dx * dx + dy * dy <= 1.0
}

pub fn oval(pic: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32, size: u32, fill: Option<Ink>) {
    let (l, t, r, b) = norm(x0, y0, x1, y1);
    let (lf, tf, rf, bf) = (l as f32, t as f32, r as f32, b as f32);
    shape(pic, (l, t, r, b), size, fill, |x, y, s| {
        let s = s as f32;
        in_ellipse(x, y, lf + s, tf + s, rf - s, bf - s)
    });
}

/// Even-odd point-in-polygon test at the pixel's centre.
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

/// A closed polygon: filled with `fill` if given, outlined in black.
pub fn polygon(pic: &mut Picture, points: &[(i32, i32)], size: u32, fill: Option<Ink>) {
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
        line(pic, a.0, a.1, b.0, b.1, size, Pixel::Black);
    }
}

/// A regular polygon dragged out from its centre to a corner.
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

/// Bucket fill: the region of same-coloured pixels (clear counts as white)
/// around (x, y), painted with `ink`.
pub fn fill(pic: &mut Picture, x: i32, y: i32, ink: Ink) {
    let (w, h) = (pic.width as i32, pic.height as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let target = is_dark(pic.get(x, y));
    // Find the whole region before painting, since a pattern adds both colours.
    let mut seen = vec![false; (w * h) as usize];
    let mut region = Vec::new();
    let mut stack = vec![(x, y)];
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= w || y >= h {
            continue;
        }
        let i = (y * w + x) as usize;
        if seen[i] || is_dark(pic.get(x, y)) != target {
            continue;
        }
        seen[i] = true;
        region.push((x, y));
        stack.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
    }
    // Filling a solid area with its own colour does nothing.
    if ink == Ink::Solid(if target { Pixel::Black } else { Pixel::White }) {
        return;
    }
    for (x, y) in region {
        pic.set(x, y, ink.at(x, y));
    }
}

pub fn spray(pic: &mut Picture, x: i32, y: i32, ink: Ink) {
    for _ in 0..12 {
        let dx: i32 = rand::random_range(-8..=8);
        let dy: i32 = rand::random_range(-8..=8);
        if dx * dx + dy * dy <= 64 {
            // Spraying a pattern reveals it dot by dot.
            let p = ink.at(x + dx, y + dy);
            if p == Pixel::Black {
                pic.set(x + dx, y + dy, p);
            }
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

/// A selected area: a rectangle, optionally narrowed by a lasso outline.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub rect: (i32, i32, i32, i32),
    pub lasso: Option<Vec<(i32, i32)>>,
}

impl Region {
    pub fn all(pic: &Picture) -> Region {
        Region { rect: (0, 0, pic.width as i32, pic.height as i32), lasso: None }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (l, t, r, b) = self.rect;
        x >= l && x < r && y >= t && y < b && self.lasso.as_ref().is_none_or(|pts| in_polygon(pts, x, y))
    }

    fn pixels(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (l, t, r, b) = self.rect;
        (t..b).flat_map(move |y| (l..r).map(move |x| (x, y))).filter(|(x, y)| self.contains(*x, *y))
    }

    /// Lift the selection out of the picture, leaving it clear.
    pub fn lift(&self, pic: &mut Picture) -> Picture {
        let (l, t, r, b) = self.rect;
        let mut out = Picture::new((r - l).max(0) as u32, (b - t).max(0) as u32);
        for (x, y) in self.pixels().collect::<Vec<_>>() {
            out.set(x - l, y - t, pic.get(x, y));
            pic.set(x, y, Pixel::Clear);
        }
        out
    }
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
    let dark = |p: Pixel| p == Pixel::Black;
    match op {
        PaintOp::Fill => pixels.iter().for_each(|&(x, y)| pic.set(x, y, ink.at(x, y))),
        PaintOp::Invert => pixels.iter().for_each(|&(x, y)| {
            pic.set(x, y, if dark(pic.get(x, y)) { Pixel::White } else { Pixel::Black });
        }),
        PaintOp::Darken | PaintOp::Lighten => {
            // Sprinkle one pixel in eight towards black or white.
            for &(x, y) in &pixels {
                if rand::random_range(0..8) == 0 {
                    pic.set(x, y, if op == PaintOp::Darken { Pixel::Black } else { Pixel::White });
                }
            }
        }
        PaintOp::TraceEdges => {
            // Outline shapes: pixels next to black become black, black becomes white.
            let before = pic.clone();
            for &(x, y) in &pixels {
                let near = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| dark(before.get(x + dx, y + dy)));
                let p = before.get(x, y);
                if dark(p) {
                    pic.set(x, y, Pixel::White);
                } else if near {
                    pic.set(x, y, Pixel::Black);
                }
            }
        }
        PaintOp::FlipHorizontal | PaintOp::FlipVertical => {
            let lifted = region.lift(pic);
            let (w, h) = (lifted.width as i32, lifted.height as i32);
            for y in 0..h {
                for x in 0..w {
                    let p = lifted.get(x, y);
                    if p != Pixel::Clear {
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
                    if p != Pixel::Clear {
                        let (nx, ny) = if op == PaintOp::RotateRight { (h - 1 - y, x) } else { (y, w - 1 - x) };
                        pic.set(l + nx, t + ny, p);
                    }
                }
            }
            return (l, t, l + (b - t), t + (r - l));
        }
        PaintOp::Opaque => pixels.iter().for_each(|&(x, y)| {
            if pic.get(x, y) == Pixel::Clear {
                pic.set(x, y, Pixel::White);
            }
        }),
        PaintOp::Transparent => pixels.iter().for_each(|&(x, y)| {
            if pic.get(x, y) == Pixel::White {
                pic.set(x, y, Pixel::Clear);
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
}

impl Settings {
    pub fn ink(&self) -> Ink {
        Ink::Pattern(self.pattern)
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
    match tool {
        DragTool::Pencil => {
            let color = if is_dark(pic.get(a.0, a.1)) { Pixel::White } else { Pixel::Black };
            line(pic, a.0, a.1, b.0, b.1, 1, color);
        }
        DragTool::Brush => {
            let shape = brush_shape(s.brush);
            line_with(a.0, a.1, b.0, b.1, |x, y| stamp(pic, x, y, &shape, s.ink()));
        }
        DragTool::Eraser => line(pic, a.0, a.1, b.0, b.1, 16, Pixel::Clear),
        DragTool::Spray => line_with(a.0, a.1, b.0, b.1, |x, y| spray(pic, x, y, s.ink())),
        DragTool::Curve | DragTool::Line => line(pic, a.0, a.1, b.0, b.1, s.line_size, Pixel::Black),
        DragTool::Rect => rect(pic, a.0, a.1, b.0, b.1, s.line_size, s.fill()),
        DragTool::RoundRect => round_rect(pic, a.0, a.1, b.0, b.1, s.line_size, s.fill()),
        DragTool::Oval => oval(pic, a.0, a.1, b.0, b.1, s.line_size, s.fill()),
        DragTool::RegularPolygon => {
            let pts = regular_polygon_points(a, b, s.poly_sides);
            polygon(pic, &pts, s.line_size, s.fill());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_bounded() {
        let mut p = Picture::new(20, 20);
        rect(&mut p, 2, 2, 10, 10, 1, None);
        fill(&mut p, 5, 5, Ink::Solid(Pixel::Black));
        assert_eq!(p.get(5, 5), Pixel::Black);
        assert_eq!(p.get(15, 15), Pixel::Clear);
        fill(&mut p, 15, 15, Ink::Solid(Pixel::Black));
        assert_eq!(p.get(0, 0), Pixel::Black);
    }

    #[test]
    fn pattern_fill_covers_the_whole_region() {
        let mut p = Picture::new(16, 16);
        fill(&mut p, 0, 0, Ink::Pattern(DEFAULT_PATTERNS[2]));
        // A 50% gray: alternating black and white everywhere.
        assert_eq!(p.get(0, 0), Pixel::Black);
        assert_eq!(p.get(1, 0), Pixel::White);
        assert_eq!(p.get(15, 15), Pixel::Black);
        assert_eq!(p.get(14, 15), Pixel::White);
    }

    #[test]
    fn shapes() {
        let mut p = Picture::new(30, 30);
        oval(&mut p, 0, 0, 29, 29, 1, None);
        assert_eq!(p.get(15, 0), Pixel::Black);
        assert_eq!(p.get(15, 15), Pixel::Clear);
        line(&mut p, 0, 0, 29, 29, 1, Pixel::Black);
        assert_eq!(p.get(10, 10), Pixel::Black);
        let mut q = Picture::new(30, 30);
        rect(&mut q, 0, 0, 29, 29, 2, Some(Ink::Solid(Pixel::White)));
        assert_eq!((q.get(1, 15), q.get(15, 15)), (Pixel::Black, Pixel::White));
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
        polygon(&mut p, &pts, 1, Some(Ink::Solid(Pixel::White)));
        assert_eq!(p.get(50, 50), Pixel::White);
        assert_eq!(p.get(60, 50), Pixel::Black);
    }

    #[test]
    fn paint_ops() {
        let mut p = Picture::new(4, 2);
        p.set(0, 0, Pixel::Black);
        let all = Region::all(&p);
        apply_op(&mut p, &all, PaintOp::Invert, Ink::Solid(Pixel::Black));
        assert_eq!((p.get(0, 0), p.get(1, 0)), (Pixel::White, Pixel::Black));
        apply_op(&mut p, &all, PaintOp::FlipHorizontal, Ink::Solid(Pixel::Black));
        assert_eq!(p.get(3, 0), Pixel::White);
        let mut q = Picture::new(4, 4);
        q.set(3, 0, Pixel::Black);
        let rect = apply_op(&mut q, &Region { rect: (0, 0, 4, 1), lasso: None }, PaintOp::RotateRight, Ink::Solid(Pixel::Black));
        assert_eq!(rect, (0, 0, 1, 4));
        assert_eq!(q.get(0, 3), Pixel::Black);
        assert_eq!(PaintOp::parse("trace edges"), Some(PaintOp::TraceEdges));
    }

    #[test]
    fn brushes() {
        assert_eq!(brush_shape(6).len(), 4);
        assert_eq!(brush_shape(17), brush_shape(1));
        let mut p = Picture::new(20, 20);
        let s = Settings { line_size: 1, filled: false, pattern: [0xff; 8], brush: 10, poly_sides: 4 };
        apply_drag(&mut p, DragTool::Brush, (10, 10), (10, 10), &s);
        assert_eq!((p.get(6, 10), p.get(10, 11)), (Pixel::Black, Pixel::Clear));
    }
}
