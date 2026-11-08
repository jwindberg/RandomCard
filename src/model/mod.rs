//! The stack data model: stacks contain backgrounds and cards; backgrounds and
//! cards each hold a picture layer plus buttons and fields ("parts").

pub mod bitmap_font;
pub mod color;
mod demo;
pub mod home;
pub mod metrics;
pub mod password;
mod picture;
pub mod styles;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use styles::StyleRun;

pub use picture::{Picture, Pixel};

pub type Id = u32;

pub const DEFAULT_WIDTH: u32 = 512;
pub const DEFAULT_HEIGHT: u32 = 342;

/// A Command-key shortcut as this platform writes it: ⌘M on a Mac, where
/// HyperCard's Command key is, and Ctrl+M elsewhere.
pub fn shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") { format!("⌘{key}") } else { format!("Ctrl+{key}") }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Where a popup's chosen item starts, from the left of its menu box: room
/// for the check mark its menu shows, so the item lines up with the menu.
pub const POPUP_TEXT_INSET: i32 = 15;
/// The gap between a popup's triangle and the right of its menu box.
pub const POPUP_TRIANGLE_RIGHT: i32 = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartStyle {
    Transparent,
    Opaque,
    Rectangle,
    RoundRect,
    Shadow,
    CheckBox,
    Radio,
    /// HyperCard 2's standard Macintosh push button.
    Standard,
    Default,
    Oval,
    /// A popup menu: the button's lines are its items.
    Popup,
    Scrolling,
    /// A slider, which HyperCard never had: its contents is its value,
    /// from its min to its max in steps of its step, as HTML's range input.
    Slider,
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
        PartStyle::Standard,
        PartStyle::Default,
        PartStyle::Oval,
        PartStyle::Popup,
        PartStyle::Slider,
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
            PartStyle::Standard => "standard",
            PartStyle::Default => "default",
            PartStyle::Oval => "oval",
            PartStyle::Popup => "popup",
            PartStyle::Scrolling => "scrolling",
            PartStyle::Slider => "slider",
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
            "standard" => PartStyle::Standard,
            "default" => PartStyle::Default,
            "popup" => PartStyle::Popup,
            "oval" => PartStyle::Oval,
            "scrolling" => PartStyle::Scrolling,
            "slider" => PartStyle::Slider,
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

/// How wide a field's choices ▼ is.
pub const CHOICE_ARROW: i32 = 18;

/// A number as a slider shows it: whole numbers without a decimal point,
/// others to at most six places.
pub fn number_text(v: f64) -> String {
    let v = (v * 1e6).round() / 1e6;
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

fn is_zero(n: &f64) -> bool {
    *n == 0.0
}

fn is_hundred(n: &f64) -> bool {
    *n == 100.0
}

fn is_one(n: &f64) -> bool {
    *n == 1.0
}

fn is_false(b: &bool) -> bool {
    !*b
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
    /// Style runs of `contents`, for card fields and shared background text.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<StyleRun>,
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
    /// For popups: the width of the title at the left, where the name goes.
    pub title_width: u32,
    /// For popups: the chosen line, from 1, or 0 for none.
    pub selected_line: u32,
    /// For sliders: the range of values, and the step between them (`the
    /// min`, `the max`, `the step`, as HTML's range input names them).
    #[serde(skip_serializing_if = "is_zero")]
    pub min: f64,
    #[serde(skip_serializing_if = "is_hundred")]
    pub max: f64,
    #[serde(skip_serializing_if = "is_one")]
    pub step: f64,

    // Fields
    pub lock_text: bool,
    pub dont_wrap: bool,
    pub auto_tab: bool,
    pub wide_margins: bool,
    pub show_lines: bool,
    pub shared_text: bool,
    pub dont_search: bool,
    pub auto_select: bool,
    /// With autoSelect, Shift-click selects a run of lines.
    pub multiple_lines: bool,
    /// The height of a line, or 0 for the text size's usual height.
    pub text_height: u32,
    /// Every line is textHeight high, whatever its fonts.
    pub fixed_line_height: bool,
    pub scroll: i32,
    /// A field whose text isn't saved: it works while the stack is open,
    /// and opens empty (`the dontSave`). Saved only when set, so other
    /// stacks' files don't change.
    #[serde(skip_serializing_if = "is_false")]
    pub dont_save: bool,
    /// A field's choices, one a line (`the choices`): a ▼ at its right
    /// edge offers them, and choosing one puts it into the field, which can
    /// still be typed into. A combo box.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub choices: String,

    // Colors, with LiveCode's names: the text (foreColor), the fill
    // (backColor) and the frame (borderColor). None is HyperCard's black
    // and white.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fore_color: Option<color::Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back_color: Option<color::Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<color::Rgb>,
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
            styles: Vec::new(),
            contents: String::new(),
            hilite: false,
            auto_hilite: true,
            show_name: true,
            enabled: true,
            family: 0,
            shared_hilite: true,
            icon: 0,
            title_width: 0,
            selected_line: 0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            lock_text: false,
            dont_wrap: false,
            auto_tab: false,
            wide_margins: false,
            show_lines: false,
            shared_text: false,
            dont_search: false,
            auto_select: false,
            multiple_lines: false,
            text_height: 0,
            fixed_line_height: false,
            scroll: 0,
            dont_save: false,
            choices: String::new(),
            fore_color: None,
            back_color: None,
            border_color: None,
        }
    }
}

