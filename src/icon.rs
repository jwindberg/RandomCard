//! RandomCard's icon: a stack of cards with a die on the front one, in
//! black and white on a 32 × 32 grid, as a HyperCard icon would be drawn.
//! The window uses it, and `examples/make_icons.rs` writes it out as PNGs
//! for packaging.

pub const SIZE: usize = 32;

/// The icon's pixels by row: `None` outside it, `Some(true)` for black and
/// `Some(false)` for white.
pub fn pixels() -> Vec<Vec<Option<bool>>> {
    let mut px = vec![vec![None; SIZE]; SIZE];
    // Three cards, back to front, each a white face in a black frame.
    for (left, top) in [(9, 2), (6, 5), (3, 8)] {
        let (right, bottom) = (left + 20, top + 21);
        for (y, row) in px.iter_mut().enumerate().take(bottom + 1).skip(top) {
            for (x, p) in row.iter_mut().enumerate().take(right + 1).skip(left) {
                *p = Some(x == left || x == right || y == top || y == bottom);
            }
        }
    }
    // A die on the front card, showing five, with its corners rounded off.
    let (dl, dt, dr, db) = (6, 12, 19, 25);
    for (y, row) in px.iter_mut().enumerate().take(db + 1).skip(dt) {
        for (x, p) in row.iter_mut().enumerate().take(dr + 1).skip(dl) {
            let corner = (x == dl || x == dr) && (y == dt || y == db);
            if !corner {
                *p = Some(x == dl || x == dr || y == dt || y == db);
            }
        }
    }
    for (cx, cy) in [(9, 15), (17, 15), (13, 19), (9, 23), (17, 23)] {
        for (x, y) in [(cx - 1, cy - 1), (cx, cy - 1), (cx - 1, cy), (cx, cy)] {
            px[y][x] = Some(true);
        }
    }
    px
}

/// The icon as RGBA, `size` pixels square, scaled up in whole pixels so it
/// stays crisp, and transparent outside the cards.
pub fn rgba(size: usize) -> Vec<u8> {
    let px = pixels();
    let mut out = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            out.extend_from_slice(match px[y * SIZE / size][x * SIZE / size] {
                None => &[0, 0, 0, 0],
                Some(true) => &[0, 0, 0, 255],
                Some(false) => &[255, 255, 255, 255],
            });
        }
    }
    out
}
