//! The stack data model: stacks contain backgrounds and cards; backgrounds and
//! cards each hold a picture layer plus buttons and fields ("parts").

mod demo;
pub mod password;
mod picture;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use picture::{Picture, Pixel};

pub type Id = u32;

pub const DEFAULT_WIDTH: u32 = 512;
pub const DEFAULT_HEIGHT: u32 = 342;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect { left: left.min(right), top: top.min(bottom), right: left.max(right), bottom: top.max(bottom) }
    }
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
    pub fn center(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
    pub fn translate(&self, dx: i32, dy: i32) -> Rect {
        Rect { left: self.left + dx, top: self.top + dy, right: self.right + dx, bottom: self.bottom + dy }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartKind {
    Button,
    Field,
}

impl PartKind {
    pub fn name(self) -> &'static str {
        match self {
            PartKind::Button => "button",
            PartKind::Field => "field",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Layer {
    Card,
    Background,
}

impl Layer {
    pub fn name(self) -> &'static str {
        match self {
            Layer::Card => "card",
            Layer::Background => "bkgnd",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartStyle {
    Transparent,
    Opaque,
    Rectangle,
    RoundRect,
    Shadow,
    CheckBox,
    Radio,
    Default,
    Oval,
    Scrolling,
}

impl PartStyle {
    pub const BUTTON_STYLES: &'static [PartStyle] = &[
        PartStyle::Transparent,
        PartStyle::Opaque,
        PartStyle::Rectangle,
        PartStyle::RoundRect,
        PartStyle::Shadow,
        PartStyle::CheckBox,
        PartStyle::Radio,
        PartStyle::Default,
        PartStyle::Oval,
    ];
    pub const FIELD_STYLES: &'static [PartStyle] = &[
        PartStyle::Transparent,
        PartStyle::Opaque,
        PartStyle::Rectangle,
        PartStyle::Shadow,
        PartStyle::Scrolling,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PartStyle::Transparent => "transparent",
            PartStyle::Opaque => "opaque",
            PartStyle::Rectangle => "rectangle",
            PartStyle::RoundRect => "roundRect",
            PartStyle::Shadow => "shadow",
            PartStyle::CheckBox => "checkBox",
            PartStyle::Radio => "radioButton",
            PartStyle::Default => "default",
            PartStyle::Oval => "oval",
            PartStyle::Scrolling => "scrolling",
        }
    }

    pub fn parse(s: &str) -> Option<PartStyle> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "transparent" => PartStyle::Transparent,
            "opaque" => PartStyle::Opaque,
            "rectangle" | "rect" => PartStyle::Rectangle,
            "roundrect" => PartStyle::RoundRect,
            "shadow" => PartStyle::Shadow,
            "checkbox" => PartStyle::CheckBox,
            "radiobutton" | "radio" => PartStyle::Radio,
            "default" => PartStyle::Default,
            "oval" => PartStyle::Oval,
            "scrolling" => PartStyle::Scrolling,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub fn name(self) -> &'static str {
        match self {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        }
    }
    pub fn parse(s: &str) -> Option<TextAlign> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "left" => TextAlign::Left,
            "center" | "centre" => TextAlign::Center,
            "right" => TextAlign::Right,
            _ => return None,
        })
    }
}

/// A button or field.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Part {
    pub id: Id,
    pub kind: PartKind,
    pub name: String,
    pub rect: Rect,
    pub script: String,
    pub visible: bool,
    pub style: PartStyle,
    pub text_font: String,
    pub text_size: u32,
    pub text_style: String,
    pub text_align: TextAlign,
    /// Button contents, card field text, or shared background field text.
    pub contents: String,

    // Buttons
    pub hilite: bool,
    pub auto_hilite: bool,
    pub show_name: bool,
    pub enabled: bool,
    pub family: u32,
    pub shared_hilite: bool,
    pub icon: i32,

    // Fields
    pub lock_text: bool,
    pub dont_wrap: bool,
    pub auto_tab: bool,
    pub wide_margins: bool,
    pub show_lines: bool,
    pub shared_text: bool,
    pub dont_search: bool,
    pub auto_select: bool,
    pub scroll: i32,
}

