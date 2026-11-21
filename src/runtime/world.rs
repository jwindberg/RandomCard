//! State shared between the UI thread and the script thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::model::{Id, Layer, Picture, Stack};

pub type Shared = Arc<Mutex<World>>;

pub fn lock(shared: &Shared) -> MutexGuard<'_, World> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjRef {
    Stack,
    Background(Id),
    Card(Id),
    /// A button or field. `card` is the card it is seen on, which matters for
    /// background fields whose text differs from card to card.
    Part { layer: Layer, id: Id, card: Id },
    /// A stack that's loaded but not the one shown: Home, a stack in use,
    /// or one left without a file (see `World::others`).
    OtherStack(u32),
    MsgBox,
    HyperCard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Browse,
    Button,
    Field,
    Select,
    Lasso,
    Pencil,
    Brush,
    Eraser,
    Line,
    Spray,
    Rect,
    RoundRect,
    Bucket,
    Oval,
    Curve,
    Text,
    RegularPolygon,
    Polygon,
}

impl Tool {
    /// In HyperCard's tool palette order.
    pub const ALL: &'static [Tool] = &[
        Tool::Browse,
        Tool::Button,
        Tool::Field,
        Tool::Select,
        Tool::Lasso,
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Line,
        Tool::Spray,
        Tool::Rect,
        Tool::RoundRect,
        Tool::Bucket,
        Tool::Oval,
        Tool::Curve,
        Tool::Text,
        Tool::RegularPolygon,
        Tool::Polygon,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Browse => "browse",
            Tool::Button => "button",
            Tool::Field => "field",
            Tool::Select => "select",
            Tool::Lasso => "lasso",
            Tool::Pencil => "pencil",
            Tool::Brush => "brush",
            Tool::Eraser => "eraser",
            Tool::Line => "line",
            Tool::Spray => "spray can",
            Tool::Rect => "rectangle",
            Tool::RoundRect => "round rect",
            Tool::Bucket => "bucket",
            Tool::Oval => "oval",
            Tool::Curve => "curve",
            Tool::Text => "text",
            Tool::RegularPolygon => "regular polygon",
            Tool::Polygon => "polygon",
        }
    }

    /// HyperCard's tool number, as passed with the `choose` message.
    pub fn number(self) -> u32 {
        match self {
            Tool::Browse => 1,
            Tool::Button => 2,
            Tool::Field => 3,
            Tool::Select => 4,
            Tool::Lasso => 5,
            Tool::Pencil => 6,
            Tool::Brush => 7,
            Tool::Eraser => 8,
            Tool::Line => 9,
            Tool::Spray => 10,
            Tool::Rect => 11,
            Tool::RoundRect => 12,
            Tool::Bucket => 13,
            Tool::Oval => 14,
            Tool::Curve => 15,
            Tool::Text => 16,
            Tool::RegularPolygon => 17,
            Tool::Polygon => 18,
        }
    }

    /// The paint tools that work by dragging, for scripts' `drag`.
    pub fn drag_tool(self) -> Option<crate::paint::DragTool> {
        use crate::paint::DragTool as D;
        Some(match self {
            Tool::Pencil => D::Pencil,
            Tool::Brush => D::Brush,
            Tool::Eraser => D::Eraser,
            Tool::Spray => D::Spray,
            Tool::Curve => D::Curve,
            Tool::Line | Tool::Polygon => D::Line,
            Tool::Rect => D::Rect,
            Tool::RoundRect => D::RoundRect,
            Tool::Oval => D::Oval,
            Tool::RegularPolygon => D::RegularPolygon,
            _ => return None,
        })
    }

    pub fn is_paint(self) -> bool {
        !matches!(self, Tool::Browse | Tool::Button | Tool::Field)
    }

    /// Accepts names like "browse", "round rect tool" or HyperCard tool numbers.
    pub fn parse(s: &str) -> Option<Tool> {
        let s = s.trim().to_ascii_lowercase();
        let s = s.strip_suffix(" tool").unwrap_or(&s).trim().to_string();
        if let Ok(n) = s.parse::<u32>() {
            return Tool::ALL.iter().copied().find(|t| t.number() == n);
        }
        Some(match s.as_str() {
            "browse" => Tool::Browse,
            "button" => Tool::Button,
            "field" => Tool::Field,
            "select" => Tool::Select,
            "lasso" => Tool::Lasso,
            "pencil" => Tool::Pencil,
            "curve" => Tool::Curve,
            "brush" => Tool::Brush,
            "eraser" => Tool::Eraser,
            "line" => Tool::Line,
            "poly" | "polygon" => Tool::Polygon,
            "reg poly" | "regular polygon" | "reg polygon" | "regular poly" => Tool::RegularPolygon,
            "spray" | "spray can" => Tool::Spray,
            "rect" | "rectangle" => Tool::Rect,
            "round rect" | "round rectangle" | "roundrect" => Tool::RoundRect,
            "bucket" => Tool::Bucket,
            "oval" => Tool::Oval,
            "text" => Tool::Text,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub down: bool,
    pub click_x: i32,
    pub click_y: i32,
    /// Set by the UI on each click, cleared when a script asks `the mouseClick`.
    pub clicked: bool,
    pub shift: bool,
    pub option: bool,
    pub command: bool,
    /// Whether Caps Lock is on, as far as the UI can tell from typing.
    pub caps_lock: bool,
}

#[derive(Clone, Debug)]
pub struct Transition {
    pub effect: String,
    pub seconds: f32,
    pub seq: u64,
}

/// Text copied from a field, and its style runs, counted from its start.
#[derive(Clone, Debug, PartialEq)]
pub struct TextClip {
    pub text: String,
    pub runs: Vec<crate::model::styles::StyleRun>,
}

/// A run of characters in a field on a card: found text, the text
/// selection, or the word clicked in a locked field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRange {
    pub card: Id,
    pub layer: Layer,
    pub field: Id,
    /// Character offsets within the field text.
    pub start: usize,
    pub end: usize,
}

/// A window a script opened, as HyperCard 2's externals did: scripts refer
/// to it by name (`the text of window "Notes"`, `show window "Notes"`).
#[derive(Clone, Debug)]
pub struct ScriptWindow {
    pub name: String,
    pub kind: WindowKind,
    /// A text window's text, or a progress window's label.
    pub text: String,
    /// Where on the card it is.
    pub rect: crate::model::Rect,
    pub font: String,
    pub size: u32,
    pub visible: bool,
    /// Whether the user has changed the text.
    pub dirty: bool,
    /// A text window the user can read but not change (`the lockText`).
    pub locked: bool,
    /// A palette window's palette.
    pub palette: Option<std::sync::Arc<crate::model::Palette>>,
    /// What a built-in Apple palette (GeneralPalette and the like) was
    /// made from, to draw it again.
    pub apple: Option<Box<super::apple_palettes::ApplePalette>>,
    /// A picture window's picture (the `picture` command).
    pub image: Option<std::sync::Arc<crate::model::Picture>>,
    /// A Texas window's style and the message it shows.
    pub texas: Option<Box<super::texas::Texas>>,
    /// A Dialoger dialog's items and settings.
    pub dialoger: Option<Box<super::dialoger::Dialoger>>,
}

/// What a picture window shows of its picture: from where (in the
/// picture's own pixels), how magnified (each step of the scale doubles or
/// halves it), and the rect to go back to after zooming out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PictureView {
    pub scroll: (i32, i32),
    pub scale: i32,
    pub dithering: bool,
    pub unzoomed: Option<crate::model::Rect>,
}

impl PictureView {
    /// How many screen pixels a picture pixel takes.
    pub fn factor(&self) -> f32 {
        2f32.powi(self.scale.clamp(-5, 5))
    }
}