impl Part {
    /// The height of a line of this field's text (`the textHeight`).
    pub fn line_height(&self) -> u32 {
        if self.text_height > 0 { self.text_height } else { metrics::default_text_height(self.text_size) }
    }

    /// The nearest value a slider can take to `v`: a whole number of steps
    /// from its min, and within its range, as HTML's range input does.
    pub fn slider_snap(&self, v: f64) -> f64 {
        let (min, max) = (self.min, self.max.max(self.min));
        let step = if self.step > 0.0 { self.step } else { 1.0 };
        let v = if v.is_finite() { v.clamp(min, max) } else { min };
        let snapped = min + ((v - min) / step).round() * step;
        // The last step that fits, if a step would overshoot the max.
        let snapped = if snapped > max { min + ((max - min) / step).floor() * step } else { snapped };
        // Undo floating point dust: 0.1 + 0.2 shows as 0.3.
        (snapped * 1e9).round() / 1e9
    }

    /// Keep a slider's contents a value in its range, after its range or
    /// style changes.
    pub fn resnap_slider(&mut self) {
        if self.style == PartStyle::Slider && self.kind == PartKind::Button {
            self.contents = number_text(self.slider_value());
        }
    }

    /// A slider's value: its contents, snapped into its range (its min when
    /// empty or not a number).
    pub fn slider_value(&self) -> f64 {
        self.slider_snap(self.contents.trim().parse().unwrap_or(self.min))
    }

    /// Where a slider's value sits in its range, from 0 to 1.
    pub fn slider_fraction(&self) -> f64 {
        let span = self.max - self.min;
        if span <= 0.0 { 0.0 } else { ((self.slider_value() - self.min) / span).clamp(0.0, 1.0) }
    }

    /// Whether a slider runs up and down: when it's taller than it is wide.
    pub fn slider_vertical(&self) -> bool {
        self.rect.height() > self.rect.width()
    }

    /// The radius of a slider's thumb, in card pixels.
    pub fn slider_thumb_radius(&self) -> i32 {
        let across = if self.slider_vertical() { self.rect.width() } else { self.rect.height() };
        (across / 2 - 1).clamp(3, 8)
    }

    /// The ends of a slider's track along its length, as card coordinates
    /// (x across, or y down for a vertical one): where the thumb's center
    /// is at the min and at the max. A vertical slider's min is at the
    /// bottom. A named one shows its name in its titleWidth, as popups do.
    pub fn slider_track(&self) -> (i32, i32) {
        let r = self.slider_thumb_radius() + 1;
        if self.slider_vertical() {
            (self.rect.bottom - r, self.rect.top + r)
        } else {
            let title = if self.show_name && !self.name.is_empty() { self.title_width as i32 } else { 0 };
            let left = (self.rect.left + title + r).min(self.rect.right - r);
            (left, self.rect.right - r)
        }
    }

