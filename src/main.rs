//! RandomCard: a recreation of Apple's HyperCard, written in Rust.

mod hypertalk;
mod import;
mod model;
mod paint;
mod print;
mod runtime;
mod sound;
mod ui;

use model::Stack;

fn main() -> eframe::Result {
    // A stack given on the command line is opened once running, since it may
    // ask for a password.
    let open = std::env::args().nth(1).map(std::path::PathBuf::from);
    // Otherwise RandomCard opens Home, as HyperCard did.
    let stack = if open.is_some() { Stack::new("Untitled") } else { runtime::settings::open_home() };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RandomCard")
            .with_inner_size([stack.width as f32 + 220.0, stack.height as f32 + 150.0])
            .with_min_inner_size([480.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native("RandomCard", options, Box::new(|cc| Ok(Box::new(ui::App::new(cc, stack, open)))))
}