/// How a picture window is framed: HyperCard's Picture XCMD's styles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PictureStyle {
    /// A document window with a title bar, close box and zoom box.
    Zoom,
    Document,
    /// A small floating window with a thin title bar and close box.
    Windoid,
    Plain,
    Rect,
    Shadow,
    Dialog,
    RoundRect,
    Borderless,
}

impl PictureStyle {
    pub fn named(s: &str) -> Option<PictureStyle> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "" | "zoom" => PictureStyle::Zoom,
            "document" => PictureStyle::Document,
            "windoid" | "palette" => PictureStyle::Windoid,
            "plain" => PictureStyle::Plain,
            "rect" => PictureStyle::Rect,
            "shadow" => PictureStyle::Shadow,
            "dialog" => PictureStyle::Dialog,
            "roundrect" => PictureStyle::RoundRect,
            "borderless" => PictureStyle::Borderless,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowKind {
    /// Editable text (TEXWindow).
    Text,
    /// A label over a progress bar, filled to a fraction from 0 to 1
    /// (ProgressWindoid).
    Progress(f64),
    /// A palette (the `palette` command): its id, and the button that
    /// stays highlighted (from 1), or -1 for none.
    Palette { id: i32, hilited: i32 },
    /// A picture window (the `picture` command): its id, style, and what
    /// part of the picture it shows.
    Picture { id: i32, style: PictureStyle, view: PictureView },
    /// A windoid the UI keeps up to date: the mouse's position (Mousoid).
    Mouse,
    /// A windoid the UI keeps up to date: what's under the mouse (Infoid).
    Info,
    /// A window of messages (Texas, from the Developer Info Assistant).
    Texas,
    /// A dialog from the stack's resources (Dialoger).
    Dialoger,
}

impl ScriptWindow {
    /// A property, as `the fraction of window "Progress"` reads it.
    pub fn property(&self, name: &str) -> Option<String> {
        if let Some(v) = self.texas_property(name) {
            return Some(v);
        }
        let r = self.rect;
        Some(match name {
            "name" | "short name" => self.name.clone(),
            "text" | "contents" => self.text.clone(),
            "rect" | "rectangle" => format!("{},{},{},{}", r.left, r.top, r.right, r.bottom),
            // A window's loc is its top left corner.
            "loc" | "location" | "topleft" => format!("{},{}", r.left, r.top),
            "width" => r.width().to_string(),
            "height" => r.height().to_string(),
            "visible" => self.visible.to_string(),
            "textfont" => self.font.clone(),
            "textsize" => self.size.to_string(),
            "locktext" => match self.kind {
                WindowKind::Text | WindowKind::Texas => self.locked.to_string(),
                _ => return None,
            },
            "fraction" => match self.kind {
                WindowKind::Progress(f) => crate::hypertalk::value::format_number(f, "0.######"),
                _ => return None,
            },
            "id" => match self.kind {
                WindowKind::Palette { id, .. } | WindowKind::Picture { id, .. } => id.to_string(),
                _ => return None,
            },
            "buttoncount" => self.palette.as_ref()?.buttons.len().to_string(),
            "commands" => {
                let p = self.palette.as_ref()?;
                p.buttons.iter().map(|b| b.message.as_str()).collect::<Vec<_>>().join("\n")
            }
            "hilitedbutton" => match self.kind {
                WindowKind::Palette { hilited, .. } => hilited.to_string(),
                _ => return None,
            },
            // FieldPalette's three lines.
            "fields" => self.apple.as_ref().filter(|a| a.layout == super::apple_palettes::Layout::Field)?.fields.clone(),
            // The external that made it, as HyperCard 2.1 answered.
            "owner" => match self.kind {
                WindowKind::Picture { .. } => "Picture".into(),
                WindowKind::Palette { .. } => "Palette".into(),
                _ => return None,
            },
            "picturewidth" => self.image.as_ref()?.width.to_string(),
            "pictureheight" => self.image.as_ref()?.height.to_string(),
            "properties" => match self.kind {
                WindowKind::Picture { .. } => {
                    "loc,rect,globalLoc,globalRect,scroll,scale,zoom,dithering,visible,pictureWidth,pictureHeight,owner,properties".into()
                }
                WindowKind::Progress(_) => "width,text,fraction,textFont,textSize".into(),
                WindowKind::Text => "text,textFont,textSize,lockText".into(),
                WindowKind::Palette { .. } => "buttonCount,commands,hilitedButton,properties".into(),
                WindowKind::Mouse | WindowKind::Info => "loc,rect,visible,properties".into(),
                WindowKind::Texas => "text,textFont,textSize".into(),
                WindowKind::Dialoger => "loc,rect,visible,properties".into(),
            },
            _ => return None,
        })
    }

    /// Set a property; false if the window has no such property, Err if the
    /// value won't do.
    pub fn set_property(&mut self, name: &str, v: &str) -> Result<bool, String> {
        if self.set_texas_property(name, v) {
            return Ok(true);
        }
        let int = |v: &str| {
            v.trim()
                .parse::<f64>()
                .map(|n| n as i32)
                .map_err(|_| format!("Expected a number, not \"{v}\""))
        };
        let point = |v: &str| {
            crate::hypertalk::value::parse_point(v)
                .ok_or_else(|| format!("Expected a point, not \"{v}\""))
        };
        let r = self.rect;
        match name {
            "text" | "contents" => {
                self.text = v.to_string();
                self.dirty = false;
            }
            "rect" | "rectangle" => {
                let Some((l, t, rt, b)) = crate::hypertalk::value::parse_rect(v) else {
                    return Err(format!("Expected a rectangle, not \"{v}\""));
                };
                self.rect = crate::model::Rect::new(l, t, rt, b);
            }
            "loc" | "location" | "topleft" => {
                let (x, y) = point(v)?;
                self.rect = r.translate(x - r.left, y - r.top);
            }
            "width" => self.rect.right = r.left + int(v)?.max(0),
            "height" => self.rect.bottom = r.top + int(v)?.max(0),
            "visible" => self.visible = v.trim().eq_ignore_ascii_case("true"),
            "textfont" => self.font = v.to_string(),
            "textsize" => self.size = int(v)?.clamp(1, 127) as u32,
            "locktext" => match self.kind {
                WindowKind::Text | WindowKind::Texas => self.locked = v.trim().eq_ignore_ascii_case("true"),
                _ => return Ok(false),
            },
            "fraction" => match self.kind {
                WindowKind::Progress(_) => {
                    let f = v
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("Expected a number, not \"{v}\""))?;
                    self.kind = WindowKind::Progress(f.clamp(0.0, 1.0));
                }
                _ => return Ok(false),
            },
            // FieldPalette's three lines, drawn on it.
            "fields" => match &mut self.apple {
                Some(a) if a.layout == super::apple_palettes::Layout::Field => {
                    a.fields = v.to_string();
                    self.palette = Some(std::sync::Arc::new(a.render()));
                }
                _ => return Ok(false),
            },
            // Highlights a palette button without sending its message.
            "hilitedbutton" => match &mut self.kind {
                WindowKind::Palette { hilited, .. } => {
                    let n = int(v)?;
                    let count = self.palette.as_ref().map_or(0, |p| p.buttons.len()) as i32;
                    *hilited = if (1..=count).contains(&n) { n } else { -1 };
                }
                _ => return Ok(false),
            },
            _ => return Ok(false),
        }
        Ok(true)
    }
}

