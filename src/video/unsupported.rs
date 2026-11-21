//! Systems RandomCard can't play movies on yet.

use std::path::Path;
use std::sync::Arc;

use super::Frame;

pub struct Player;

impl Player {
    pub fn open(_path: &Path) -> Result<Player, String> {
        Err("RandomCard can't play movies on this system yet".into())
    }

    pub fn open_url(_url: &str) -> Result<Player, String> {
        Err("RandomCard can't play movies on this system yet".into())
    }
    pub fn size(&self) -> (u32, u32) {
        (0, 0)
    }
    pub fn duration(&self) -> f64 {
        0.0
    }
    pub fn time(&self) -> f64 {
        0.0
    }
    pub fn seek(&self, _secs: f64) {}
    pub fn set_rate(&self, _rate: f64) {}
    pub fn set_volume(&self, _level: f64) {}
    pub fn frame(&self) -> Option<Arc<Frame>> {
        None
    }
    pub fn finished(&self) -> bool {
        false
    }
}
