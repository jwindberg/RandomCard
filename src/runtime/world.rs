//! State shared between the UI thread and the script thread.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::model::{Id, Layer, Stack};

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
    MsgBox,
    HyperCard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Browse,
    Button,
    Field,
    Select,
    Pencil,
    Brush,
    Eraser,
    Line,
    Spray,
    Rect,
    RoundRect,
    Bucket,
    Oval,
}

impl Tool {
    pub const ALL: &'static [Tool] = &[
        Tool::Browse,
        Tool::Button,
        Tool::Field,
        Tool::Select,
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Line,
        Tool::Spray,
        Tool::Rect,
        Tool::RoundRect,
        Tool::Bucket,
        Tool::Oval,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Browse => "browse",
            Tool::Button => "button",
            Tool::Field => "field",
            Tool::Select => "select",
            Tool::Pencil => "pencil",
            Tool::Brush => "brush",
            Tool::Eraser => "eraser",
            Tool::Line => "line",
            Tool::Spray => "spray can",
            Tool::Rect => "rectangle",
            Tool::RoundRect => "round rect",
            Tool::Bucket => "bucket",
            Tool::Oval => "oval",
        }
    }

    pub fn is_paint(self) -> bool {
        !matches!(self, Tool::Browse | Tool::Button | Tool::Field)
    }

    /// Accepts names like "browse", "round rect tool" or HyperCard tool numbers.
    pub fn parse(s: &str) -> Option<Tool> {
        let s = s.trim().to_ascii_lowercase();
        let s = s.strip_suffix(" tool").unwrap_or(&s).trim().to_string();
        if let Ok(n) = s.parse::<u32>() {
            return Some(match n {
                1 => Tool::Browse,
                2 => Tool::Button,
                3 => Tool::Field,
                4 | 5 => Tool::Select,
                6 | 15 => Tool::Pencil,
                7 => Tool::Brush,
                8 => Tool::Eraser,
                9 | 17 | 18 => Tool::Line,
                10 => Tool::Spray,
                11 => Tool::Rect,
                12 => Tool::RoundRect,
                13 => Tool::Bucket,
                14 => Tool::Oval,
                16 => Tool::Browse,
                _ => return None,
            });
        }
        Some(match s.as_str() {
            "browse" => Tool::Browse,
            "button" => Tool::Button,
            "field" => Tool::Field,
            "select" | "lasso" => Tool::Select,
            "pencil" | "curve" => Tool::Pencil,
            "brush" => Tool::Brush,
            "eraser" => Tool::Eraser,
            "line" | "poly" | "polygon" | "reg poly" | "regular polygon" | "reg polygon" | "regular poly" => {
                Tool::Line
            }
            "spray" | "spray can" => Tool::Spray,
            "rect" | "rectangle" => Tool::Rect,
            "round rect" | "round rectangle" | "roundrect" => Tool::RoundRect,
            "bucket" => Tool::Bucket,
            "oval" => Tool::Oval,
            "text" => Tool::Browse,
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
}

#[derive(Clone, Debug)]
pub struct Transition {
    pub effect: String,
    pub seconds: f32,
    pub seq: u64,
}

#[derive(Clone, Debug)]
pub struct Found {
    pub card: Id,
    pub layer: Layer,
    pub field: Id,
    /// Character offsets within the field text.
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
pub enum Dialog {
    Answer { prompt: String, buttons: Vec<String> },
    Ask { prompt: String, default: String, password: bool },
}

#[derive(Clone, Debug)]
pub enum DialogReply {
    Button(String),
    Text(Option<String>),
}

/// Things only the UI thread can do, requested by scripts or menu commands.
#[derive(Clone, Debug)]
pub enum UiRequest {
    OpenStack,
    NewStack,
    SaveStackAs(Option<String>),
    Quit,
    SelectedPartInfo,
    CardInfo,
    BgInfo,
    StackInfo,
    EditScript(ObjRef),
    ScriptError { msg: String, obj: Option<ObjRef>, line: Option<usize> },
    BringCloser,
    SendFarther,
    Undo,
    SelectPart(ObjRef),
}

/// Messages from the UI to the script thread.
#[derive(Clone, Debug)]
pub enum Event {
    Startup,
    Send { target: ObjRef, message: String, args: Vec<String> },
    MsgBox(String),
    DoMenu(String),
    /// Replace the open stack (already loaded by the UI) and send open messages.
    OpenStack(Box<Stack>),
    Shutdown,
}

pub struct World {
    pub stack: Stack,
    pub card_id: Id,
    pub tool: Tool,
    pub edit_bg: bool,
    pub msg: String,
    pub msg_visible: bool,
    pub history: Vec<Id>,
    pub history_pos: usize,
    pub lock_screen: bool,
    pub menubar_visible: bool,
    pub mouse: MouseState,
    pub transition: Option<Transition>,
    pub transition_seq: u64,
    /// Unsaved changes.
    pub dirty: bool,
    /// Bumped whenever a picture changes so the UI can refresh textures.
    pub picture_rev: u64,
    pub requests: Vec<UiRequest>,
    pub dialog: Option<Dialog>,
    /// A script is running.
    pub busy: bool,
    pub found: Option<Found>,
    /// The field being edited, if any.
    pub focus: Option<(Layer, Id)>,
    pub beeps: u32,
    pub fill_shapes: bool,
    pub line_size: u32,
}

impl World {
    pub fn new(stack: Stack) -> World {
        let card_id = stack.cards[0].id;
        World {
            stack,
            card_id,
            tool: Tool::Browse,
            edit_bg: false,
            msg: String::new(),
            msg_visible: false,
            history: vec![card_id],
            history_pos: 0,
            lock_screen: false,
            menubar_visible: true,
            mouse: MouseState::default(),
            transition: None,
            transition_seq: 0,
            dirty: false,
            picture_rev: 0,
            requests: Vec::new(),
            dialog: None,
            busy: false,
            found: None,
            focus: None,
            beeps: 0,
            fill_shapes: false,
            line_size: 1,
        }
    }

    pub fn replace_stack(&mut self, stack: Stack) {
        let msg_visible = self.msg_visible;
        let msg = std::mem::take(&mut self.msg);
        *self = World::new(stack);
        self.msg_visible = msg_visible;
        self.msg = msg;
        self.picture_rev = rand::random();
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
}
