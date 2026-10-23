//! RandomCard: a recreation of Apple's HyperCard, written in Rust.

mod hypertalk;
mod import;
mod model;
mod runtime;
mod ui;

use model::Stack;

fn main() -> eframe::Result {
    let stack = match std::env::args().nth(1) {
        Some(path) => match import::load_any(std::path::Path::new(&path)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        None => Stack::demo(),
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RandomCard")
            .with_inner_size([stack.width as f32 + 220.0, stack.height as f32 + 150.0])
            .with_min_inner_size([480.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native("RandomCard", options, Box::new(|cc| Ok(Box::new(ui::App::new(cc, stack)))))
}
