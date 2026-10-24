//! Syntax tree for HyperTalk scripts.

use std::collections::HashMap;

use crate::model::{Layer, PartKind};

#[derive(Debug, Default)]
pub struct Script {
    pub handlers: HashMap<String, Handler>,
    pub functions: HashMap<String, Handler>,
}

#[derive(Debug)]
pub struct Handler {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ordinal {
    Nth(i64),
    Middle,
    Last,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkKind {
    Char,
    Word,
    Item,
    Line,
}

#[derive(Clone, Debug)]
pub enum ChunkIndex {
    Ordinal(Ordinal),
    Expr(Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub kind: ChunkKind,
    pub start: ChunkIndex,
    pub end: Option<Box<Expr>>,
}

/// How a specific card, background or part is chosen.
#[derive(Clone, Debug)]
pub enum Which {
    Id(Box<Expr>),
    /// A number or a name; decided at runtime.
    Expr(Box<Expr>),
    Ordinal(Ordinal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    This,
    Next,
    Prev,
}

#[derive(Clone, Debug)]
pub enum CardSpec {
    Position(Position),
    Which(Which),
    Marked(Which),
    MarkedPosition(Position),
    Recent,
    Back,
    Forth,
    OfBackground(Box<CardSpec>, Box<BgSpec>),
}

#[derive(Clone, Debug)]
pub enum BgSpec {
    Position(Position),
    Which(Which),
}

#[derive(Clone, Debug)]
pub enum ObjSpec {
    Me,
    Target,
    MsgBox,
    HyperCard,
    /// `this stack` (None) or `stack "name"`.
    Stack(Option<Box<Expr>>),
    Card(CardSpec),
    Background(BgSpec),
    Part {
        /// None means "part N" (any kind).
        kind: Option<PartKind>,
        /// None means the default layer for the kind (card buttons, bkgnd fields).
        layer: Option<Layer>,
        which: Which,
        card: Option<Box<CardSpec>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameLength {
    Default,
    Short,
    Long,
    Abbrev,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Pow,
    Mul,
    Div,
    IntDiv,
    Mod,
    Add,
    Sub,
    Concat,
    ConcatSpace,
    Lt,
    Gt,
    Le,
    Ge,
    Contains,
    IsIn,
    IsNotIn,
    IsWithin,
    IsNotWithin,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Clone, Debug)]
pub enum Countable {
    Cards(Option<Box<Expr>>),
    MarkedCards,
    Backgrounds,
    Parts { kind: Option<PartKind>, layer: Layer, of: Option<Box<ObjSpec>> },
    Chunks(ChunkKind, Box<Expr>),
    Menus,
    MenuItems(Box<ChunkIndex>),
}

/// `menu "Edit"`, `menuItem 2 of menu "Go"`, `last menuItem of menu 3`.
#[derive(Clone, Debug)]
pub enum MenuRef {
    Menu(ChunkIndex),
    Item(ChunkIndex, ChunkIndex),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Lit(String),
    /// A bare word: a variable, or its own name if never assigned.
    Var(String),
    Chunk(Box<Chunk>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `foo(a, b)`: user function first, then built-in.
    Call(String, Vec<Expr>),
    /// `the date`, `the abs of x`, `the long time`: built-in only.
    Builtin(String, NameLength, Vec<Expr>),
    Number(Countable),
    Property { name: String, length: NameLength, obj: Option<Box<ObjSpec>> },
    Object(ObjSpec),
    ThereIs(bool, ObjSpec),
    IsType(bool, Box<Expr>, String),
    Selection,
    Menu(Box<MenuRef>),
    /// `there is a menu "Game"`.
    MenuExists(Box<MenuRef>),
}

/// What `select` selects.
#[derive(Clone, Debug)]
pub enum Select {
    /// `select empty`: no selection.
    Empty,
    /// `select button 1`: the part, with the Button or Field tool.
    Part(Expr),
    /// `select [before|after] [text of] <chunk of a field>`.
    Text(Prep, Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prep {
    Into,
    Before,
    After,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    Ticks,
    Seconds,
}

#[derive(Clone, Debug)]
pub enum RepeatKind {
    Forever,
    Count(Expr),
    While(Expr),
    Until(Expr),
    With { var: String, from: Expr, to: Expr, down: bool },
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub name: Expr,
    pub speed: Option<String>,
}

#[derive(Clone, Debug)]
pub enum GoDest {
    Card(CardSpec),
    Expr(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindMode {
    Normal,
    Chars,
    Word,
    Whole,
    String,
}

#[derive(Clone, Debug)]
pub enum ShowHide {
    Object(Expr),
    MsgBox,
    MenuBar,
    TitleBar,
    CardPicture,
    BgPicture,
    PictureOf(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortStyle {
    Text,
    Numeric,
    DateTime,
}

#[derive(Clone, Debug)]
pub enum MarkWhat {
    All,
    Card(Expr),
    Where(Expr),
    Finding(FindMode, Expr, Option<Expr>),
}

#[derive(Clone, Debug)]
pub enum Cmd {
    /// `messages` is for `put items into menu "X" with menuMessages msgs`.
    Put { value: Expr, prep: Prep, dest: Option<Expr>, messages: Option<Expr> },
    Get(Expr),
    Set { prop: String, obj: Option<Expr>, value: Expr },
    CreateMenu(Expr),
    ResetMenuBar,
    Add { value: Expr, dest: Expr },
    Subtract { value: Expr, dest: Expr },
    Multiply { dest: Expr, value: Expr },
    Divide { dest: Expr, value: Expr },
    Answer { prompt: Expr, buttons: Vec<Expr> },
    AnswerFile(Expr),
    Ask { prompt: Expr, default: Option<Expr>, password: bool },
    AskFile { prompt: Expr, default: Option<Expr> },
    Beep(Option<Expr>),
    Go(GoDest),
    Wait(Expr, TimeUnit),
    WaitUntil(Expr),
    WaitWhile(Expr),
    Send { msg: Expr, target: Option<Expr> },
    Do(Expr),
    Hide(ShowHide),
    Show { what: ShowHide, at: Option<Expr> },
    ShowCards(Option<Expr>),
    LockScreen,
    UnlockScreen(Option<Effect>),
    LockMessages,
    UnlockMessages,
    VisualEffect(Effect),
    PushCard(Option<Expr>),
    PopCard(Option<(Prep, Expr)>),
    Find { mode: FindMode, what: Expr, field: Option<Expr> },
    SortChunks { kind: ChunkKind, container: Expr, ascending: bool, style: SortStyle, by: Option<Expr> },
    SortCards { ascending: bool, style: SortStyle, by: Expr, marked: bool, of: Option<Expr> },
    Delete(Expr),
    ChooseTool(Expr),
    DoMenu(Expr),
    Mark(MarkWhat),
    Unmark(MarkWhat),
    OpenFile(Expr),
    CloseFile(Expr),
    ReadFile { file: Expr, until: Option<Expr>, count: Option<Expr>, at: Option<Expr> },
    WriteFile { data: Expr, file: Expr, at: Option<Expr>, at_end: bool },
    Enable(Expr),
    Disable(Expr),
    EditScript(Expr),
    Select(Select),
    ClickAt(Expr),
    /// `drag from 10,10 to 100,100`: paints with the current paint tool.
    Drag { from: Expr, to: Expr },
    Type(Expr),
    Convert { value: Expr, to: String },
    /// `play "harpsichord" tempo 200 "c4 e g"`
    Play { sound: Expr, tempo: Option<Expr>, notes: Vec<Expr> },
    PlayStop,
    Speak(Expr),
    Dial(Expr),
    SaveStack(Option<Expr>),
    ArrowKey(Expr),
    NoOp,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    /// A built-in command. `name` is its message name (e.g. "go"), and
    /// `args` the rest of the statement as written, passed to handlers that
    /// intercept the command.
    Cmd { name: String, cmd: Cmd, args: String },
    If { cond: Expr, then: Vec<Stmt>, els: Option<Vec<Stmt>> },
    Repeat { kind: RepeatKind, body: Vec<Stmt> },
    Global(Vec<String>),
    Return(Option<Expr>),
    ExitHandler,
    ExitRepeat,
    ExitToHyperCard,
    NextRepeat,
    Pass,
    /// A user-defined command or system message, sent through the message path.
    Message { name: String, args: Vec<Expr> },
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub line: usize,
    pub kind: StmtKind,
}
