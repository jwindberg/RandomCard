//! State shared between the UI thread and the script thread.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

#[derive(Clone, Debug)]
pub struct Transition {
    pub effect: String,
    pub seconds: f32,
    pub seq: u64,
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

/// Things only the UI thread can do, requested by scripts or menu commands.
#[derive(Clone, Debug)]
pub enum UiRequest {
    OpenStack,
    ImportStack,
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
    ProtectStack,
    Preferences,
    ToggleToolPanel,
    /// Show the Recent dialog.
    Recent,
    /// Put the cursor in the message box at this character.
    FocusMessage(usize),
    /// Show or hide the message or variable watcher.
    MessageWatcher(bool),
    VariableWatcher(bool),
    /// A key's message wasn't handled: do what the key does by default.
    Key(ObjRef, KeyAction),
}

/// Messages from the UI to the script thread.
#[derive(Clone, Debug)]
pub enum Event {
    Startup,
    Send { target: ObjRef, message: String, args: Vec<String> },
    MsgBox(String),
    DoMenu(String),
    /// A keyboard message (keyDown, returnInField, tabKey, ...), with what
    /// the key does if no handler takes it.
    Key { target: ObjRef, message: String, args: Vec<String>, default: KeyAction },
    /// Open a stack file (RandomCard or HyperCard), asking for its password
    /// if it needs one.
    OpenFile(std::path::PathBuf),
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
    /// Cards visited, each once, most recent last, for the Recent dialog.
    pub recent: Vec<Id>,
    pub lock_screen: bool,
    pub menubar_visible: bool,
    /// The menu bar, as scripts have changed it.
    pub menus: Vec<super::menus::Menu>,
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
    /// Font, size and style for painted text.
    pub paint_font: String,
    pub paint_text_size: u32,
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
    /// Stacks in use (`start using`), by name, in message-path order.
    pub in_use: Vec<String>,
    /// Where the Home stack lives.
    pub home_path: Option<PathBuf>,
    /// Counts stack changes, so the UI can drop windows of the old stack.
    pub stack_serial: u64,
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

impl World {
    pub fn new(stack: Stack) -> World {
        let card_id = stack.cards[0].id;
        // An imported HyperCard stack hasn't been saved in RandomCard's format yet.
        let dirty = stack.imported_from.is_some() && stack.path.is_none();
        World {
            stack,
            card_id,
            tool: Tool::Browse,
            edit_bg: false,
            msg: String::new(),
            msg_visible: false,
            history: vec![card_id],
            history_pos: 0,
            recent: vec![card_id],
            lock_screen: false,
            menubar_visible: true,
            menus: super::menus::standard_menus(),
            mouse: MouseState::default(),
            transition: None,
            transition_seq: 0,
            dirty,
            picture_rev: 0,
            requests: Vec::new(),
            dialog: None,
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
            paint_font: "Geneva".into(),
            paint_text_size: 12,
            paint_text_style: "plain".into(),
            text_point: None,
            undo: Vec::new(),
            redo: Vec::new(),
            user_level: 5,
            unlocked: false,
            others: Vec::new(),
            next_stack_key: 1,
            in_use: Vec::new(),
            home_path: None,
            stack_serial: 0,
            debug: None,
            checkpoints: Vec::new(),
            globals_view: Vec::new(),
            watch_messages: false,
            message_log: Default::default(),
            trace_delay: 0,
        }
    }

    /// Show another stack. The one being left is saved if it has a file,
    /// and stays loaded if it's Home, in use, or has no file to reopen.
    pub fn replace_stack(&mut self, stack: Stack) {
        let mut fresh = World::new(stack);
        fresh.msg_visible = self.msg_visible;
        fresh.msg = std::mem::take(&mut self.msg);
        fresh.user_level = self.user_level;
        fresh.others = std::mem::take(&mut self.others);
        fresh.next_stack_key = self.next_stack_key;
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
            self.add_other(old);
        }
        self.load_home();
    }

    /// Remember a card for the Recent dialog, which holds 42 as HyperCard's did.
    pub fn note_recent(&mut self, id: Id) {
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

    pub fn other(&self, key: u32) -> Option<&Stack> {
        self.others.iter().find(|o| o.key == key).map(|o| &o.stack)
    }

    pub fn other_mut(&mut self, key: u32) -> Option<&mut Stack> {
        self.others.iter_mut().find(|o| o.key == key).map(|o| &mut o.stack)
    }

    /// A loaded stack by name, or by file path.
    pub fn find_other(&self, name: &str, path: Option<&std::path::Path>) -> Option<u32> {
        self.others
            .iter()
            .find(|o| names_stack(&o.stack, name) || path.is_some() && same_path(o.stack.path.as_deref(), path))
            .map(|o| o.key)
    }

    /// Take a stack out of the loaded ones, to show it.
    pub fn take_other(&mut self, key: u32) -> Option<Stack> {
        let i = self.others.iter().position(|o| o.key == key)?;
        Some(self.others.remove(i).stack)
    }

    /// The loaded Home stack, unless it's the current stack.
    pub fn home_key(&self) -> Option<u32> {
        self.others.iter().find(|o| self.is_home(&o.stack)).map(|o| o.key)
    }

    /// Make sure Home is loaded, for the message path.
    pub fn load_home(&mut self) {
        let Some(path) = self.home_path.clone() else { return };
        if self.is_home(&self.stack) || self.home_key().is_some() {
            return;
        }
        if let Ok(home) = Stack::load(&path) {
            self.add_other(home);
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
pub fn names_stack(s: &Stack, name: &str) -> bool {
    let name = name.trim();
    s.name.eq_ignore_ascii_case(name)
        || s.path.as_ref().is_some_and(|p| {
            p.to_string_lossy().eq_ignore_ascii_case(name)
                || p.file_stem().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))
        })
}
