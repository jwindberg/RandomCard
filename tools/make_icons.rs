//! Writes RandomCard's icon as PNGs for packaging: icon_<size>.png for 16
//! to 1024 pixels, and icon.ico for Windows (which build.rs embeds).
//!
//! Usage: cargo run --example make_icons -- <output folder>
//! (Cargo builds helper programs like this one as "examples"; see
//! Cargo.toml.)

#[path = "../src/icon.rs"]
mod icon;

#[path = "icon_files.rs"]
mod icon_files;
use icon_files::{ico, png};

fn main() {
    let folder = std::env::args().nth(1).expect("an output folder");
    std::fs::create_dir_all(&folder).expect("creating the folder");
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        let path = format!("{folder}/icon_{size}.png");
        std::fs::write(&path, png(size, &icon::rgba(size as usize))).expect("writing an icon");
    }
    // The Windows icon, as build.rs embeds it in the executable.
    let images: Vec<(u32, Vec<u8>)> =
        [16u32, 32, 48, 64, 128, 256].iter().map(|&s| (s, png(s, &icon::rgba(s as usize)))).collect();
    std::fs::write(format!("{folder}/icon.ico"), ico(&images)).expect("writing the Windows icon");
}
