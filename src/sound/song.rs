//! Songs: soundtracker modules (MOD, XM, S3M, IT) a stack carries, as
//! HyperCard games played music with the PlayMOD external. They play on a
//! channel of their own beside sound effects, looping until stopped.

use std::collections::HashMap;
use std::num::NonZero;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use xmrs::prelude::Module;
use xmrsplayer::prelude::XmrsPlayer;

/// A module parsed once and kept for the program's life: the player
/// borrows it while playing, and a stack has only a few songs.
fn parsed(bytes: &[u8]) -> Result<&'static Module, String> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    static CACHE: OnceLock<Mutex<HashMap<u64, &'static Module>>> = OnceLock::new();
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    let key = h.finish();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|p| p.into_inner());
    if let Some(m) = cache.get(&key) {
        return Ok(m);
    }
    let module = Module::load(bytes).map_err(|e| format!("Not a song RandomCard can play ({e:?})"))?;
    let module: &'static Module = Box::leak(Box::new(module));
    cache.insert(key, module);
    Ok(module)
}

/// Whether these bytes are a module RandomCard can play.
pub fn check(bytes: &[u8]) -> Result<(), String> {
    parsed(bytes).map(|_| ())
}

/// A song playing: stereo samples, looping forever.
pub struct SongSource {
    player: XmrsPlayer<'static>,
    rate: NonZero<u32>,
}

impl SongSource {
    pub fn new(bytes: &[u8], rate: u32) -> Result<SongSource, String> {
        let module = parsed(bytes)?;
        let mut player = XmrsPlayer::new(module, rate, 0);
        // 0: loop forever, as game music does until it's stopped.
        player.set_max_loop_count(0);
        Ok(SongSource { player, rate: NonZero::new(rate).ok_or("No sample rate")? })
    }
}

impl Iterator for SongSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.player.next().map(|s| s as f32 / 32768.0)
    }
}

impl rodio::Source for SongSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(2).unwrap()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
