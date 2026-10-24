//! Running stacks: the shared world state and the HyperTalk interpreter.

mod builtins;
mod commands;
pub mod dates;
pub mod interp;
mod menu_commands;
pub mod menus;
mod objects;
pub mod settings;
pub mod undo;
pub mod world;

#[cfg(test)]
mod tests;

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use crate::model::Stack;
use world::{DialogReply, Event, Shared, World};

/// Handles the UI keeps for talking to the script thread.
pub struct Runtime {
    pub world: Shared,
    pub events: Sender<Event>,
    pub dialog_replies: Sender<DialogReply>,
    pub abort: Arc<AtomicBool>,
}

/// Start the script thread with a stack, or with a file to open (which may
/// need a password, so it's opened on the script thread).
pub fn start(stack: Stack, open: Option<std::path::PathBuf>, user_level: u8) -> Runtime {
    let mut world = World::new(stack);
    world.user_level = user_level.clamp(1, 5);
    let world = Arc::new(Mutex::new(world));
    let (events, event_rx) = channel();
    let (dialog_replies, dialog_rx) = channel();
    let abort = Arc::new(AtomicBool::new(false));
    let (w, a) = (world.clone(), abort.clone());
    std::thread::Builder::new()
        .name("hypertalk".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(move || {
            let mut interp = interp::Interp::new(w, Some(dialog_rx), a);
            interp.audio = crate::sound::Player::new();
            interp.run(event_rx)
        })
        .expect("can't start script thread");
    let _ = events.send(match open {
        Some(path) => Event::OpenFile(path),
        None => Event::Startup,
    });
    Runtime { world, events, dialog_replies, abort }
}