    /// The value at a point along a slider's length (x, or y when
    /// vertical), snapped to its steps.
    pub fn slider_value_at(&self, along: i32) -> f64 {
        let (a, b) = self.slider_track();
        let f = if a == b { 0.0 } else { ((along - a) as f64 / (b - a) as f64).clamp(0.0, 1.0) };
        self.slider_snap(self.min + f * (self.max - self.min))
    }

    /// A popup's chosen item: its selected line, or empty if none is chosen.
    pub fn selected_text(&self) -> &str {
        match self.selected_line {
            0 => "",
            n => self.contents.split('\n').nth(n as usize - 1).unwrap_or(""),
        }
    }

    /// A popup's menu box: right of the title, inside the 1-pixel shadow
    /// along its right and bottom edges.
    pub fn popup_box(&self) -> Rect {
        let r = self.rect;
        Rect::new((r.left + self.title_width as i32).min(r.right - 1), r.top, r.right - 1, r.bottom - 1)
    }

    /// The left, top and right corners of the downward triangle in a
    /// popup's menu box, 11 pixels wide and 6 high.
    pub fn popup_triangle(&self) -> [(i32, i32); 3] {
        let b = self.popup_box();
        let (left, top) = (b.right - POPUP_TRIANGLE_RIGHT - 11, (b.top + b.bottom) / 2 - 3);
        [(left, top), (left + 11, top), (left + 5, top + 6)]
    }

    /// Where a field with choices draws its ▼, inside its right edge.
    pub fn choice_arrow(&self) -> Option<Rect> {
        if self.kind != PartKind::Field || self.choices.trim().is_empty() {
            return None;
        }
        let r = self.rect;
        Some(Rect::new((r.right - CHOICE_ARROW).max(r.left), r.top, r.right, r.bottom))
    }

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
    /// AddColor items, drawn in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<color::ColorItem>,
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
    /// Style runs of the per-card text of background fields.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub bg_styles: HashMap<Id, Vec<StyleRun>>,
    /// AddColor items, drawn in order after the background's.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<color::ColorItem>,
}

/// A 32×32 button icon. Clear pixels are transparent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Icon {
    pub name: String,
    pub picture: Picture,
}

/// A HyperCard 2 palette: a picture with buttons that send messages, shown
/// in a floating window by the `palette` command (a 'PLTE' resource and its
/// 'PICT' in HyperCard).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub name: String,
    pub picture: Picture,
    pub buttons: Vec<PaletteButton>,
}

/// A palette button: where it is in the palette's picture, and the message
/// it sends to the current card.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaletteButton {
    pub rect: Rect,
    pub message: String,
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

/// One size of a bitmap font family, as a `FONT` or `NFNT` resource holds
/// it (see `bitmap_font`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackFont {
    pub family: String,
    pub size: u32,
    #[serde(with = "bytes64")]
    pub data: Vec<u8>,
}

impl StackFont {
    pub fn parse(&self) -> Option<bitmap_font::BitmapFont> {
        bitmap_font::BitmapFont::parse(&self.family, self.size, &self.data)
    }
}

/// A song: a soundtracker module (MOD, XM, S3M or IT) as its file holds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    #[serde(with = "bytes64")]
    pub data: Vec<u8>,
}