impl Default for Part {
    fn default() -> Self {
        Part {
            id: 0,
            kind: PartKind::Button,
            name: String::new(),
            rect: Rect::new(0, 0, 100, 24),
            script: String::new(),
            visible: true,
            style: PartStyle::RoundRect,
            text_font: "Geneva".into(),
            text_size: 12,
            text_style: "plain".into(),
            text_align: TextAlign::Center,
            contents: String::new(),
            hilite: false,
            auto_hilite: true,
            show_name: true,
            enabled: true,
            family: 0,
            shared_hilite: true,
            icon: 0,
            lock_text: false,
            dont_wrap: false,
            auto_tab: false,
            wide_margins: false,
            show_lines: false,
            shared_text: false,
            dont_search: false,
            auto_select: false,
            scroll: 0,
        }
    }
}

impl Part {
    pub fn new_button(id: Id, name: &str, rect: Rect) -> Part {
        Part { id, kind: PartKind::Button, name: name.into(), rect, ..Part::default() }
    }

    pub fn new_field(id: Id, name: &str, rect: Rect) -> Part {
        Part {
            id,
            kind: PartKind::Field,
            name: name.into(),
            rect,
            style: PartStyle::Rectangle,
            text_align: TextAlign::Left,
            auto_hilite: false,
            show_name: false,
            ..Part::default()
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Background {
    pub id: Id,
    pub name: String,
    pub script: String,
    pub parts: Vec<Part>,
    pub picture: Picture,
    pub show_pict: bool,
    pub cant_delete: bool,
    pub dont_search: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    pub id: Id,
    pub name: String,
    pub script: String,
    pub background: Id,
    pub parts: Vec<Part>,
    pub picture: Picture,
    pub show_pict: bool,
    pub marked: bool,
    pub cant_delete: bool,
    pub dont_search: bool,
    /// Per-card text of background fields (unless the field has sharedText).
    pub bg_text: HashMap<Id, String>,
    /// Per-card hilite of background buttons (unless the button has sharedHilite).
    pub bg_hilite: HashMap<Id, bool>,
}

/// A 32×32 button icon. Clear pixels are transparent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Icon {
    pub name: String,
    pub picture: Picture,
}

/// A sampled sound, as played by HyperTalk's `play` command.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sound {
    pub name: String,
    /// Samples per second.
    pub rate: f32,
    /// The MIDI note the sample sounds at when played unchanged (60 = middle C).
    pub base_note: u8,
    /// Sustain loop, in samples; no loop when `loop_end <= loop_start`.
    pub loop_start: u32,
    pub loop_end: u32,
    /// Mono 16-bit samples.
    #[serde(with = "pcm")]
    pub samples: Vec<i16>,
}

/// Samples are stored as base64 little-endian 16-bit PCM.
mod pcm {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(samples: &[i16], s: S) -> Result<S::Ok, S::Error> {
        let bytes: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<i16>, D::Error> {
        let text = String::deserialize(d)?;
        let bytes = STANDARD.decode(text).map_err(serde::de::Error::custom)?;
        Ok(bytes.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)).collect())
    }
}

/// Protect Stack settings. They deter changes rather than secure anything.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Protection {
    /// The highest user level this stack allows (5 = no limit).
    pub user_level_limit: u8,
    /// HyperCard-compatible password hash (see `password.rs`).
    pub password_hash: Option<u32>,
    pub cant_delete: bool,
    pub cant_abort: bool,
    pub cant_peek: bool,
    /// Ask for the password before opening the stack.
    pub private_access: bool,
}