#[derive(Clone, Debug)]
pub enum Dialog {
    Answer { prompt: String, buttons: Vec<String> },
    Ask { prompt: String, default: String, password: bool },
    /// A pop-up menu at a point on the card, for externals like PopUpMenu.
    /// The reply is the chosen item's number, from 1, or None.
    Menu { items: Vec<String>, left: i32, top: i32, checked: u32 },
    /// A list to choose from, for ListSelect. Mode 0 shows it without
    /// selection, 1 lets one line be chosen, 2 a run of lines, 3 any lines.
    /// Empty button names hide those buttons. The reply is DialogReply::List.
    List(ListDialog),
    /// A Dialoger dialog, by its window's name: the UI draws it from the
    /// window and replies Hit(item) for each click.
    Dialoger(String),
}

#[derive(Clone, Debug, Default)]
pub struct ListDialog {
    pub prompt: String,
    pub items: Vec<String>,
    pub mode: u8,
    /// Lines chosen at the start, from 0.
    pub selected: Vec<usize>,
    pub ok: String,
    pub cancel: String,
    pub double_click: bool,
    pub font: Option<(String, u16)>,
    /// Where on the screen: the top left, and the bottom right if given.
    pub at: Option<(i32, i32)>,
    pub size: Option<(i32, i32)>,
    /// Named buttons in place of OK and Cancel (ShowList's), the default
    /// first, each with whether a line is drawn above it. The reply names
    /// the one clicked.
    pub buttons: Vec<(String, bool)>,
    /// Typing and the arrow keys leave the selection alone.
    pub no_typing: bool,
}

#[derive(Clone, Debug)]
pub enum DialogReply {
    Button(String),
    Text(Option<String>),
    /// The lines chosen in a list, from 0, and how the user left it:
    /// "Validated", "Cancelled" or "TabKey".
    List(Vec<usize>, String),
    /// A click on a Dialoger dialog's item, from 1; 0 to cancel it.
    Hit(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditOp {
    Cut,
    Copy,
    Paste,
    Clear,
}

/// What a key does when no script handles its message.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyAction {
    /// Type this text into the field being edited.
    Insert(String),
    /// Move to the next unlocked field, or the first one if none is active.
    NextField,
    /// Stop editing the field.
    CloseField,
    /// Type into the message box (HyperCard's blind typing).
    BlindType(String),
    /// Run what's in the message box.
    RunMessage,
    /// Do a menu command (for command-key shortcuts).
    Menu(String),
    Undo,
    Redo,
    Nothing,
}

/// Whether a script names RandomCard's palette. HyperCard had a Tools
/// palette (`tool window`) and a Patterns palette (`pattern window`);
/// RandomCard has one palette with the tools, patterns and colors, which
/// both names mean.
/// The message box, by the names scripts give its window.
pub fn is_message_window(name: &str) -> bool {
    matches!(name.trim().to_ascii_lowercase().as_str(), "message" | "message box" | "msg" | "message window" | "msg box")
}

pub fn is_palette_name(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "tool" | "tools" | "tool window" | "tools window" | "pattern" | "patterns" | "pattern window" | "patterns window"
    )
}

/// A palette as the UI last showed it, for scripts to read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaletteState {
    pub visible: bool,
    /// Its rect on screen, in global coordinates.
    pub rect: crate::model::Rect,
}

/// The Options menu's dialogs for paint settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintDialog {
    LineSize,
    BrushShape,
    PolygonSides,
}

/// Things only the UI thread can do, requested by scripts or menu commands.
#[derive(Clone, Debug)]
pub enum UiRequest {
    /// Put text on the system clipboard (a script's Copy Text or Cut Text).
    CopyText(String),
    OpenStack,
    /// File ▸ Open Stack in New Window…
    OpenStackInNewWindow,
    NewStack,
    SaveStackAs(Option<String>),
    Quit,
    SelectedPartInfo,
    /// Print the field selected with the Field tool (Print Field…).
    PrintSelectedField,
    CardInfo,
    BgInfo,
    StackInfo,
    EditScript(ObjRef),
    ScriptError { msg: String, obj: Option<ObjRef>, line: Option<usize> },
    BringCloser,
    SendFarther,
    BringToFront,
    SendToBack,
    /// Cut, Copy, Paste or Clear of the selected part or picture.
    Edit(EditOp),
    Undo,
    Redo,
    SelectPart(ObjRef),
    /// A Paint menu command, applied to the selection (or the whole picture).
    PaintOp(crate::paint::PaintOp),
    SelectAll,
    /// Paint ▸ Select: the new element, or what's painted in the
    /// selection or the layer, held tightly.
    SelectPainted,
    /// Paint ▸ Pickup: the selection takes the picture under it.
    Pickup,
    /// Options ▸ Rotate, Slant, Distort or Perspective: handles on the
    /// rectangular selection's corners.
    Reshape(crate::paint::Reshape),
    /// Paint ▸ Revert: the layer's paint (or the selection's part of it)
    /// as last kept.
    Revert,
    /// Options ▸ FatBits: magnify the paint, or stop.
    FatBits,
    /// Edit ▸ Icon…: the icon editor, on the selected button's icon.
    IconEditor,
    /// Style ▸ Text Color…: a color for the selected text.
    TextColor,
    /// Select this rectangle of the picture with the Select tool (after
    /// Import Paint).
    SelectPaint((i32, i32, i32, i32)),
    /// Save a picture of the card as it looks to this PNG file.
    ExportPaint(std::path::PathBuf),
    ProtectStack,
    Preferences,
    /// View ▸ Tools Palette.
    TogglePalette,
    /// Show the Recent dialog.
    Recent,
    /// Put the cursor in the message box at this character.
    FocusMessage(usize),
    /// Show or hide the message or variable watcher.
    MessageWatcher(bool),
    VariableWatcher(bool),
    /// Show or hide a palette (`show tool window`, `hide pattern window`).
    ShowPalette(bool),
    /// Move a palette's top left corner to this point on screen
    /// (`set the loc of tool window`).
    MovePalette(i32, i32),
    /// Move the window so the card's top left corner is at this point on screen.
    MoveWindow(i32, i32),
    /// The cards changed size: fit the window around them.
    FitCard,
    /// Open a URL, such as a `mailto:` message, in the program that handles it.
    OpenUrl(String),
    /// Keep this user level in Settings: it was set from the Home stack.
    SaveUserLevel(u8),
    /// Keep the Power Keys option in Settings: it was chosen in the Options menu.
    SavePowerKeys(bool),
    /// Options ▸ Edit Pattern…: edit the current pattern.
    EditPattern,
    /// Show the Line Size, Brush Shape or Polygon Sides dialog.
    PaintDialog(PaintDialog),
    /// Ask for the user's attention while RandomCard is in the background
    /// (Notification): the Dock icon bounces, the taskbar entry flashes.
    Attention,
    /// A key's message wasn't handled: do what the key does by default.
    Key(ObjRef, KeyAction),
}

