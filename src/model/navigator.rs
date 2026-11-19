//! HyperCard's Navigator palette, which came with the application rather
//! than a stack: `palette "Navigator"` (Power Tools' `nav`, Readymade
//! Buttons). RandomCard draws its own, with the same buttons sending the
//! same menu commands: Back, Home, Help and Recent; First, Prev, Next and
//! Last; Find, Message and Next Window.

use super::demo::{circle, frame, hline, line, vline};
use super::{Palette, PaletteButton, Picture, Pixel, Rect};

/// The palette's picture and buttons.
pub fn navigator() -> Palette {
    let (w, h) = (95, 65);
    let mut p = Picture::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            p.set(x, y, Pixel::WHITE);
        }
    }
    // Three rows: four cells, four cells, then three.
    let cells: [(i32, i32, i32, i32, &str); 11] = [
        (0, 0, 24, 22, "Back"),
        (24, 0, 48, 22, "Home"),
        (48, 0, 71, 22, "Help"),
        (71, 0, 95, 22, "Recent"),
        (0, 22, 24, 44, "First"),
        (24, 22, 48, 44, "Prev"),
        (48, 22, 71, 44, "Next"),
        (71, 22, 95, 44, "Last"),
        (0, 44, 35, 65, "Find…"),
        (35, 44, 71, 65, "Message"),
        (71, 44, 95, 65, "Next Window"),
    ];
    frame(&mut p, 0, 0, w as i32 - 1, h as i32 - 1);
    for &(l, t, r, b, _) in &cells {
        frame(&mut p, l, t, r.min(w as i32 - 1), b.min(h as i32 - 1));
    }
    let centre = |(l, t, r, b): (i32, i32, i32, i32)| ((l + r) / 2, (t + b) / 2);
    let at = |i: usize| centre((cells[i].0, cells[i].1, cells[i].2, cells[i].3));
    // Back: an arrow turning back on itself.
    let (x, y) = at(0);
    arrow(&mut p, x - 6, y + 3, -1);
    vline(&mut p, x + 5, y - 5, y + 3);
    hline(&mut p, x - 1, x + 5, y + 3);
    // Home: a house.
    let (x, y) = at(1);
    line(&mut p, x - 7, y, x, y - 7);
    line(&mut p, x, y - 7, x + 7, y);
    frame(&mut p, x - 5, y, x + 5, y + 7);
    frame(&mut p, x - 1, y + 3, x + 1, y + 7);
    // Help: a question mark in a diamond.
    let (x, y) = at(2);
    line(&mut p, x, y - 8, x + 8, y);
    line(&mut p, x + 8, y, x, y + 8);
    line(&mut p, x, y + 8, x - 8, y);
    line(&mut p, x - 8, y, x, y - 8);
    circle(&mut p, x, y - 2, 2, false);
    vline(&mut p, x, y, y + 2);
    p.set(x, y + 4, Pixel::BLACK);
    // Recent: little cards in rows.
    let (x, y) = at(3);
    frame(&mut p, x - 8, y - 6, x + 8, y + 6);
    for row in [y - 3, y, y + 3] {
        for col in [x - 6, x - 2, x + 2, x + 5] {
            hline(&mut p, col, col + 2, row);
        }
    }
    // First and Last: arrows against a bar; Prev and Next: arrows.
    let (x, y) = at(4);
    vline(&mut p, x - 7, y - 6, y + 6);
    arrow(&mut p, x - 5, y, -1);
    let (x, y) = at(5);
    arrow(&mut p, x - 4, y, -1);
    let (x, y) = at(6);
    arrow(&mut p, x + 4, y, 1);
    let (x, y) = at(7);
    vline(&mut p, x + 7, y - 6, y + 6);
    arrow(&mut p, x + 5, y, 1);
    // Find: a magnifying glass over a line of text.
    let (x, y) = at(8);
    frame(&mut p, x - 12, y - 4, x + 12, y + 4);
    circle(&mut p, x, y - 1, 4, false);
    line(&mut p, x + 3, y + 2, x + 7, y + 7);
    // Message: the message box.
    let (x, y) = at(9);
    frame(&mut p, x - 12, y - 4, x + 12, y + 4);
    hline(&mut p, x - 10, x + 10, y + 1);
    // Next Window: two windows, one in front.
    let (x, y) = at(10);
    frame(&mut p, x - 7, y - 6, x + 3, y + 2);
    frame(&mut p, x - 3, y - 2, x + 7, y + 6);
    for yy in y - 1..y + 6 {
        for xx in x - 2..x + 7 {
            p.set(xx, yy, Pixel::WHITE);
        }
    }
    frame(&mut p, x - 3, y - 2, x + 7, y + 6);
    let buttons = cells
        .iter()
        .map(|&(l, t, r, b, item)| PaletteButton { rect: Rect::new(l + 1, t + 1, r, b), message: format!("doMenu \"{item}\"") })
        .collect();
    Palette { name: "Navigator".into(), picture: p, buttons }
}

/// An arrow pointing left (`dir` -1) or right (1), its point at x, y.
fn arrow(p: &mut Picture, x: i32, y: i32, dir: i32) {
    line(p, x, y, x - dir * 5, y - 5);
    line(p, x, y, x - dir * 5, y + 5);
    vline(p, x - dir * 5, y - 5, y - 2);
    vline(p, x - dir * 5, y + 2, y + 5);
    hline(p, (x - dir * 5).min(x - dir * 10), (x - dir * 5).max(x - dir * 10), y - 2);
    hline(p, (x - dir * 5).min(x - dir * 10), (x - dir * 5).max(x - dir * 10), y + 2);
    vline(p, x - dir * 10, y - 2, y + 2);
}