impl Default for Protection {
    fn default() -> Self {
        Protection {
            user_level_limit: 5,
            password_hash: None,
            cant_delete: false,
            cant_abort: false,
            cant_peek: false,
            private_access: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Stack {
    pub name: String,
    pub script: String,
    pub width: u32,
    pub height: u32,
    pub next_id: Id,
    pub backgrounds: Vec<Background>,
    pub cards: Vec<Card>,
    pub cant_modify: bool,
    pub protection: Protection,
    /// Button icons by resource id.
    pub icons: BTreeMap<i32, Icon>,
    /// Sounds by resource id; scripts play them by name.
    pub sounds: BTreeMap<i32, Sound>,
    /// The paint pattern palette: 40 patterns of 8 rows, numbered from 1.
    #[serde(default = "default_patterns")]
    pub patterns: Vec<[u8; 8]>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
    /// The original HyperCard file this stack was imported from, until saved.
    #[serde(skip)]
    pub imported_from: Option<PathBuf>,
}

fn default_patterns() -> Vec<[u8; 8]> {
    crate::paint::DEFAULT_PATTERNS.to_vec()
}

impl Default for Stack {
    fn default() -> Self {
        Stack::new("Untitled")
    }
}

impl Stack {
    pub fn new(name: &str) -> Stack {
        let (w, h) = (DEFAULT_WIDTH, DEFAULT_HEIGHT);
        let bg = Background { id: 2, picture: Picture::new(w, h), show_pict: true, ..Background::default() };
        let card =
            Card { id: 3, background: 2, picture: Picture::new(w, h), show_pict: true, ..Card::default() };
        Stack {
            name: name.into(),
            script: String::new(),
            width: w,
            height: h,
            next_id: 100,
            backgrounds: vec![bg],
            cards: vec![card],
            cant_modify: false,
            protection: Protection::default(),
            icons: BTreeMap::new(),
            sounds: BTreeMap::new(),
            patterns: default_patterns(),
            path: None,
            imported_from: None,
        }
    }

    pub fn demo() -> Stack {
        demo::build()
    }

    /// Find an icon by id, or by name (case-insensitively).
    pub fn find_icon(&self, id_or_name: &str) -> Option<i32> {
        let t = id_or_name.trim();
        if let Ok(id) = t.parse::<i32>() {
            return Some(id);
        }
        self.icons.iter().find(|(_, i)| i.name.eq_ignore_ascii_case(t)).map(|(id, _)| *id)
    }

    /// Find a sound by name (case-insensitively) or id.
    pub fn find_sound(&self, name: &str) -> Option<&Sound> {
        let t = name.trim();
        self.sounds
            .values()
            .find(|s| s.name.eq_ignore_ascii_case(t))
            .or_else(|| t.parse::<i32>().ok().and_then(|id| self.sounds.get(&id)))
    }

    /// Pattern `n` (numbered from 1), or black if there's no such pattern.
    pub fn pattern(&self, n: usize) -> [u8; 8] {
        self.patterns.get(n.wrapping_sub(1)).copied().unwrap_or([0xff; 8])
    }

    pub fn alloc_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn card_index(&self, id: Id) -> Option<usize> {
        self.cards.iter().position(|c| c.id == id)
    }

    pub fn bg_index(&self, id: Id) -> Option<usize> {
        self.backgrounds.iter().position(|b| b.id == id)
    }

    pub fn background(&self, id: Id) -> Option<&Background> {
        self.backgrounds.iter().find(|b| b.id == id)
    }

    pub fn background_mut(&mut self, id: Id) -> Option<&mut Background> {
        self.backgrounds.iter_mut().find(|b| b.id == id)
    }

    pub fn card(&self, id: Id) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }

    pub fn card_mut(&mut self, id: Id) -> Option<&mut Card> {
        self.cards.iter_mut().find(|c| c.id == id)
    }

    /// Cards that share the given background, in stack order.
    pub fn cards_of_bg(&self, bg: Id) -> Vec<usize> {
        self.cards.iter().enumerate().filter(|(_, c)| c.background == bg).map(|(i, _)| i).collect()
    }

    /// The parts on a given layer: card parts for `card`, or the parts of its background.
    pub fn parts(&self, card: Id, layer: Layer) -> &[Part] {
        let Some(c) = self.card(card) else { return &[] };
        match layer {
            Layer::Card => &c.parts,
            Layer::Background => self.background(c.background).map(|b| b.parts.as_slice()).unwrap_or(&[]),
        }
    }

    pub fn part(&self, card: Id, layer: Layer, id: Id) -> Option<&Part> {
        self.parts(card, layer).iter().find(|p| p.id == id)
    }

    pub fn part_mut(&mut self, card: Id, layer: Layer, id: Id) -> Option<&mut Part> {
        let bg = self.card(card)?.background;
        let parts = match layer {
            Layer::Card => &mut self.card_mut(card)?.parts,
            Layer::Background => &mut self.background_mut(bg)?.parts,
        };
        parts.iter_mut().find(|p| p.id == id)
    }

    /// Text of a field as seen on a particular card.
    pub fn field_text(&self, card: Id, layer: Layer, id: Id) -> String {
        let Some(p) = self.part(card, layer, id) else { return String::new() };
        if layer == Layer::Background && p.kind == PartKind::Field && !p.shared_text {
            self.card(card).and_then(|c| c.bg_text.get(&id).cloned()).unwrap_or_default()
        } else {
            p.contents.clone()
        }
    }

    pub fn set_field_text(&mut self, card: Id, layer: Layer, id: Id, text: String) {
        let shared = match self.part(card, layer, id) {
            Some(p) => p.kind == PartKind::Button || p.shared_text,
            None => return,
        };
        if layer == Layer::Background && !shared {
            if let Some(c) = self.card_mut(card) {
                c.bg_text.insert(id, text);
            }
        } else if let Some(p) = self.part_mut(card, layer, id) {
            p.contents = text;
        }
    }

    pub fn hilite(&self, card: Id, layer: Layer, id: Id) -> bool {
        let Some(p) = self.part(card, layer, id) else { return false };
        if layer == Layer::Background && !p.shared_hilite {
            self.card(card).and_then(|c| c.bg_hilite.get(&id).copied()).unwrap_or(false)
        } else {
            p.hilite
        }
    }

    pub fn set_hilite(&mut self, card: Id, layer: Layer, id: Id, on: bool) {
        let Some(p) = self.part(card, layer, id) else { return };
        let (family, shared) = (p.family, p.shared_hilite);
        if on && family != 0 {
            // Radio-style families: only one button of a family may be hilited.
            let others: Vec<Id> = self
                .parts(card, layer)
                .iter()
                .filter(|q| q.kind == PartKind::Button && q.family == family && q.id != id)
                .map(|q| q.id)
                .collect();
            for other in others {
                self.set_hilite(card, layer, other, false);
            }
        }
        if layer == Layer::Background && !shared {
            if let Some(c) = self.card_mut(card) {
                c.bg_hilite.insert(id, on);
            }
        } else if let Some(p) = self.part_mut(card, layer, id) {
            p.hilite = on;
        }
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("Can't save {}: {e}", path.display()))
    }

    pub fn load(path: &std::path::Path) -> Result<Stack, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
        let mut stack: Stack =
            serde_json::from_str(&text).map_err(|e| format!("{} is not a stack: {e}", path.display()))?;
        if stack.cards.is_empty() || stack.backgrounds.is_empty() {
            return Err(format!("{} contains no cards", path.display()));
        }
        let (w, h) = (stack.width, stack.height);
        for b in &mut stack.backgrounds {
            b.picture.ensure_size(w, h);
        }
        for c in &mut stack.cards {
            c.picture.ensure_size(w, h);
        }
        stack.path = Some(path.to_path_buf());
        Ok(stack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_round_trip() {
        let mut stack = Stack::demo();
        stack.cards[0].picture.set(5, 5, Pixel::Black);
        let sound = Sound { name: "Ding".into(), rate: 11025.0, base_note: 62, loop_start: 2, loop_end: 4, samples: vec![0, -32768, 32767, 12] };
        stack.sounds.insert(7, sound.clone());
        let dir = std::env::temp_dir().join(format!("randomcard-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("demo.rcstack");
        stack.save(&path).unwrap();
        let loaded = Stack::load(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(loaded.cards.len(), stack.cards.len());
        assert_eq!(loaded.script, stack.script);
        assert_eq!(loaded.cards[0].picture, stack.cards[0].picture);
        let last = loaded.cards.last().unwrap();
        assert_eq!(last.bg_text, stack.cards.last().unwrap().bg_text);
        assert_eq!(loaded.path.as_deref(), Some(path.as_path()));
        assert_eq!(loaded.sounds[&7], sound);
    }
}
