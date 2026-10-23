//! Running stacks: the shared world state and the HyperTalk interpreter.

mod builtins;
mod commands;
pub mod dates;
pub mod interp;
mod objects;
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

pub fn start(stack: Stack) -> Runtime {
    let world = Arc::new(Mutex::new(World::new(stack)));
    let (events, event_rx) = channel();
    let (dialog_replies, dialog_rx) = channel();
    let abort = Arc::new(AtomicBool::new(false));
    let (w, a) = (world.clone(), abort.clone());
    std::thread::Builder::new()
        .name("hypertalk".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(move || interp::Interp::new(w, Some(dialog_rx), a).run(event_rx))
        .expect("can't start script thread");
    let _ = events.send(Event::Startup);
    Runtime { world, events, dialog_replies, abort }
}
