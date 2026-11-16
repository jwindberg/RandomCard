//! RandomCard: a recreation of Apple's HyperCard, written in Rust.

mod hypertalk;
mod icon;
mod import;
mod model;
mod paint;
mod print;
mod runtime;
mod sound;
mod ui;
mod video;
mod web;

use model::Stack;

fn main() -> eframe::Result {
    // A stack given on the command line is opened once running, since it may
    // ask for a password.
    let open = std::env::args().nth(1).map(std::path::PathBuf::from);
    // Otherwise RandomCard opens Home, as HyperCard did.
    let stack = if open.is_some() { Stack::new("Untitled") } else { runtime::settings::open_home() };
    let mut options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RandomCard")
            .with_inner_size([stack.width as f32 + ui::SIDE_ROOM, stack.height as f32 + 150.0])
            .with_min_inner_size([480.0, 360.0])
            .with_icon(eframe::egui::IconData { rgba: icon::rgba(128), width: 128, height: 128 }),
        ..Default::default()
    };
    // Ask the GPU only for limits it has. egui's defaults want 16 inter-stage
    // shader variables, the Raspberry Pi 5's GPU offers 15, and the window
    // never opens.
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        let egui_descriptor = setup.device_descriptor.clone();
        setup.device_descriptor = std::sync::Arc::new(move |adapter| {
            let mut descriptor = egui_descriptor(adapter);
            descriptor.required_limits = descriptor.required_limits.or_worse_values_from(&adapter.limits());
            descriptor
        });
    }
    // Don't wait on vsync to present. The palette and stack windows are drawn
    // inside the main window's frame, and on Wayland a vsynced present waits
    // for the compositor's go-ahead, which never comes for a window it isn't
    // showing (hidden, covered, off-screen). One such window would then stall
    // every frame and the whole app stops answering clicks. egui only redraws
    // when something changes, so this costs nothing while idle.
    options.wgpu_options.surface.present_mode = eframe::wgpu::PresentMode::AutoNoVsync;
    // On a Mac, RandomCard installs its own native menus.
    #[cfg(target_os = "macos")]
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_default_menu(false);
        }));
    }
    eframe::run_native("RandomCard", options, Box::new(|cc| Ok(Box::new(ui::App::new(cc, stack, open)))))
}