/// Messages from the UI to the script thread.
#[derive(Clone, Debug)]
pub enum Event {
    /// A click in a picture window, at a point in its picture: down or up.
    PictureMouse { name: String, down: bool, at: (i32, i32) },
    /// The zoom box of a picture window.
    PictureZoom(String),
    /// A button or the close box of a Texas window.
    Texas { name: String, action: super::texas::TexasAction },
    /// A discussion window's arrows or pop-up: show message n, from 0.
    TexasShow { name: String, n: usize },
    /// A click on a floating Dialoger dialog's item, from 1.
    DialogerHit { name: String, item: usize },
    /// A web request made `with message` has its reply: send the message
    /// to the object whose script asked, with the URL and the reply.
    WebReply { target: ObjRef, message: String, url: String, reply: Result<crate::web::http::Response, String>, millis: f64 },
    Startup,
    Send { target: ObjRef, message: String, args: Vec<String> },
    MsgBox(String),
    DoMenu(String),
    /// A keyboard message (keyDown, returnInField, tabKey, ...), with what
    /// the key does if no handler takes it.
    /// `at`: when the key was typed, so `flush keys` can drop older ones.
    Key { target: ObjRef, message: String, args: Vec<String>, default: KeyAction, at: std::time::Instant },
    /// Open a stack file (RandomCard or HyperCard), asking for its password
    /// if it needs one.
    OpenFile(std::path::PathBuf),
    /// Open a stack file in a new card window.
    OpenFileInWindow(std::path::PathBuf),
    /// The user clicked a card window behind the current one.
    FrontWindow(u32),
    /// The user closed a stack's card window.
    CloseStackWindow(u32),
    /// The user closed a script's text window: the card is sent
    /// TEXWindowClosing name, changed, while the window can still be read,
    /// and then it closes.
    CloseWindow(String),
    /// The user closed a movie window, or it closed at its end: the card
    /// is sent closeMovie, and it closes.
    CloseMovie(u32),
    /// A movie file dropped on the card window: it opens in a movie window
    /// down and to the right of the card's corner, as in HyperCard 2.4.
    OpenMovie(std::path::PathBuf),
    /// The user clicked button `n` (from 0) of the named palette.
    PaletteButton(String, usize),
    /// Stack Info's Resize: make every card this size.
    ResizeCards(u32, u32),
    /// The user resized the window, which makes the card window this size.
    SizeCardWindow(i32, i32),
    /// The user moved the window: the card window's top left corner is now
    /// here on screen.
    CardWindowMoved(i32, i32),
    Shutdown,
}

pub struct World {
    pub stack: Stack,
    pub card_id: Id,
    pub tool: Tool,
    pub edit_bg: bool,
    pub msg: String,
    pub msg_visible: bool,
    /// Where a script's `type` goes in the message box, in characters, when
    /// it has the insertion point (after Find): Enter then runs it.
    pub msg_insert: Option<usize>,
    /// Where scripts have put the message box. It's drawn under the card
    /// whatever this says, but scripts read back what they set.
    pub msg_rect: crate::model::Rect,
    /// Where the card's top left corner is on screen, in global coordinates,
    /// as `the rect of card window` reports it. The UI keeps it up to date;
    /// scripts move the window by setting it.
    pub window_origin: (i32, i32),
    /// How far the card is scrolled in a card window smaller than it, as
    /// scripts set it or the scroll bars do (see `scroll`).
    pub card_scroll: (i32, i32),
    /// The card window's size when it's smaller than the card, which a
    /// script or the user made it (see `card_window_size`); None shows the
    /// whole card.
    pub card_window: Option<(i32, i32)>,
    /// The card window's rect before `set the zoomed of card window to
    /// true`, while it's zoomed: setting it false puts it back there.
    pub zoomed_from: Option<crate::model::Rect>,
    /// Command-Shift-E has shown the card window's size box: the next click
    /// drags it.
    pub size_box: bool,
    /// The main screen, for `the screenRect`. The UI fills it in.
    pub screen_rect: crate::model::Rect,
    pub history: Vec<Id>,
    pub history_pos: usize,
    /// Cards visited, each once, most recent last, for the Recent dialog.
    pub recent: Vec<Id>,
    pub lock_screen: bool,
    /// `the lockRecent`: cards visited stay off the Recent list, until
    /// scripts finish.
    pub lock_recent: bool,
    /// `the dialingTime`: how long, in ticks, a modem's line stayed open
    /// after `dial`. There's no modem; scripts read back what they set.
    pub dialing_time: u32,
    /// `the dialingVolume`: how loud `dial`'s touch tones are, 0 to 7.
    pub dialing_volume: u32,
    pub menubar_visible: bool,
    /// The menu bar, as scripts have changed it.
    pub menus: Vec<super::menus::Menu>,
    pub mouse: MouseState,
    pub transition: Option<Transition>,
    pub transition_seq: u64,
    /// AddColor's switches and temporary items.
    pub addcolor: super::addcolor::AddColorState,
    /// The keys held down now, by name (`the keysDown`), kept by the UI.
    pub keys_down: Vec<String>,
    /// Unsaved changes.
    pub dirty: bool,
    /// Bumped whenever a picture changes so the UI can refresh textures.
    pub picture_rev: u64,
    pub requests: Vec<UiRequest>,
    /// Text windows scripts have opened (TEXWindow), in front last.
    pub script_windows: Vec<ScriptWindow>,
    /// Movie windows (HyperCard 2.4's Movie command), in front last. They
    /// stay open from card to card, as in HyperCard.
    pub movies: Vec<super::movies::MovieWindow>,
    /// The id the next movie window gets.
    pub next_movie_id: u32,
    /// The palette, as the UI shows it.
    pub palette_state: PaletteState,
    /// Where each palette was last shown, by lowercased name, so the
    /// `palette` command without a point puts it back there.
    pub palette_locs: HashMap<String, (i32, i32)>,
    /// A zoomRect animation: from, to, and when it started.
    pub zoom: Option<(crate::model::Rect, crate::model::Rect, std::time::Instant)>,
    pub dialog: Option<Dialog>,
    /// The button or field selected with the Button or Field tool, by the
    /// user or by `select button 1`.
    pub selected_part: Option<ObjRef>,
    /// What Copy and Cut last put on RandomCard's clipboard.
    pub clipboard: Option<Clip>,
    /// A script is running.
    pub busy: bool,
    pub found: Option<FieldRange>,
    /// The selected text, or the insertion point (an empty range), in a field.
    pub selection: Option<FieldRange>,
    /// Bumped when a script changes the selection, so the UI shows it.
    pub selection_rev: u64,
    /// Where the selection starts on the card, published by the UI.
    pub selection_loc: Option<(i32, i32)>,
    /// The word last clicked in a locked field.
    pub click: Option<FieldRange>,
    /// The field being edited, if any.
    pub focus: Option<(Layer, Id)>,
    pub beeps: u32,
    pub fill_shapes: bool,
    pub line_size: u32,
    /// The current paint pattern, numbered from 1.
    pub pattern: usize,
    pub brush: u32,
    pub poly_sides: u32,
    /// The spray can's circle across, in pixels (`the spraySize`), and the
    /// dots it sprays at a time (`the sprayDensity`): beyond HyperCard,
    /// whose spray was fixed at 16 and 12.
    pub spray_size: u32,
    pub spray_density: u32,
    /// The layer's picture before the last thing painted on it: what's
    /// changed since is HyperCard's new element, which Select selects and
    /// the Paint menu works on until the next press on the card.
    pub painted_from: Option<crate::runtime::undo::LayerPicture>,
    /// The layer's picture as last kept, for Revert. HyperCard held paint
    /// changes until Keep, going to another card or layer, or quitting.
    pub kept: Option<crate::runtime::undo::LayerPicture>,
    /// The space between shapes drawn with Draw Multiple (`the multiSpace`).
    pub multi_space: u32,
    /// The Options menu's settings: Grid (`the grid`), Draw Centered
    /// (`the centered`), Draw Multiple (`the multiple`), and Power Keys.
    pub grid: bool,
    pub centered: bool,
    pub multiple: bool,
    pub power_keys: bool,
    /// The paint colors: lines, borders, the pencil and text
    /// (`the penColor`), and the pattern's set bits (`the brushColor`).
    pub line_color: crate::model::Pixel,
    pub fill_color: crate::model::Pixel,
    /// Font, size and style for painted text.
    pub paint_font: String,
    pub paint_text_size: u32,
    /// The line height a script set for painted text (`the textHeight`);
    /// None for the size's own, a third more than the size.
    pub paint_text_height: Option<u32>,
    pub paint_text_style: String,
    /// Where scripts' `type` paints with the text tool (set by `click at`).
    pub text_point: Option<(i32, i32)>,
    /// Undo and redo history (see `undo.rs`).
    pub undo: Vec<super::undo::UndoEntry>,
    pub redo: Vec<super::undo::UndoEntry>,
    /// The user level, 1 Browsing … 5 Scripting. Global, as in HyperCard: it
    /// carries over from stack to stack. The stack's protection caps it.
    pub user_level: u8,
    /// The stack's password was given this session, lifting its level limit.
    pub unlocked: bool,
    /// Stacks loaded besides the current one: Home, stacks in use, and stacks
    /// left that have no file to come back to.
    pub others: Vec<LoadedStack>,
    pub next_stack_key: u32,
    /// The key the current stack had while it was loaded behind, when a
    /// script went to it: it keeps it, so a handler of it that's running
    /// still finds it, and gets it back when another stack comes in.
    pub current_key: Option<u32>,
    /// Stacks open in card windows behind the current one, the most
    /// recently in front first. The current stack's window is `window_key`.
    pub windows: Vec<StackWindow>,
    /// The current card window: 0 for the first, which RandomCard opens
    /// with; others take their stack's key.
    pub window_key: u32,
    /// Another stack is in for a moment, for a script to reach its cards
    /// and parts; the window keeps showing what it showed.
    pub lent: bool,
    /// Stacks in use (`start using`), by name, in message-path order.
    pub in_use: Vec<String>,
    /// Where the Home stack lives.
    pub home_path: Option<PathBuf>,
    /// Counts stack changes, so the UI can drop windows of the old stack.
    pub stack_serial: u64,
    /// WindName's title for the window, and the stack_serial it was set
    /// for: it lasts until another stack opens.
    pub window_title: Option<(u64, String)>,
    /// The kinds of data last copied, as the Mac's scrap types ("TEXT",
    /// "PICT"), for ClipInfo.
    pub clip_types: String,
    /// Text last copied from a field, with its styles, so pasting it
    /// into a field brings them along.
    pub text_clip: Option<TextClip>,
    /// Whether RandomCard's window is in front, for Notification.
    pub focused: bool,
    /// Where a script is stopped in the debugger.
    pub debug: Option<DebugPause>,
    /// Checkpoints: a script's owner and a line in it, from 1.
    pub checkpoints: Vec<(ObjRef, usize)>,
    /// Global variables as of the last pause or handler, for the variable watcher.
    pub globals_view: Vec<(String, String)>,
    /// Messages sent, while the message watcher is open.
    pub watch_messages: bool,
    pub message_log: std::collections::VecDeque<WatchedMessage>,
    /// Ticks (60ths of a second) to wait between lines while tracing.
    pub trace_delay: u32,
}

