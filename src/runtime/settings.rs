//! RandomCard's own preferences (HyperCard kept these in the Home stack),
//! stored in `~/.config/randomcard/settings.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::Stack;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The user level RandomCard runs at: 1 Browsing … 5 Scripting.
    pub user_level: u8,
    /// Whether the tool panel is shown (when the user level allows tools).
    pub show_tools: bool,
}

impl Default for Settings {
    fn default() -> Self {
        // Like the full HyperCard: scripting level, tools at hand.
        Settings { user_level: 5, show_tools: true }
    }
}

/// RandomCard's folder in the user's config directory.
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("randomcard"))
}

fn path() -> Option<PathBuf> {
    Some(config_dir()?.join("settings.json"))
}

/// Where the Home stack lives.
pub fn home_path() -> Option<PathBuf> {
    Some(config_dir()?.join("Home.rcstack"))
}

/// The Home stack, made the first time RandomCard runs. A Home file that
/// can't be read is left alone, and a fresh Home is used without a file.
pub fn open_home() -> Stack {
    let Some(path) = home_path() else { return Stack::home() };
    if path.exists() {
        return Stack::load(&path).unwrap_or_else(|e| {
            eprintln!("RandomCard: {e}; using a new Home stack for now.");
            Stack::home()
        });
    }
    let mut home = Stack::home();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if home.save(&path).is_ok() {
        home.path = Some(path);
    }
    home
}

impl Settings {
    pub fn load() -> Settings {
        let mut s: Settings = path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        s.user_level = s.user_level.clamp(1, 5);
        s
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, json);
        }
    }
}

/// HyperCard's names for the user levels.
pub fn level_name(level: u8) -> &'static str {
    match level {
        1 => "Browsing",
        2 => "Typing",
        3 => "Painting",
        4 => "Authoring",
        _ => "Scripting",
    }
}
