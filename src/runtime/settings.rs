//! RandomCard's own preferences (HyperCard kept these in the Home stack),
//! stored in `~/.config/randomcard/settings.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("randomcard").join("settings.json"))
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