/// A script stopped in the debugger.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugPause {
    pub obj: ObjRef,
    pub handler: String,
    pub line: usize,
    /// The handler's local variables, by name.
    pub locals: Vec<(String, String)>,
    /// The handlers running, outermost first.
    pub calls: Vec<String>,
    /// False while the debugger runs on to the next stop (stepping, tracing).
    pub paused: bool,
}

/// What the debugger is told to do next.
#[derive(Clone, Debug, PartialEq)]
pub enum DebugCmd {
    /// Run to the next line of this handler, or of a handler it returns to.
    Step,
    /// Stop at the next line run, even inside a handler this one calls.
    StepInto,
    /// Run, showing each line as it goes.
    Trace,
    Go,
    SetVar { name: String, value: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WatchedMessage {
    /// Counts messages, to find this one again when it's been handled.
    pub seq: u64,
    pub name: String,
    /// How many handlers deep it was sent.
    pub depth: usize,
    pub handled: bool,
}

/// How many cards the Recent dialog shows.
pub const RECENT_CARDS: usize = 42;
/// How long a zoomRect animation takes.
pub const ZOOM_TIME: std::time::Duration = std::time::Duration::from_millis(250);

/// How many messages the message watcher keeps.
pub const MESSAGE_LOG_LEN: usize = 500;

/// Whether two references name the same script. A background part is the
/// same part on every card.
pub fn same_script(a: ObjRef, b: ObjRef) -> bool {
    match (a, b) {
        (ObjRef::Part { layer: Layer::Background, id: x, .. }, ObjRef::Part { layer: Layer::Background, id: y, .. }) => x == y,
        _ => a == b,
    }
}

pub struct LoadedStack {
    pub key: u32,
    pub stack: Stack,
}

/// What `lend_stack` changed, for `return_stack` to put back.
pub struct Lent {
    key: u32,
    card_id: Id,
    dirty: bool,
    undo: usize,
    redo: usize,
    lent: bool,
}

/// A stack open in a card window behind the current one, with what its
/// window keeps while it's there: the card it shows, its history and its
/// undo. Bringing it forward swaps these with the World's own.
pub struct StackWindow {
    pub key: u32,
    pub stack: Stack,
    pub card_id: Id,
    pub edit_bg: bool,
    pub history: Vec<Id>,
    pub history_pos: usize,
    pub recent: Vec<Id>,
    pub undo: Vec<super::undo::UndoEntry>,
    pub redo: Vec<super::undo::UndoEntry>,
    pub dirty: bool,
    pub window_origin: (i32, i32),
    pub card_scroll: (i32, i32),
    pub card_window: Option<(i32, i32)>,
    pub window_title: Option<(u64, String)>,
    pub unlocked: bool,
}

impl StackWindow {
    /// A new window for a stack, showing its first card.
    pub fn new(key: u32, stack: Stack, window_origin: (i32, i32)) -> StackWindow {
        let card_id = stack.cards[0].id;
        let dirty = stack.imported_from.is_some() && stack.path.is_none();
        StackWindow {
            key,
            stack,
            card_id,
            edit_bg: false,
            history: vec![card_id],
            history_pos: 0,
            recent: vec![card_id],
            undo: Vec::new(),
            redo: Vec::new(),
            dirty,
            window_origin,
            card_scroll: (0, 0),
            card_window: None,
            window_title: None,
            unlocked: false,
        }
    }
}

impl World {
    pub fn new(stack: Stack) -> World {
        let card_id = stack.cards[0].id;
        let (width, height) = (stack.width as i32, stack.height as i32);
        let msg_rect = crate::model::Rect::new(0, height, width, height + 30);
        // An imported HyperCard stack hasn't been saved in RandomCard's format yet.
        let dirty = stack.imported_from.is_some() && stack.path.is_none();
        World {
            stack,
            card_id,
            tool: Tool::Browse,
            edit_bg: false,
            msg: String::new(),
            msg_visible: false,
            msg_insert: None,
            msg_rect,
            window_origin: (0, 0),
            card_scroll: (0, 0),
            card_window: None,
            zoomed_from: None,
            size_box: false,
            screen_rect: crate::model::Rect::new(0, 0, 1920, 1080),
            history: vec![card_id],
            history_pos: 0,
            recent: vec![card_id],
            lock_screen: false,
            lock_recent: false,
            dialing_time: 180,
            dialing_volume: 7,
            menubar_visible: true,
            menus: super::menus::standard_menus(),
            mouse: MouseState::default(),
            transition: None,
            transition_seq: 0,
            addcolor: Default::default(),
            keys_down: Vec::new(),
            dirty,
            picture_rev: 0,
            requests: Vec::new(),
            script_windows: Vec::new(),
            movies: Vec::new(),
            next_movie_id: 1001,
            palette_state: Default::default(),
            palette_locs: HashMap::new(),
            zoom: None,
            dialog: None,
            selected_part: None,
            clipboard: None,
            busy: false,
            found: None,
            selection: None,
            selection_rev: 0,
            selection_loc: None,
            click: None,
            focus: None,
            beeps: 0,
            fill_shapes: false,
            line_size: 1,
            pattern: 1,
            brush: 3,
            poly_sides: 4,
            spray_size: crate::paint::SPRAY_SIZE,
            spray_density: crate::paint::SPRAY_DENSITY,
            painted_from: None,
            kept: None,
            multi_space: 1,
            grid: false,
            centered: false,
            multiple: false,
            power_keys: false,
            line_color: crate::model::Pixel::BLACK,
            fill_color: crate::model::Pixel::BLACK,
            paint_font: "Geneva".into(),
            paint_text_size: 12,
            paint_text_height: None,
            paint_text_style: "plain".into(),
            text_point: None,
            undo: Vec::new(),
            redo: Vec::new(),
            user_level: 5,
            unlocked: false,
            others: Vec::new(),
            next_stack_key: 1,
            current_key: None,
            windows: Vec::new(),
            window_key: 0,
            lent: false,
            in_use: Vec::new(),
            home_path: None,
            stack_serial: 0,
            window_title: None,
            clip_types: String::new(),
            text_clip: None,
            focused: true,
            debug: None,
            checkpoints: Vec::new(),
            globals_view: Vec::new(),
            watch_messages: false,
            message_log: Default::default(),
            trace_delay: 0,
        }
    }

    /// The card window's size: the card's, or smaller, but never bigger.
    pub fn card_window_size(&self) -> (i32, i32) {
        let (cw, ch) = (self.stack.width as i32, self.stack.height as i32);
        self.card_window.map_or((cw, ch), |(w, h)| (w.min(cw), h.min(ch)))
    }

    /// How far the card is scrolled in its window: as far as it was set,
    /// but no further than shows the card's far edge.
    pub fn scroll(&self) -> (i32, i32) {
        let (ww, wh) = self.card_window_size();
        let (cw, ch) = (self.stack.width as i32, self.stack.height as i32);
        (self.card_scroll.0.clamp(0, cw - ww), self.card_scroll.1.clamp(0, ch - wh))
    }

    /// Make the card window this size, within the card's. Returns whether
    /// that changed it.
    pub fn set_card_window_size(&mut self, width: i32, height: i32) -> bool {
        let before = self.card_window_size();
        let (cw, ch) = (self.stack.width as i32, self.stack.height as i32);
        let min = *crate::model::CARD_SIZES.start() as i32;
        let size = (width.clamp(min.min(cw), cw), height.clamp(min.min(ch), ch));
        self.card_window = (size != (cw, ch)).then_some(size);
        self.card_window_size() != before
    }

    /// Show another stack. The one being left is saved if it has a file,
    /// and stays loaded if it's Home, in use, or has no file to reopen.
    pub fn replace_stack(&mut self, stack: Stack) {
        let mut fresh = World::new(stack);
        fresh.msg_visible = self.msg_visible;
        fresh.msg_rect = self.msg_rect;
        fresh.window_origin = self.window_origin;
        fresh.script_windows = std::mem::take(&mut self.script_windows);
        fresh.screen_rect = self.screen_rect;
        fresh.msg = std::mem::take(&mut self.msg);
        fresh.user_level = self.user_level;
        fresh.others = std::mem::take(&mut self.others);
        fresh.next_stack_key = self.next_stack_key;
        let old_key = self.current_key.take();
        fresh.windows = std::mem::take(&mut self.windows);
        fresh.window_key = self.window_key;
        fresh.in_use = std::mem::take(&mut self.in_use);
        fresh.home_path = self.home_path.take();
        fresh.stack_serial = self.stack_serial + 1;
        fresh.globals_view = std::mem::take(&mut self.globals_view);
        fresh.watch_messages = self.watch_messages;
        fresh.message_log = std::mem::take(&mut self.message_log);
        fresh.trace_delay = self.trace_delay;
        fresh.picture_rev = rand::random();
        let dirty = self.dirty && !self.stack.cant_modify;
        let old = std::mem::replace(self, fresh).stack;
        // Changes with nowhere to be saved keep the stack loaded.
        let unsaved = match &old.path {
            Some(p) => dirty && old.save(p).is_err(),
            None => dirty,
        };
        if unsaved || self.is_home(&old) || self.is_in_use(&old.name) {
            match old_key {
                Some(key) => self.others.push(LoadedStack { key, stack: old }),
                None => {
                    self.add_other(old);
                }
            }
        }
        self.load_home();
    }

    /// Swap the current window's stack and what goes with it for a window's.
    fn swap_window(&mut self, sw: &mut StackWindow) {
        use std::mem::swap;
        swap(&mut self.window_key, &mut sw.key);
        swap(&mut self.stack, &mut sw.stack);
        swap(&mut self.card_id, &mut sw.card_id);
        swap(&mut self.edit_bg, &mut sw.edit_bg);
        swap(&mut self.history, &mut sw.history);
        swap(&mut self.history_pos, &mut sw.history_pos);
        swap(&mut self.recent, &mut sw.recent);
        swap(&mut self.undo, &mut sw.undo);
        swap(&mut self.redo, &mut sw.redo);
        swap(&mut self.dirty, &mut sw.dirty);
        swap(&mut self.window_origin, &mut sw.window_origin);
        swap(&mut self.card_scroll, &mut sw.card_scroll);
        swap(&mut self.card_window, &mut sw.card_window);
        swap(&mut self.window_title, &mut sw.window_title);
        swap(&mut self.unlocked, &mut sw.unlocked);
        // What was selected or found belongs to the window left behind.
        self.found = None;
        self.selection = None;
        self.click = None;
        self.focus = None;
        self.text_point = None;
        self.size_box = false;
        self.transition = None;
        self.stack_serial += 1;
        self.picture_rev = rand::random();
    }

    /// Open a stack in a new card window, in front.
    pub fn open_window(&mut self, stack: Stack) {
        let key = self.next_stack_key;
        self.next_stack_key += 1;
        let (x, y) = self.window_origin;
        let mut sw = StackWindow::new(key, stack, (x + 24, y + 24));
        self.swap_window(&mut sw);
        self.windows.insert(0, sw);
    }

    /// Bring a card window forward; false if there's none by that key.
    pub fn front_window(&mut self, key: u32) -> bool {
        let Some(i) = self.windows.iter().position(|sw| sw.key == key) else { return false };
        let mut sw = self.windows.remove(i);
        self.swap_window(&mut sw);
        self.windows.insert(0, sw);
        true
    }

    /// Close the current card window, bringing the one behind it forward.
    /// Its stack is saved, and stays loaded if it's Home, in use, or has
    /// changes with nowhere to go. False if it's the only window.
    pub fn close_front_window(&mut self) -> bool {
        if self.windows.is_empty() {
            return false;
        }
        let mut sw = self.windows.remove(0);
        self.swap_window(&mut sw);
        let dirty = sw.dirty && !sw.stack.cant_modify;
        let old = sw.stack;
        let unsaved = match &old.path {
            Some(p) => dirty && old.save(p).is_err(),
            None => dirty,
        };
        if unsaved || self.is_home(&old) || self.is_in_use(&old.name) {
            self.add_other(old);
        }
        true
    }

    /// The card window showing a stack, by name or path: the current one's
    /// key or one behind it.
    pub fn window_of(&self, name: &str, path: Option<&std::path::Path>) -> Option<u32> {
        let shows = |s: &Stack| names_stack(s, name) || path.is_some() && same_path(stack_file(s), path);
        if shows(&self.stack) {
            return Some(self.window_key);
        }
        self.windows.iter().find(|sw| shows(&sw.stack)).map(|sw| sw.key)
    }

    /// Lend a loaded stack for a moment, as if it were current, so a script
    /// can reach `field 1 of card 2 of stack "X"`: it comes in at its own
    /// card. `return_stack` puts everything back.
    pub fn lend_stack(&mut self, key: u32) -> Option<Lent> {
        let (stack, card) = if let Some(sw) = self.windows.iter_mut().find(|sw| sw.key == key) {
            let card = sw.card_id;
            (&mut sw.stack, card)
        } else {
            let o = self.others.iter_mut().find(|o| o.key == key)?;
            let card = o.stack.cards[0].id;
            (&mut o.stack, card)
        };
        std::mem::swap(&mut self.stack, stack);
        let lent = Lent {
            key,
            card_id: std::mem::replace(&mut self.card_id, card),
            dirty: std::mem::replace(&mut self.dirty, false),
            undo: self.undo.len(),
            redo: self.redo.len(),
            lent: std::mem::replace(&mut self.lent, true),
        };
        Some(lent)
    }

    /// Put back a lent stack. Its changes are saved if it's only loaded,
    /// or wait in its window to be saved; the current stack's undo history
    /// doesn't take them.
    pub fn return_stack(&mut self, lent: Lent) {
        let changed = std::mem::replace(&mut self.dirty, lent.dirty);
        self.card_id = lent.card_id;
        self.undo.truncate(lent.undo);
        self.redo.truncate(lent.redo);
        self.lent = lent.lent;
        if let Some(sw) = self.windows.iter_mut().find(|sw| sw.key == lent.key) {
            std::mem::swap(&mut self.stack, &mut sw.stack);
            sw.dirty |= changed;
        } else if let Some(o) = self.others.iter_mut().find(|o| o.key == lent.key) {
            std::mem::swap(&mut self.stack, &mut o.stack);
            if changed && !o.stack.cant_modify && let Some(p) = &o.stack.path {
                let _ = o.stack.save(p);
            }
        }
    }

    /// Every open stack, the current one first, then the windows behind.
    pub fn open_stacks(&self) -> impl Iterator<Item = &Stack> {
        std::iter::once(&self.stack).chain(self.windows.iter().map(|sw| &sw.stack))
    }

    /// Remember a card for the Recent dialog, which holds 42 as HyperCard's did.
    pub fn note_recent(&mut self, id: Id) {
        if self.lock_recent {
            return;
        }
        self.recent.retain(|c| *c != id);
        self.recent.push(id);
        if self.recent.len() > RECENT_CARDS {
            self.recent.remove(0);
        }
    }

    pub fn is_home(&self, s: &Stack) -> bool {
        same_path(s.path.as_deref(), self.home_path.as_deref())
    }

    pub fn is_in_use(&self, name: &str) -> bool {
        self.in_use.iter().any(|n| n.eq_ignore_ascii_case(name))
    }

    /// Keep a stack loaded; returns its key.
    pub fn add_other(&mut self, stack: Stack) -> u32 {
        let key = self.next_stack_key;
        self.next_stack_key += 1;
        self.others.push(LoadedStack { key, stack });
        key
    }

    /// Whether a key is the current stack's: the one it had loaded behind,
    /// or its card window's.
    fn is_current_key(&self, key: u32) -> bool {
        self.current_key == Some(key) || key != 0 && key == self.window_key
    }

    /// A loaded stack by key: one kept loaded, or one in a window behind,
    /// or the current stack if the key is its.
    pub fn other(&self, key: u32) -> Option<&Stack> {
        if self.is_current_key(key) {
            return Some(&self.stack);
        }
        let open = self.windows.iter().find(|sw| sw.key == key).map(|sw| &sw.stack);
        open.or_else(|| self.others.iter().find(|o| o.key == key).map(|o| &o.stack))
    }

    pub fn other_mut(&mut self, key: u32) -> Option<&mut Stack> {
        if self.is_current_key(key) {
            return Some(&mut self.stack);
        }
        if let Some(sw) = self.windows.iter_mut().find(|sw| sw.key == key) {
            return Some(&mut sw.stack);
        }
        self.others.iter_mut().find(|o| o.key == key).map(|o| &mut o.stack)
    }

    /// A loaded stack by name, or by file path.
    pub fn find_other(&self, name: &str, path: Option<&std::path::Path>) -> Option<u32> {
        let shows = |s: &Stack| names_stack(s, name) || path.is_some() && same_path(stack_file(s), path);
        let open = self.windows.iter().find(|sw| shows(&sw.stack)).map(|sw| sw.key);
        open.or_else(|| self.others.iter().find(|o| shows(&o.stack)).map(|o| o.key))
    }

    /// Take a stack out of the loaded ones, to show it.
    pub fn take_other(&mut self, key: u32) -> Option<Stack> {
        let i = self.others.iter().position(|o| o.key == key)?;
        Some(self.others.remove(i).stack)
    }

    /// The loaded Home stack, unless it's the current stack.
    pub fn home_key(&self) -> Option<u32> {
        let open = self.windows.iter().find(|sw| self.is_home(&sw.stack)).map(|sw| sw.key);
        open.or_else(|| self.others.iter().find(|o| self.is_home(&o.stack)).map(|o| o.key))
    }

    /// Make sure Home is loaded, for the message path.
    pub fn load_home(&mut self) {
        let Some(path) = self.home_path.clone() else { return };
        if self.is_home(&self.stack) || self.home_key().is_some() {
            return;
        }
        if let Ok(mut home) = Stack::load(&path) {
            if crate::model::home::update(&mut home) {
                let _ = home.save(&path);
            }
            crate::model::home::show_level(&mut home, self.user_level);
            self.add_other(home);
        }
    }

    /// The Home stack, if it's loaded: the current stack or another.
    pub fn home_mut(&mut self) -> Option<&mut Stack> {
        if self.is_home(&self.stack) {
            return Some(&mut self.stack);
        }
        let key = self.home_key()?;
        self.other_mut(key)
    }

    /// Show the user level on Home's User Preferences card, where stacks
    /// made for HyperCard read it.
    pub fn show_level_at_home(&mut self) {
        let level = self.user_level;
        if let Some(home) = self.home_mut() {
            crate::model::home::show_level(home, level);
        }
    }

    /// The highest level the stack allows right now.
    pub fn level_limit(&self) -> u8 {
        if self.unlocked { 5 } else { self.stack.protection.user_level_limit.clamp(1, 5) }
    }

    /// The level in effect: the user level, capped by the stack's protection.
    pub fn level(&self) -> u8 {
        self.user_level.min(self.level_limit())
    }

    /// The level for the user's own actions: a Can't Modify stack can only
    /// be browsed, though its scripts still run.
    pub fn editing_level(&self) -> u8 {
        if self.stack.cant_modify { 1 } else { self.level() }
    }

    /// The selection, if it's in a field on the current card.
    pub fn current_selection(&self) -> Option<&FieldRange> {
        self.selection.as_ref().filter(|r| r.card == self.card_id)
    }

    /// Select text in a field, for scripts: the UI follows.
    pub fn set_selection(&mut self, range: Option<FieldRange>) {
        self.selection = range;
        self.selection_rev += 1;
    }

    /// Copy Text: the characters of `r` with their fonts, sizes and styles.
    pub fn copy_text(&self, r: &FieldRange) -> TextClip {
        use crate::model::styles::{self, TextStyle};
        let runs = self.stack.field_styles(r.card, r.layer, r.field);
        let own = self.stack.part(r.card, r.layer, r.field).map_or_else(TextStyle::default, |p| TextStyle {
            font: Some(p.text_font.clone()),
            size: Some(p.text_size),
            style: Some(p.text_style.clone()),
            color: p.fore_color,
        });
        TextClip { text: self.range_text(r), runs: styles::copy_runs(&runs, r.start, r.end.max(r.start), &own) }
    }

    /// Paste Text: `r` replaced by the copied text, which keeps its styles.
    /// The insertion point goes after it. An empty clip deletes `r` (Cut).
    pub fn paste_text(&mut self, r: &FieldRange, clip: &TextClip) {
        use crate::model::styles;
        let old = self.stack.field_text(r.card, r.layer, r.field);
        let len = old.chars().count();
        let (start, end) = (r.start.min(len), r.end.clamp(r.start.min(len), len));
        let inserted = clip.text.chars().count();
        let text: String = old.chars().take(start).chain(clip.text.chars()).chain(old.chars().skip(end)).collect();
        let new_len = text.chars().count();
        let mut runs = self.stack.field_styles(r.card, r.layer, r.field);
        styles::edit(&mut runs, start, end - start, inserted, new_len);
        styles::paste_runs(&mut runs, start, &clip.runs, inserted, new_len);
        self.stack.set_field_content(r.card, r.layer, r.field, text, runs);
        self.dirty = true;
        let at = start + inserted;
        self.set_selection(Some(FieldRange { start: at, end: at, ..r.clone() }));
    }

    pub fn range_text(&self, r: &FieldRange) -> String {
        let text = self.stack.field_text(r.card, r.layer, r.field);
        text.chars().skip(r.start).take(r.end.saturating_sub(r.start)).collect()
    }

    /// How HyperCard names a range's field: "card field 2", "bkgnd field 1".
    pub fn range_field_name(&self, r: &FieldRange) -> String {
        let fields = self.stack.parts(r.card, r.layer).iter().filter(|p| p.kind == crate::model::PartKind::Field);
        format!("{} field {}", r.layer.name(), fields.take_while(|p| p.id != r.field).count() + 1)
    }

    pub fn bg_id(&self) -> Id {
        self.stack.card(self.card_id).map(|c| c.background).unwrap_or(0)
    }

    pub fn card_index(&self) -> usize {
        self.stack.card_index(self.card_id).unwrap_or(0)
    }

    pub fn current_layer(&self) -> Layer {
        if self.edit_bg { Layer::Background } else { Layer::Card }
    }

    /// The picture paint tools draw on: the card's, or the background's
    /// while editing the background.
    pub fn layer_picture_mut(&mut self) -> Option<&mut Picture> {
        let (card, bg) = (self.card_id, self.bg_id());
        match self.current_layer() {
            Layer::Card => self.stack.card_mut(card).map(|c| &mut c.picture),
            Layer::Background => self.stack.background_mut(bg).map(|b| &mut b.picture),
        }
    }

    pub fn paint_settings(&self) -> crate::paint::Settings {
        crate::paint::Settings {
            line_size: self.line_size,
            filled: self.fill_shapes,
            pattern: self.stack.pattern(self.pattern),
            brush: self.brush,
            poly_sides: self.poly_sides,
            spray_size: self.spray_size,
            spray_density: self.spray_density,
            centered: self.centered,
            line_color: self.line_color,
            fill_color: self.fill_color,
        }
    }
}

/// The whitespace-delimited word around character `at`, as character offsets
/// (an empty range if `at` is on a space or past the end).
pub fn word_at(text: &str, at: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    if at >= chars.len() || chars[at].is_whitespace() {
        return (at, at);
    }
    let start = (0..at).rev().take_while(|&i| !chars[i].is_whitespace()).last().unwrap_or(at);
    let end = (at..chars.len()).find(|&i| chars[i].is_whitespace()).unwrap_or(chars.len());
    (start, end)
}

/// The line holding character `at`, without its return, as character offsets.
pub fn line_at(text: &str, at: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let at = at.min(chars.len());
    let start = (0..at).rev().find(|&i| chars[i] == '\n').map(|i| i + 1).unwrap_or(0);
    let end = (at..chars.len()).find(|&i| chars[i] == '\n').unwrap_or(chars.len());
    (start, end)
}

/// The 1-based line number of character `at`.
pub fn line_number(text: &str, at: usize) -> usize {
    text.chars().take(at).filter(|c| *c == '\n').count() + 1
}

/// Whether two stack files are the same file.
pub fn same_path(a: Option<&std::path::Path>, b: Option<&std::path::Path>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b || a.canonicalize().ok().is_some_and(|x| b.canonicalize().ok() == Some(x)),
        _ => false,
    }
}

/// Whether a name refers to a stack: its name, its file's name, or its path.
/// The file a stack came from: its own, or the HyperCard stack it was
/// converted from, until it's saved.
pub fn stack_file(s: &Stack) -> Option<&std::path::Path> {
    s.path.as_deref().or(s.imported_from.as_deref())
}

pub fn names_stack(s: &Stack, name: &str) -> bool {
    let name = name.trim();
    s.name.eq_ignore_ascii_case(name)
        || stack_file(s).is_some_and(|p| {
            p.to_string_lossy().eq_ignore_ascii_case(name)
                || p.file_stem().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))
        })
}