/// Bytes stored as base64.
mod bytes64 {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(b: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(b))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
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

/// The version of the `.rcstack` file format this RandomCard writes. Files
/// from before it was recorded are format 1. A newer RandomCard that changes
/// the format bumps this and upgrades older files as it loads them.
pub const STACK_FORMAT: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Stack {
    /// The file format the stack was saved in (see `STACK_FORMAT`).
    pub format: u32,
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
    /// Palettes by resource id, for the `palette` command.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub palettes: BTreeMap<i32, Palette>,
    /// Songs (soundtracker modules) by resource id; `play` plays them by
    /// name, on their own channel.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub songs: BTreeMap<i32, Song>,
    /// Bitmap fonts the stack carries: one size of a family each, as its
    /// `FONT` or `NFNT` resource held it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<StackFont>,
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
            format: STACK_FORMAT,
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
            songs: BTreeMap::new(),
            fonts: Vec::new(),
            palettes: BTreeMap::new(),
            patterns: default_patterns(),
            path: None,
            imported_from: None,
        }
    }

    pub fn demo() -> Stack {
        demo::build()
    }

    /// A new Home stack, for the first time RandomCard runs.
    pub fn home() -> Stack {
        home::build()
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
    pub fn find_song(&self, name: &str) -> Option<&Song> {
        let t = name.trim();
        self.songs.values().find(|s| s.name.eq_ignore_ascii_case(t))
    }

    pub fn find_sound(&self, name: &str) -> Option<&Sound> {
        let t = name.trim();
        self.sounds
            .values()
            .find(|s| s.name.eq_ignore_ascii_case(t))
            .or_else(|| t.parse::<i32>().ok().and_then(|id| self.sounds.get(&id)))
    }

    /// Find a palette by name (case-insensitively) or id.
    pub fn find_palette(&self, name: &str) -> Option<(i32, &Palette)> {
        let t = name.trim();
        self.palettes
            .iter()
            .find(|(_, p)| p.name.eq_ignore_ascii_case(t))
            .or_else(|| t.parse::<i32>().ok().and_then(|id| self.palettes.get_key_value(&id)))
            .map(|(id, p)| (*id, p))
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

    /// Set a field's text. Its style runs follow the edit: new text takes
    /// the style of the text it replaces.
    pub fn set_field_text(&mut self, card: Id, layer: Layer, id: Id, text: String) {
        let old = self.field_text(card, layer, id);
        let mut runs = self.field_styles(card, layer, id);
        styles::retext(&mut runs, &old, &text);
        self.set_field_content(card, layer, id, text, runs);
    }

    /// Set a field's text and style runs together.
    pub fn set_field_content(&mut self, card: Id, layer: Layer, id: Id, text: String, runs: Vec<StyleRun>) {
        let (shared, text) = match self.part(card, layer, id) {
            // A slider's contents is its value, kept in its range.
            Some(p) if p.style == PartStyle::Slider && p.kind == PartKind::Button => {
                (true, number_text(p.slider_snap(text.trim().parse().unwrap_or(p.min))))
            }
            Some(p) => (p.kind == PartKind::Button || p.shared_text, text),
            None => return,
        };
        if layer == Layer::Background && !shared {
            if let Some(c) = self.card_mut(card) {
                c.bg_text.insert(id, text);
                if runs.is_empty() {
                    c.bg_styles.remove(&id);
                } else {
                    c.bg_styles.insert(id, runs);
                }
            }
        } else if let Some(p) = self.part_mut(card, layer, id) {
            p.contents = text;
            p.styles = runs;
        }
    }

    /// Change every style run of a field, on every card (for setting the
    /// whole field's textFont, textSize or textStyle).
    pub fn restyle_field(&mut self, card: Id, layer: Layer, id: Id, change: impl Fn(&mut styles::TextStyle)) {
        let fix = |runs: &mut Vec<StyleRun>, len: usize| {
            for r in runs.iter_mut() {
                change(&mut r.style);
            }
            styles::normalize(runs, len);
        };
        if layer == Layer::Background {
            let bg = self.card(card).map(|c| c.background);
            for c in self.cards.iter_mut().filter(|c| Some(c.background) == bg) {
                let len = c.bg_text.get(&id).map(|t| t.chars().count()).unwrap_or(0);
                if let Some(runs) = c.bg_styles.get_mut(&id) {
                    fix(runs, len);
                }
                c.bg_styles.retain(|_, r| !r.is_empty());
            }
        }
        if let Some(p) = self.part_mut(card, layer, id) {
            let len = p.contents.chars().count();
            fix(&mut p.styles, len);
        }
    }

    /// A field's style runs (empty when all its text has the field's style).
    pub fn field_styles(&self, card: Id, layer: Layer, id: Id) -> Vec<StyleRun> {
        let Some(p) = self.part(card, layer, id) else { return Vec::new() };
        if layer == Layer::Background && p.kind == PartKind::Field && !p.shared_text {
            self.card(card).and_then(|c| c.bg_styles.get(&id).cloned()).unwrap_or_default()
        } else {
            p.styles.clone()
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

    /// Compact Stack: drop what nothing uses any more. Cards keep the text,
    /// hilite and style runs of background fields and buttons that have
    /// since been deleted; style runs can outlast their text; and an
    /// imported stack can have backgrounds no card uses.
    pub fn compact(&mut self) {
        self.backgrounds.retain(|b| self.cards.iter().any(|c| c.background == b.id));
        for c in &mut self.cards {
            let Some(bg) = self.backgrounds.iter().find(|b| b.id == c.background) else { continue };
            let has = |id: &Id, kind: Option<PartKind>| bg.parts.iter().any(|p| p.id == *id && kind.is_none_or(|k| p.kind == k));
            c.bg_text.retain(|id, _| has(id, Some(PartKind::Field)));
            c.bg_hilite.retain(|id, _| has(id, None));
            c.bg_styles.retain(|id, _| has(id, Some(PartKind::Field)));
            for (id, runs) in c.bg_styles.iter_mut() {
                styles::normalize(runs, c.bg_text.get(id).map_or(0, |t| t.chars().count()));
            }
            c.bg_styles.retain(|_, runs| !runs.is_empty());
        }
        let parts = self.cards.iter_mut().flat_map(|c| c.parts.iter_mut());
        for p in parts.chain(self.backgrounds.iter_mut().flat_map(|b| b.parts.iter_mut())) {
            let len = p.contents.chars().count();
            styles::normalize(&mut p.styles, len);
        }
    }

    /// The size of the stack as saved, in bytes.
    pub fn size(&self) -> usize {
        serde_json::to_string(&*self.for_saving()).map(|j| j.len()).unwrap_or(0)
    }

    /// The stack as its file holds it: fields marked Don't Save are empty
    /// there (their own text, each card's text for a background field, and
    /// their styles). The stack itself is only copied when there are some.
    pub fn for_saving(&self) -> std::borrow::Cow<'_, Stack> {
        let all = || self.cards.iter().flat_map(|c| &c.parts).chain(self.backgrounds.iter().flat_map(|b| &b.parts));
        if !all().any(|p| p.dont_save) {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut s = self.clone();
        let mut bg_unsaved = Vec::new();
        for p in s.cards.iter_mut().flat_map(|c| c.parts.iter_mut()).chain(s.backgrounds.iter_mut().flat_map(|b| b.parts.iter_mut())) {
            if p.dont_save {
                p.contents.clear();
                p.styles.clear();
                bg_unsaved.push(p.id);
            }
        }
        for c in &mut s.cards {
            for id in &bg_unsaved {
                c.bg_text.remove(id);
                c.bg_styles.remove(id);
            }
        }
        std::borrow::Cow::Owned(s)
    }

    /// The stack's file contents.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&*self.for_saving()).map_err(|e| e.to_string())
    }

    /// What Compact Stack would free, in bytes (`the freeSize`).
    pub fn free_size(&self) -> usize {
        let mut compacted = self.clone();
        compacted.compact();
        self.size().saturating_sub(compacted.size())
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let json = self.to_json()?;
        std::fs::write(path, json).map_err(|e| format!("Can't save {}: {e}", path.display()))
    }

    pub fn load(path: &std::path::Path) -> Result<Stack, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
        let mut stack: Stack =
            serde_json::from_str(&text).map_err(|e| format!("{} is not a stack: {e}", path.display()))?;
        if stack.format > STACK_FORMAT {
            return Err(format!(
                "{} was saved by a newer RandomCard (stack format {}). This version reads up to format {STACK_FORMAT}.",
                path.display(),
                stack.format
            ));
        }
        // Loaded stacks save in the current format.
        stack.format = STACK_FORMAT;
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
    fn dont_save_fields_save_empty() {
        let mut s = Stack::new("t");
        let card = s.cards[0].id;
        let mut out = Part::new_field(50, "Out", Rect::new(0, 0, 10, 10));
        out.contents = "result".into();
        out.styles = vec![styles::StyleRun { start: 0, style: styles::TextStyle { size: Some(9), ..Default::default() } }];
        out.dont_save = true;
        let mut kept = Part::new_field(51, "Kept", Rect::new(0, 0, 10, 10));
        kept.contents = "keep me".into();
        s.cards[0].parts.extend([out, kept]);
        // A background field: each card's own text isn't saved either.
        let mut bg = Part::new_field(52, "Status", Rect::new(0, 0, 10, 10));
        bg.dont_save = true;
        s.backgrounds[0].parts.push(bg);
        s.cards[0].bg_text.insert(52, "OK".into());
        let saved = s.for_saving();
        assert_eq!(saved.part(card, Layer::Card, 50).unwrap().contents, "");
        assert!(saved.part(card, Layer::Card, 50).unwrap().styles.is_empty());
        assert_eq!(saved.part(card, Layer::Card, 51).unwrap().contents, "keep me");
        assert!(!saved.cards[0].bg_text.contains_key(&52));
        // The stack in memory keeps it all, and the setting is saved.
        assert_eq!(s.part(card, Layer::Card, 50).unwrap().contents, "result");
        let back: Stack = serde_json::from_str(&s.to_json().unwrap()).unwrap();
        assert!(back.part(card, Layer::Card, 50).unwrap().dont_save);
        // Stacks without it save as they always did.
        let plain = Stack::new("p");
        assert!(matches!(plain.for_saving(), std::borrow::Cow::Borrowed(_)));
        assert!(!plain.to_json().unwrap().contains("dont_save"));
    }

    #[test]
    fn compacting() {
        let mut stack = Stack::demo();
        let card = stack.cards[0].id;
        // Text, a hilite and styles left behind by a deleted background field.
        stack.cards[0].bg_text.insert(9999, "left behind".into());
        stack.cards[0].bg_hilite.insert(9999, true);
        stack.cards[0].bg_styles.insert(9999, vec![StyleRun { start: 0, style: Default::default() }]);
        let unused = Background { id: 9998, ..stack.backgrounds[0].clone() };
        stack.backgrounds.push(unused);
        let before = stack.clone();
        assert!(stack.free_size() > 0);
        stack.compact();
        assert_eq!(stack.free_size(), 0);
        assert!(!stack.cards[0].bg_text.contains_key(&9999) && !stack.cards[0].bg_hilite.contains_key(&9999));
        assert!(stack.backgrounds.iter().all(|b| b.id != 9998));
        assert_eq!(stack.backgrounds.len(), before.backgrounds.len() - 1);
        assert_eq!(stack.size(), before.size() - before.free_size());
        // Everything in use stays.
        assert_eq!(stack.cards.len(), before.cards.len());
        let _ = card;
    }

    #[test]
    fn save_and_load_round_trip() {
        let mut stack = Stack::demo();
        stack.cards[0].picture.set(5, 5, Pixel::BLACK);
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
        // The demo's lesson has styled text.
        let lesson = |s: &Stack| s.cards[1].parts.iter().find(|p| p.name == "Lesson").unwrap().styles.clone();
        assert!(!lesson(&stack).is_empty());
        assert_eq!(lesson(&loaded), lesson(&stack));
        assert_eq!(loaded.path.as_deref(), Some(path.as_path()));
        assert_eq!(loaded.sounds[&7], sound);
    }

    #[test]
    fn stack_format() {
        let dir = std::env::temp_dir().join(format!("randomcard-format-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.rcstack");
        let saved = |json: serde_json::Value| std::fs::write(&path, json.to_string()).unwrap();
        let mut json = serde_json::to_value(Stack::new("S")).unwrap();
        assert_eq!(json["format"], STACK_FORMAT);
        // Stacks saved before the format was recorded are format 1.
        json.as_object_mut().unwrap().remove("format");
        saved(json.clone());
        assert_eq!(Stack::load(&path).unwrap().format, STACK_FORMAT);
        // A newer RandomCard's stack is refused, not misread.
        json["format"] = (STACK_FORMAT + 1).into();
        saved(json);
        let err = Stack::load(&path).unwrap_err();
        std::fs::remove_dir_all(&dir).ok();
        assert!(err.contains("newer RandomCard"), "{err}");
    }
}
