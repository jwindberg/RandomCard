//! Typefaces from the system for writing RandomCard's own fonts lack. The
//! built-in fonts cover Latin text; Japanese (imported from MacJapanese,
//! typed, or put by a script) needs the system's Japanese font, which is
//! loaded the first time Japanese text opens in a stack or is drawn.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use eframe::egui::{self, FontData, FontDefinitions};

use crate::model::Stack;

const JAPANESE: &str = "randomcard::japanese";

/// Japanese text has been seen, so the fonts should have the fallback.
static WANTED: AtomicBool = AtomicBool::new(false);
/// The fonts in use have it.
static ADDED: AtomicBool = AtomicBool::new(false);

/// The system's Japanese fonts, best first: a file and the face in it.
#[cfg(target_os = "linux")]
const JAPANESE_FONTS: &[(&str, u32)] = &[
    // Debian and Ubuntu; Fedora; Arch.
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf", 0),
    ("/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf", 0),
];
#[cfg(target_os = "macos")]
const JAPANESE_FONTS: &[(&str, u32)] = &[
    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
    ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
    ("/Library/Fonts/Osaka.ttf", 0),
];
#[cfg(target_os = "windows")]
const JAPANESE_FONTS: &[(&str, u32)] = &[
    ("C:\\Windows\\Fonts\\YuGothM.ttc", 0),
    ("C:\\Windows\\Fonts\\meiryo.ttc", 0),
    ("C:\\Windows\\Fonts\\msgothic.ttc", 0),
];
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
const JAPANESE_FONTS: &[(&str, u32)] = &[];

fn japanese_font() -> Option<&'static FontData> {
    static FONT: OnceLock<Option<FontData>> = OnceLock::new();
    FONT.get_or_init(|| {
        JAPANESE_FONTS.iter().find_map(|(path, index)| {
            let bytes = std::fs::read(path).ok()?;
            Some(FontData::from_blob(Arc::new(bytes), *index))
        })
    })
    .as_ref()
}

/// Whether text has Japanese in it: kana, kanji or full-width forms.
fn is_japanese(text: &str) -> bool {
    text.chars().any(|c| matches!(c as u32, 0x3000..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0xff00..=0xffef))
}

/// Whether a stack shows Japanese: in its text or its names. Scripts
/// aren't shown until they run or are opened, which `note_text` sees.
fn stack_is_japanese(stack: &Stack) -> bool {
    let parts = stack.cards.iter().flat_map(|c| &c.parts).chain(stack.backgrounds.iter().flat_map(|b| &b.parts));
    stack.cards.iter().any(|c| is_japanese(&c.name) || c.bg_text.values().any(|t| is_japanese(t)))
        || stack.backgrounds.iter().any(|b| is_japanese(&b.name))
        || parts.into_iter().any(|p| is_japanese(&p.contents) || is_japanese(&p.name))
}

/// Note text about to be drawn: Japanese in it asks for the fallback.
pub fn note_text(text: &str) {
    if !WANTED.load(Ordering::Relaxed) && is_japanese(text) {
        WANTED.store(true, Ordering::Relaxed);
    }
}

/// Note text typed, entered with an input method or pasted, which may go
/// where `note_text` doesn't see it (the message box, dialogs).
pub fn note_input(ctx: &egui::Context) {
    ctx.input(|i| {
        for e in &i.events {
            match e {
                egui::Event::Text(t) | egui::Event::Paste(t) | egui::Event::Ime(egui::ImeEvent::Commit(t)) => note_text(t),
                _ => {}
            }
        }
    });
}

/// Whether text has asked for a fallback the fonts in use don't have yet
/// (and the system has one).
pub fn pending() -> bool {
    WANTED.load(Ordering::Relaxed) && !ADDED.load(Ordering::Relaxed) && japanese_font().is_some()
}

/// Add the fonts the writing needs, the stack's or any drawn so far, last
/// in every family, so they draw only what the others can't.
pub fn add_fallbacks(defs: &mut FontDefinitions, stack: &Stack) {
    if stack_is_japanese(stack) {
        WANTED.store(true, Ordering::Relaxed);
    }
    ADDED.store(false, Ordering::Relaxed);
    if !WANTED.load(Ordering::Relaxed) {
        return;
    }
    let Some(font) = japanese_font() else { return };
    ADDED.store(true, Ordering::Relaxed);
    defs.font_data.insert(JAPANESE.into(), font.clone().into());
    for family in defs.families.values_mut() {
        family.push(JAPANESE.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_text() {
        assert!(is_japanese("あなたの番です"));
        assert!(is_japanese("ｱ"));
        assert!(!is_japanese("café ¬ “quotes” …"));
    }
}