/// What Copy and Cut put on RandomCard's own clipboard: a part, or painted
/// pixels. (Text goes on the system's.)
#[derive(Clone, Debug)]
pub enum Clip {
    /// A button or field, with the text and hilite it had on its card.
    Part { part: Box<crate::model::Part>, text: String, styles: Vec<crate::model::styles::StyleRun>, hilite: bool },
    /// Painted pixels and where they came from.
    Picture { pixels: crate::model::Picture, rect: (i32, i32, i32, i32) },
}

impl World {
    /// A part as Copy takes it: with the text and hilite it has on this card.
    pub fn copy_part(&self, obj: ObjRef) -> Option<Clip> {
        let ObjRef::Part { layer, id, card } = obj else { return None };
        let part = self.stack.part(card, layer, id).cloned()?;
        Some(Clip::Part {
            part: Box::new(part),
            text: self.stack.field_text(card, layer, id),
            styles: self.stack.field_styles(card, layer, id),
            hilite: self.stack.hilite(card, layer, id),
        })
    }

    /// Paste a part onto the layer being edited, keeping its text and
    /// hilite; the new part's reference.
    pub fn paste_part(
        &mut self,
        mut part: crate::model::Part,
        text: String,
        styles: Vec<crate::model::styles::StyleRun>,
        hilite: bool,
    ) -> ObjRef {
        use crate::model::{Layer, PartKind};
        let kind = part.kind;
        let layer = self.current_layer();
        self.record_parts(layer);
        let (card, bg) = (self.card_id, self.bg_id());
        let id = self.stack.alloc_id();
        part.id = id;
        match layer {
            Layer::Card => {
                if kind == PartKind::Field {
                    part.contents = text;
                    part.styles = styles;
                }
                part.hilite = hilite;
                if let Some(c) = self.stack.card_mut(card) {
                    c.parts.push(part);
                }
            }
            Layer::Background => {
                // Background text and hilites may belong to each card.
                let per_card_text = kind == PartKind::Field && !part.shared_text;
                let per_card_hilite = kind == PartKind::Button && !part.shared_hilite;
                if kind == PartKind::Field {
                    part.contents = if per_card_text { String::new() } else { text.clone() };
                    part.styles = if per_card_text { Vec::new() } else { styles.clone() };
                }
                if let Some(c) = self.stack.card_mut(card) {
                    if per_card_text {
                        c.bg_text.insert(id, text);
                        if !styles.is_empty() {
                            c.bg_styles.insert(id, styles);
                        }
                    }
                    if per_card_hilite {
                        c.bg_hilite.insert(id, hilite);
                    }
                }
                if !per_card_hilite {
                    part.hilite = hilite;
                }
                if let Some(b) = self.stack.background_mut(bg) {
                    b.parts.push(part);
                }
            }
        }
        self.dirty = true;
        ObjRef::Part { layer, id, card }
    }
}
