//! The HyperTalk interpreter. It runs on its own thread, owns no stack data
//! itself, and reads and writes the shared `World` in short critical sections.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, MutexGuard};
use std::time::{Duration, Instant};

use crate::hypertalk::ast::*;
use crate::hypertalk::chunk::{self, Resolved};
use crate::hypertalk::parser::{parse_expression, parse_object, parse_script, parse_statements};
use crate::hypertalk::value::*;
use crate::model::{Id, Layer, Stack};

use super::world::{Dialog, DialogReply, Event, KeyAction, ObjRef, Shared, UiRequest, World, lock};

const MAX_DEPTH: usize = 128;

/// System messages that are silently ignored when no script handles them.
const SYSTEM_MESSAGES: &[&str] = &[
    "mouseup", "mousedown", "mousestilldown", "mouseenter", "mouseleave", "mousewithin", "mousedoubleclick",
    "openstack", "closestack", "resumestack", "suspendstack", "startup", "quit", "opencard", "closecard",
    "openbackground", "closebackground", "newcard", "deletecard", "newbackground", "deletebackground",
    "newbutton", "deletebutton", "newfield", "deletefield", "newstack", "deletestack", "openfield",
    "closefield", "exitfield", "keydown", "returninfield", "enterinfield", "tabkey", "returnkey", "enterkey",
    "functionkey", "controlkey", "commandkeydown", "idle", "help", "choose", "resume", "suspend",
];

#[derive(Debug, Clone)]
pub struct ScriptError {
    pub msg: String,
    pub obj: Option<ObjRef>,
    pub line: Option<usize>,
}

/// Non-local control flow, including errors.
#[derive(Debug)]
pub enum Ctl {
    Exit,
    ExitRepeat,
    NextRepeat,
    Pass,
    Return(String),
    ExitToHyperCard,
    Error(ScriptError),
}

pub type R<T> = Result<T, Ctl>;

pub fn err<T>(msg: impl Into<String>) -> R<T> {
    Err(Ctl::Error(ScriptError { msg: msg.into(), obj: None, line: None }))
}

pub struct Frame {
    pub vars: HashMap<String, String>,
    pub globals: HashSet<String>,
    pub me: ObjRef,
    pub target: ObjRef,
    pub message: String,
    pub params: Vec<String>,
    /// Statements typed into the message box, which have no script to blame for errors.
    pub msgbox: bool,
}

impl Frame {
    pub fn new(me: ObjRef, target: ObjRef, message: &str, params: Vec<String>) -> Frame {
        Frame {
            vars: HashMap::new(),
            globals: HashSet::new(),
            me,
            target,
            message: message.to_string(),
            params,
            msgbox: false,
        }
    }
}

pub struct OpenFile {
    pub content: String,
    pub pos: usize,
    pub dirty: bool,
}

/// Where a script is stored, for caching parsed scripts.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ScriptKey {
    Stack,
    Background(Id),
    Card(Id),
    Part(Layer, Id),
}

pub struct Interp {
    pub world: Shared,
    pub abort: Arc<AtomicBool>,
    dialog_rx: Option<Receiver<DialogReply>>,
    pub globals: HashMap<String, String>,
    scripts: HashMap<ScriptKey, (u64, Result<Rc<Script>, String>)>,
    pub frames: Vec<Frame>,
    pub result: String,
    pub item_delimiter: String,
    pub number_format: String,
    pub lock_messages: bool,
    pub pending_effect: Option<(String, Option<String>)>,
    pub files: HashMap<String, OpenFile>,
    pub started: Instant,
    pub clipboard_card: Option<crate::model::Card>,
    pub pushed_cards: Vec<Id>,
    msgbox_vars: HashMap<String, String>,
    /// For tests: canned dialog answers.
    pub canned_answers: Vec<DialogReply>,
    pub audio: crate::sound::Player,
    /// Names of all message handlers in the stack's scripts, gathered when a
    /// command first needs them during an event.
    handler_names: Option<HashSet<String>>,
}

impl Interp {
    pub fn new(world: Shared, dialog_rx: Option<Receiver<DialogReply>>, abort: Arc<AtomicBool>) -> Interp {
        Interp {
            world,
            abort,
            dialog_rx,
            globals: HashMap::new(),
            scripts: HashMap::new(),
            frames: Vec::new(),
            result: String::new(),
            item_delimiter: ",".into(),
            number_format: "0.######".into(),
            lock_messages: false,
            pending_effect: None,
            files: HashMap::new(),
            started: Instant::now(),
            clipboard_card: None,
            pushed_cards: Vec::new(),
            msgbox_vars: HashMap::new(),
            canned_answers: Vec::new(),
            audio: crate::sound::Player::silent(),
            handler_names: None,
        }
    }

    pub fn w(&self) -> MutexGuard<'_, World> {
        lock(&self.world)
    }

    // ----- event loop -----

    pub fn run(mut self, rx: Receiver<Event>) {
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Event::Shutdown) => break,
                Ok(ev) => self.handle_event(ev),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => self.idle(),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        self.close_all_files();
    }

    fn idle(&mut self) {
        let (card, ok) = {
            let w = self.w();
            (w.card_id, w.tool == super::world::Tool::Browse && w.dialog.is_none())
        };
        if !ok || !self.path_handles(ObjRef::Card(card), "idle") {
            return;
        }
        self.handle_event(Event::Send { target: ObjRef::Card(card), message: "idle".into(), args: vec![] });
    }

    /// Cheap check whether any script on the path mentions a handler, to avoid
    /// waking up for `idle` when nothing listens.
    fn path_handles(&mut self, target: ObjRef, name: &str) -> bool {
        for obj in self.message_path(target) {
            if let Ok(Some(s)) = self.script_for(obj)
                && s.handlers.contains_key(name)
            {
                return true;
            }
        }
        false
    }

    pub fn handle_event(&mut self, ev: Event) {
        self.abort.store(false, Ordering::SeqCst);
        self.handler_names = None;
        {
            let mut w = self.w();
            w.busy = true;
            // The click that started this handler doesn't count for `the mouseClick`.
            w.mouse.clicked = false;
        }
        let outcome = match ev {
            Event::Startup => self.open_stack_messages(),
            Event::OpenFile(path) => match self.load_stack_file(&path) {
                Ok(Some(stack)) => self.switch_stack(stack),
                Ok(None) => Ok(()),
                Err(e) => Err(e),
            },
            Event::Send { target, message, args } => self.dispatch_top(target, &message, args),
            Event::MsgBox(text) => self.run_msgbox(&text),
            Event::DoMenu(item) => {
                let card = ObjRef::Card(self.w().card_id);
                self.dispatch_top(card, "doMenu", vec![item])
            }
            Event::Key { target, message, args, default } => self.key_message(target, &message, args, default),
            Event::Shutdown => Ok(()),
        };
        self.frames.clear();
        if let Err(Ctl::Error(e)) = outcome {
            self.w().requests.push(UiRequest::ScriptError { msg: e.msg, obj: e.obj, line: e.line });
        }
        // HyperCard resets these when all handlers finish.
        self.item_delimiter = ",".into();
        self.number_format = "0.######".into();
        self.lock_messages = false;
        let mut w = self.w();
        w.lock_screen = false;
        w.busy = false;
    }

    fn dispatch_top(&mut self, target: ObjRef, message: &str, args: Vec<String>) -> R<()> {
        let card = self.current_card_ref();
        self.frames.push(Frame::new(card, target, message, args.clone()));
        let handled = self.send_message(target, message, &args);
        self.frames.pop();
        match handled {
            Ok(true) => Ok(()),
            Ok(false) => self.default_handler(target, message, &args),
            Err(Ctl::ExitToHyperCard) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// What HyperCard itself does with a message nobody handled.
    fn default_handler(&mut self, _target: ObjRef, message: &str, args: &[String]) -> R<()> {
        let card = self.current_card_ref();
        self.frames.push(Frame::new(card, card, message, args.to_vec()));
        let r = match message.to_ascii_lowercase().as_str() {
            "domenu" => self.perform_menu(args.first().map(String::as_str).unwrap_or("")),
            "arrowkey" => self.arrow_key(args.first().map(String::as_str).unwrap_or("")),
            _ => Ok(()),
        };
        self.frames.pop();
        match r {
            Err(Ctl::ExitToHyperCard) => Ok(()),
            other => other,
        }
    }

    /// Send a keyboard message; if nobody takes it, the key does its default.
    fn key_message(&mut self, target: ObjRef, message: &str, args: Vec<String>, default: KeyAction) -> R<()> {
        let card = self.current_card_ref();
        self.frames.push(Frame::new(card, target, message, args.clone()));
        let handled = self.send_message(target, message, &args);
        self.frames.pop();
        match handled {
            Ok(true) | Err(Ctl::ExitToHyperCard) => Ok(()),
            Ok(false) => match default {
                KeyAction::Menu(item) => {
                    self.frames.push(Frame::new(card, card, "commandKeyDown", args));
                    let r = self.perform_menu(&item);
                    self.frames.pop();
                    r
                }
                KeyAction::Nothing => Ok(()),
                action => {
                    self.w().requests.push(UiRequest::Key(target, action));
                    Ok(())
                }
            },
            Err(e) => Err(e),
        }
    }

    fn open_stack_messages(&mut self) -> R<()> {
        let card = self.current_card_ref();
        for m in ["openStack", "openBackground", "openCard"] {
            self.dispatch_top(card, m, vec![])?;
        }
        Ok(())
    }

    fn switch_stack(&mut self, stack: Stack) -> R<()> {
        let card = self.current_card_ref();
        for m in ["closeCard", "closeBackground", "closeStack"] {
            // Errors in the old stack shouldn't prevent opening the new one.
            let _ = self.dispatch_top(card, m, vec![]);
        }
        self.scripts.clear();
        self.w().replace_stack(stack);
        self.open_stack_messages()
    }

    fn run_msgbox(&mut self, text: &str) -> R<()> {
        let stmts = parse_statements(text).or_else(err)?;
        let card = self.current_card_ref();
        let mut frame = Frame::new(card, card, "", vec![]);
        frame.vars = std::mem::take(&mut self.msgbox_vars);
        frame.msgbox = true;
        self.frames.push(frame);
        let mut r = Ok(());
        for s in &stmts {
            r = match &s.kind {
                StmtKind::Expr(e) => self.eval(e).map(|v| {
                    let mut w = self.w();
                    w.msg = v;
                    w.msg_visible = true;
                }),
                StmtKind::Message { name, args } => self.message_stmt(name, args, true),
                _ => self.exec(s),
            };
            if r.is_err() {
                break;
            }
        }
        if let Some(f) = self.frames.pop() {
            self.msgbox_vars = f.vars;
        }
        match r {
            Err(Ctl::Return(_) | Ctl::Exit | Ctl::ExitToHyperCard | Ctl::Pass) => Ok(()),
            other => other.map(|_| ()),
        }
    }

    // ----- scripts and messages -----

    fn script_key(obj: ObjRef) -> Option<ScriptKey> {
        Some(match obj {
            ObjRef::Stack => ScriptKey::Stack,
            ObjRef::Background(id) => ScriptKey::Background(id),
            ObjRef::Card(id) => ScriptKey::Card(id),
            ObjRef::Part { layer, id, .. } => ScriptKey::Part(layer, id),
            _ => return None,
        })
    }

    pub fn script_text(&self, obj: ObjRef) -> Option<String> {
        let w = self.w();
        let s = &w.stack;
        Some(match obj {
            ObjRef::Stack => s.script.clone(),
            ObjRef::Background(id) => s.background(id)?.script.clone(),
            ObjRef::Card(id) => s.card(id)?.script.clone(),
            ObjRef::Part { layer, id, card } => s.part(card, layer, id)?.script.clone(),
            _ => return None,
        })
    }

    fn script_for(&mut self, obj: ObjRef) -> R<Option<Rc<Script>>> {
        let Some(key) = Self::script_key(obj) else { return Ok(None) };
        let Some(text) = self.script_text(obj) else { return Ok(None) };
        if text.trim().is_empty() {
            return Ok(None);
        }
        let hash = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut h);
            h.finish()
        };
        let parsed = match self.scripts.get(&key) {
            Some((h, parsed)) if *h == hash => parsed.clone(),
            _ => {
                let parsed = parse_script(&text).map(Rc::new);
                self.scripts.insert(key, (hash, parsed.clone()));
                parsed
            }
        };
        match parsed {
            Ok(s) => Ok(Some(s)),
            Err(e) => {
                let line = e.strip_prefix("line ").and_then(|r| r.split(':').next()).and_then(|n| n.parse().ok());
                let msg = format!("Script error in {}: {}", self.object_name(obj), e);
                Err(Ctl::Error(ScriptError { msg, obj: Some(obj), line }))
            }
        }
    }

    pub fn current_card_ref(&self) -> ObjRef {
        ObjRef::Card(self.w().card_id)
    }

    pub fn message_path(&self, target: ObjRef) -> Vec<ObjRef> {
        let w = self.w();
        let current = w.card_id;
        let bg_of = |card: Id| w.stack.card(card).map(|c| c.background);
        let mut path = Vec::new();
        match target {
            ObjRef::Part { card, .. } => {
                path.push(target);
                path.push(ObjRef::Card(card));
                if let Some(bg) = bg_of(card) {
                    path.push(ObjRef::Background(bg));
                }
            }
            ObjRef::Card(id) => {
                path.push(target);
                if let Some(bg) = bg_of(id) {
                    path.push(ObjRef::Background(bg));
                }
            }
            ObjRef::Background(_) => path.push(target),
            ObjRef::MsgBox => {
                path.push(ObjRef::Card(current));
                if let Some(bg) = bg_of(current) {
                    path.push(ObjRef::Background(bg));
                }
            }
            ObjRef::Stack => {}
            ObjRef::HyperCard => return path,
        }
        path.push(ObjRef::Stack);
        path
    }

    /// Send a message along the path starting at `target`. Returns whether a
    /// handler took it (without passing it on).
    pub fn send_message(&mut self, target: ObjRef, name: &str, args: &[String]) -> R<bool> {
        let lname = name.to_ascii_lowercase();
        for obj in self.message_path(target) {
            let Some(script) = self.script_for(obj)? else { continue };
            let Some(handler) = script.handlers.get(&lname) else { continue };
            match self.run_handler(handler, obj, target, name, args) {
                Ok(v) => {
                    self.result = v.unwrap_or_default();
                    return Ok(true);
                }
                Err(Ctl::Pass) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(false)
    }

    fn run_handler(
        &mut self,
        h: &Handler,
        me: ObjRef,
        target: ObjRef,
        name: &str,
        args: &[String],
    ) -> R<Option<String>> {
        if self.frames.len() > MAX_DEPTH {
            return err("Too much recursion");
        }
        let mut frame = Frame::new(me, target, name, args.to_vec());
        for (i, p) in h.params.iter().enumerate() {
            frame.vars.insert(p.clone(), args.get(i).cloned().unwrap_or_default());
        }
        self.frames.push(frame);
        let r = self.exec_block(&h.body);
        self.frames.pop();
        match r {
            Ok(()) | Err(Ctl::Exit) | Err(Ctl::ExitRepeat) | Err(Ctl::NextRepeat) => Ok(None),
            Err(Ctl::Return(v)) => Ok(Some(v)),
            Err(Ctl::Error(mut e)) => {
                if e.obj.is_none() {
                    e.obj = Some(me);
                }
                Err(Ctl::Error(e))
            }
            Err(e) => Err(e),
        }
    }

    pub fn call_function(&mut self, name: &str, args: Vec<String>) -> R<String> {
        let lname = name.to_ascii_lowercase();
        let me = self.frame().me;
        for obj in self.message_path(me) {
            let Some(script) = self.script_for(obj)? else { continue };
            let Some(f) = script.functions.get(&lname) else { continue };
            let target = self.frame().target;
            match self.run_handler(f, obj, target, name, &args) {
                Ok(v) => return Ok(v.unwrap_or_default()),
                Err(Ctl::Pass) => continue,
                Err(e) => return Err(e),
            }
        }
        self.builtin(&lname, NameLength::Default, args)
    }

    pub fn frame(&self) -> &Frame {
        self.frames.last().expect("no frame")
    }

    pub fn frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("no frame")
    }

    // ----- variables -----

    pub fn get_var(&self, name: &str) -> Option<String> {
        let name = name.to_lowercase();
        let f = self.frame();
        if f.globals.contains(&name) {
            return Some(self.globals.get(&name).cloned().unwrap_or_default());
        }
        f.vars.get(&name).cloned()
    }

    pub fn set_var(&mut self, name: &str, value: String) {
        let name = name.to_lowercase();
        if self.frame().globals.contains(&name) {
            self.globals.insert(name, value);
        } else {
            self.frame_mut().vars.insert(name, value);
        }
    }

    // ----- statements -----

    pub fn exec_block(&mut self, stmts: &[Stmt]) -> R<()> {
        for s in stmts {
            self.exec(s)?;
        }
        Ok(())
    }

    fn check_abort(&self) -> R<()> {
        if self.abort.load(Ordering::SeqCst) { Err(Ctl::ExitToHyperCard) } else { Ok(()) }
    }

    pub fn exec(&mut self, s: &Stmt) -> R<()> {
        self.check_abort()?;
        let r = self.exec_kind(&s.kind);
        match r {
            Err(Ctl::Error(mut e)) => {
                if e.line.is_none() && e.obj.is_none() && self.frames.last().is_some_and(|f| !f.msgbox) {
                    e.line = Some(s.line);
                    e.obj = self.frames.last().map(|f| f.me);
                }
                Err(Ctl::Error(e))
            }
            other => other,
        }
    }

    fn exec_kind(&mut self, k: &StmtKind) -> R<()> {
        match k {
            StmtKind::Cmd { name, cmd, args } => {
                if self.intercepted(name, cmd, args)? {
                    return Ok(());
                }
                self.command(cmd)
            }
            StmtKind::If { cond, then, els } => {
                let v = self.eval(cond)?;
                let b = parse_bool(&v).ok_or_else(|| {
                    Ctl::Error(ScriptError { msg: format!("Expected true or false here, not \"{v}\""), obj: None, line: None })
                })?;
                if b {
                    self.exec_block(then)
                } else if let Some(els) = els {
                    self.exec_block(els)
                } else {
                    Ok(())
                }
            }
            StmtKind::Repeat { kind, body } => self.repeat(kind, body),
            StmtKind::Global(names) => {
                for n in names {
                    self.frame_mut().globals.insert(n.clone());
                    self.frame_mut().vars.remove(n);
                }
                Ok(())
            }
            StmtKind::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e)?,
                    None => String::new(),
                };
                Err(Ctl::Return(v))
            }
            StmtKind::ExitHandler => Err(Ctl::Exit),
            StmtKind::ExitRepeat => Err(Ctl::ExitRepeat),
            StmtKind::ExitToHyperCard => Err(Ctl::ExitToHyperCard),
            StmtKind::NextRepeat => Err(Ctl::NextRepeat),
            StmtKind::Pass => Err(Ctl::Pass),
            StmtKind::Message { name, args } => self.message_stmt(name, args, false),
            StmtKind::Expr(e) => {
                // A function call used as a statement, or a bare expression.
                self.eval(e)?;
                Ok(())
            }
        }
    }

    /// Built-in commands are messages too: a handler with the command's name
    /// can take it (and the command doesn't run) or pass it on. Returns
    /// whether a handler took it.
    fn intercepted(&mut self, name: &str, cmd: &Cmd, args: &str) -> R<bool> {
        // doMenu and arrowKey send their own messages.
        if matches!(name, "domenu" | "arrowkey") {
            return Ok(false);
        }
        // A command used inside its own handler goes straight to RandomCard.
        if self.frame().message.eq_ignore_ascii_case(name) {
            return Ok(false);
        }
        if !self.handler_names().contains(name) {
            return Ok(false);
        }
        let params = match cmd {
            Cmd::ChooseTool(e) => {
                let v = self.eval(e)?;
                match super::world::Tool::parse(&v) {
                    Some(t) => vec![t.name().to_string(), t.number().to_string()],
                    None => vec![v],
                }
            }
            _ if args.is_empty() => Vec::new(),
            _ => vec![args.to_string()],
        };
        let me = self.frame().me;
        self.send_message(me, name, &params)
    }

    /// Every handler name defined anywhere in the stack (lowercase).
    fn handler_names(&mut self) -> &HashSet<String> {
        if self.handler_names.is_none() {
            let mut names = HashSet::new();
            {
                let w = self.w();
                let s = &w.stack;
                let scripts = std::iter::once(&s.script)
                    .chain(s.backgrounds.iter().flat_map(|b| std::iter::once(&b.script).chain(b.parts.iter().map(|p| &p.script))))
                    .chain(s.cards.iter().flat_map(|c| std::iter::once(&c.script).chain(c.parts.iter().map(|p| &p.script))));
                for script in scripts {
                    for line in script.lines() {
                        let mut words = line.split_whitespace();
                        if words.next().is_some_and(|w| w.eq_ignore_ascii_case("on"))
                            && let Some(n) = words.next()
                        {
                            names.insert(n.to_ascii_lowercase());
                        }
                    }
                }
            }
            self.handler_names = Some(names);
        }
        self.handler_names.as_ref().unwrap()
    }

    /// Scripts changed: handler names must be gathered again.
    pub fn scripts_changed(&mut self) {
        self.handler_names = None;
    }

    fn message_stmt(&mut self, name: &str, args: &[Expr], from_msgbox: bool) -> R<()> {
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            vals.push(self.eval(a)?);
        }
        let target = if from_msgbox { self.current_card_ref() } else { self.frame().me };
        if self.send_message(target, name, &vals)? {
            return Ok(());
        }
        let lname = name.to_ascii_lowercase();
        if lname == "domenu" || lname == "arrowkey" {
            return self.default_handler(target, name, &vals);
        }
        if SYSTEM_MESSAGES.contains(&lname.as_str()) {
            return Ok(());
        }
        if from_msgbox && args.is_empty() {
            // Typing a variable or literal into the message box shows its value.
            if let Some(v) = self.get_var(&lname) {
                let mut w = self.w();
                w.msg = v;
                return Ok(());
            }
        }
        err(format!("Can't understand \"{name}\""))
    }

    fn repeat(&mut self, kind: &RepeatKind, body: &[Stmt]) -> R<()> {
        let run_body = |me: &mut Self| -> R<bool> {
            match me.exec_block(body) {
                Ok(()) | Err(Ctl::NextRepeat) => Ok(true),
                Err(Ctl::ExitRepeat) => Ok(false),
                Err(e) => Err(e),
            }
        };
        match kind {
            RepeatKind::Forever => while run_body(self)? {},
            RepeatKind::Count(e) => {
                let n = self.eval_number(e)?;
                let mut i = 0.0;
                while i < n {
                    if !run_body(self)? {
                        break;
                    }
                    i += 1.0;
                }
            }
            RepeatKind::While(c) => {
                while self.eval_bool(c)? {
                    if !run_body(self)? {
                        break;
                    }
                }
            }
            RepeatKind::Until(c) => {
                while !self.eval_bool(c)? {
                    if !run_body(self)? {
                        break;
                    }
                }
            }
            RepeatKind::With { var, from, to, down } => {
                let mut i = self.eval_number(from)?;
                let end = self.eval_number(to)?;
                loop {
                    if (*down && i < end) || (!*down && i > end) {
                        break;
                    }
                    let s = self.fmt(i);
                    self.set_var(var, s);
                    if !run_body(self)? {
                        break;
                    }
                    // The body may change the loop variable.
                    let cur = self.get_var(var).and_then(|v| parse_number(&v)).unwrap_or(i);
                    i = if *down { cur - 1.0 } else { cur + 1.0 };
                }
            }
        }
        Ok(())
    }

    // ----- expressions -----

    pub fn fmt(&self, n: f64) -> String {
        format_number(n, &self.number_format)
    }

    pub fn eval_number(&mut self, e: &Expr) -> R<f64> {
        let v = self.eval(e)?;
        to_number(&v)
    }

    pub fn eval_bool(&mut self, e: &Expr) -> R<bool> {
        let v = self.eval(e)?;
        to_bool(&v)
    }

    pub fn eval(&mut self, e: &Expr) -> R<String> {
        match e {
            Expr::Lit(s) => Ok(s.clone()),
            Expr::Var(name) => Ok(self.get_var(name).unwrap_or_else(|| {
                if name.eq_ignore_ascii_case("it") { String::new() } else { name.clone() }
            })),
            Expr::Chunk(c, target) => {
                let base = self.eval(target)?;
                let r = self.resolve_chunk(c, &base)?;
                Ok(chunk::get(&base, &[r], &self.item_delimiter))
            }
            Expr::Unary(UnOp::Neg, a) => {
                let n = self.eval_number(a)?;
                Ok(self.fmt(-n))
            }
            Expr::Unary(UnOp::Not, a) => Ok(bool_str(!self.eval_bool(a)?)),
            Expr::Binary(op, a, b) => self.binary(*op, a, b),
            Expr::Call(name, _) if name == "__menu" => Ok(String::new()),
            Expr::Call(name, args) if name == "__propof" => {
                let prop = self.eval(&args[0])?;
                let length = match self.eval(&args[1])?.as_str() {
                    "short" => NameLength::Short,
                    "long" => NameLength::Long,
                    "abbrev" => NameLength::Abbrev,
                    _ => NameLength::Default,
                };
                if matches!(&args[2], Expr::Call(n, _) if n == "__menu") {
                    return Ok(String::new());
                }
                if matches!(args[2], Expr::Chunk(..)) {
                    // Text styles of chunks: text is unstyled, so report plain.
                    return Ok(match prop.as_str() {
                        "textstyle" => "plain".into(),
                        _ => String::new(),
                    });
                }
                let obj = self.resolve_dynamic(&args[2])?;
                self.get_property(obj, &prop, length)
            }
            Expr::Call(name, args) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                self.call_function(name, vals)
            }
            Expr::Builtin(name, len, args) => {
                if name == "value" {
                    let src = self.eval(&args[0])?;
                    return self.value_of(&src);
                }
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                self.builtin(name, *len, vals)
            }
            Expr::Number(c) => self.count(c),
            Expr::Property { name, length, obj } => match obj {
                Some(spec) => {
                    let o = self.resolve(spec)?;
                    self.get_property(o, name, *length)
                }
                None => self.get_global_property(name),
            },
            Expr::Object(spec) => {
                let o = self.resolve(spec)?;
                self.object_value(o)
            }
            Expr::ThereIs(positive, spec) => {
                let exists = match self.resolve(spec) {
                    Ok(_) => true,
                    Err(Ctl::Error(_)) => false,
                    Err(e) => return Err(e),
                };
                Ok(bool_str(exists == *positive))
            }
            Expr::IsType(positive, e, t) => {
                let v = self.eval(e)?;
                let is = match t.as_str() {
                    "number" => parse_number(&v).is_some(),
                    "integer" => parse_int(&v).is_some(),
                    "point" => parse_point(&v).is_some(),
                    "rect" | "rectangle" => parse_rect(&v).is_some(),
                    "logical" | "boolean" | "bool" => parse_bool(&v).is_some(),
                    "date" => super::dates::parse_date(&v).is_some(),
                    "empty" => v.is_empty(),
                    _ => false,
                };
                Ok(bool_str(is == *positive))
            }
            Expr::Selection => Ok(String::new()),
        }
    }

    pub fn value_of(&mut self, src: &str) -> R<String> {
        if src.trim().is_empty() {
            return Ok(String::new());
        }
        match parse_expression(src) {
            Ok(e) => self.eval(&e),
            Err(_) => Ok(src.to_string()),
        }
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> R<String> {
        match op {
            BinOp::And => {
                if !self.eval_bool(a)? {
                    return Ok(bool_str(false));
                }
                return Ok(bool_str(self.eval_bool(b)?));
            }
            BinOp::Or => {
                if self.eval_bool(a)? {
                    return Ok(bool_str(true));
                }
                return Ok(bool_str(self.eval_bool(b)?));
            }
            _ => {}
        }
        let x = self.eval(a)?;
        let y = self.eval(b)?;
        let num = |s: &str| to_number(s);
        Ok(match op {
            BinOp::Concat => x + &y,
            BinOp::ConcatSpace => x + " " + &y,
            BinOp::Add => self.fmt(num(&x)? + num(&y)?),
            BinOp::Sub => self.fmt(num(&x)? - num(&y)?),
            BinOp::Mul => self.fmt(num(&x)? * num(&y)?),
            BinOp::Div => {
                let d = num(&y)?;
                if d == 0.0 {
                    return err("Can't divide by zero");
                }
                self.fmt(num(&x)? / d)
            }
            BinOp::IntDiv => {
                let d = num(&y)?;
                if d == 0.0 {
                    return err("Can't divide by zero");
                }
                self.fmt((num(&x)? / d).trunc())
            }
            BinOp::Mod => {
                let d = num(&y)?;
                if d == 0.0 {
                    return err("Can't divide by zero");
                }
                self.fmt(num(&x)? % d)
            }
            BinOp::Pow => self.fmt(num(&x)?.powf(num(&y)?)),
            BinOp::Eq => bool_str(equals(&x, &y)),
            BinOp::Ne => bool_str(!equals(&x, &y)),
            BinOp::Lt => bool_str(compare(&x, &y).is_lt()),
            BinOp::Gt => bool_str(compare(&x, &y).is_gt()),
            BinOp::Le => bool_str(compare(&x, &y).is_le()),
            BinOp::Ge => bool_str(compare(&x, &y).is_ge()),
            BinOp::Contains => bool_str(x.to_lowercase().contains(&y.to_lowercase())),
            BinOp::IsIn => bool_str(y.to_lowercase().contains(&x.to_lowercase())),
            BinOp::IsNotIn => bool_str(!y.to_lowercase().contains(&x.to_lowercase())),
            BinOp::IsWithin | BinOp::IsNotWithin => {
                let Some((px, py)) = parse_point(&x) else {
                    return err(format!("Expected a point here, not \"{x}\""));
                };
                let Some((l, t, r, bt)) = parse_rect(&y) else {
                    return err(format!("Expected a rectangle here, not \"{y}\""));
                };
                let inside = px >= l && px < r && py >= t && py < bt;
                bool_str(inside == (op == BinOp::IsWithin))
            }
            BinOp::And | BinOp::Or => unreachable!(),
        })
    }

    fn ordinal_index(&self, o: Ordinal, count: usize) -> i64 {
        match o {
            Ordinal::Nth(n) => n,
            Ordinal::Last => count as i64,
            Ordinal::Middle => (count as i64 / 2) + 1,
            Ordinal::Any => {
                if count == 0 {
                    1
                } else {
                    rand::random_range(1..=count as i64)
                }
            }
        }
    }

    /// Resolve chunk indexes against the text they apply to.
    pub fn resolve_chunk(&mut self, c: &Chunk, text: &str) -> R<Resolved> {
        let count = count_chunks(text, c.kind, &self.item_delimiter);
        let start = match &c.start {
            ChunkIndex::Ordinal(o) => self.ordinal_index(*o, count),
            ChunkIndex::Expr(e) => to_int(&self.eval(e)?)?,
        };
        let end = match &c.end {
            Some(e) => to_int(&self.eval(e)?)?,
            None => start,
        };
        Ok(Resolved { kind: c.kind, start, end })
    }

    // ----- containers -----

    /// Split `dest` into its chunks (outermost first) and the base container.
    fn container_parts(dest: &Expr) -> (Vec<&Chunk>, &Expr) {
        let mut chunks = Vec::new();
        let mut e = dest;
        while let Expr::Chunk(c, inner) = e {
            chunks.push(c.as_ref());
            e = inner;
        }
        (chunks, e)
    }

    fn read_container(&mut self, base: &Expr) -> R<String> {
        match base {
            Expr::Var(name) => Ok(self.get_var(name).unwrap_or_default()),
            Expr::Object(spec) => {
                let o = self.resolve(spec)?;
                self.object_value(o)
            }
            Expr::Selection => Ok(String::new()),
            _ => err("Expected a container (a variable, field or the message box) here"),
        }
    }

    fn write_container(&mut self, base: &Expr, value: String) -> R<()> {
        match base {
            Expr::Var(name) => {
                self.set_var(name, value);
                Ok(())
            }
            Expr::Object(spec) => {
                let o = self.resolve(spec)?;
                self.set_object_value(o, value)
            }
            Expr::Selection => Ok(()),
            _ => err("Expected a container (a variable, field or the message box) here"),
        }
    }

    pub fn put_into(&mut self, dest: &Expr, prep: Prep, value: &str) -> R<()> {
        if matches!(dest, Expr::Call(n, _) if n == "__menu") {
            // Menus aren't scriptable; putting into one does nothing.
            return Ok(());
        }
        let (chunks, base) = Self::container_parts(dest);
        if chunks.is_empty() && prep == Prep::Into {
            return self.write_container(base, value.to_string());
        }
        let mut text = self.read_container(base)?;
        let resolved = self.resolve_chunks(&chunks, &text)?;
        if resolved.is_empty() {
            match prep {
                Prep::Into => text = value.to_string(),
                Prep::Before => text.insert_str(0, value),
                Prep::After => text.push_str(value),
            }
        } else {
            chunk::put(&mut text, &resolved, &self.item_delimiter, prep, value);
        }
        self.write_container(base, text)
    }

    pub fn delete_chunk(&mut self, dest: &Expr) -> R<()> {
        let (chunks, base) = Self::container_parts(dest);
        let mut text = self.read_container(base)?;
        let resolved = self.resolve_chunks(&chunks, &text)?;
        chunk::delete(&mut text, &resolved, &self.item_delimiter);
        self.write_container(base, text)
    }

    fn resolve_chunks(&mut self, chunks: &[&Chunk], base: &str) -> R<Vec<Resolved>> {
        // Innermost (rightmost) first, so ordinals see the text they apply to.
        let mut inner_first: Vec<Resolved> = Vec::new();
        for c in chunks.iter().rev() {
            let outer_first: Vec<Resolved> = inner_first.iter().rev().copied().collect();
            let sub = chunk::get(base, &outer_first, &self.item_delimiter);
            let sub = if outer_first.is_empty() { base.to_string() } else { sub };
            inner_first.push(self.resolve_chunk(c, &sub)?);
        }
        inner_first.reverse();
        Ok(inner_first)
    }

    // ----- dialogs -----

    pub fn dialog(&mut self, d: Dialog) -> R<DialogReply> {
        if !self.canned_answers.is_empty() {
            return Ok(self.canned_answers.remove(0));
        }
        let Some(rx) = &self.dialog_rx else {
            return Ok(DialogReply::Text(None));
        };
        // Drain stale replies.
        while rx.try_recv().is_ok() {}
        self.w().dialog = Some(d);
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(r) => return Ok(r),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if self.abort.load(Ordering::SeqCst) {
                        self.w().dialog = None;
                        return Err(Ctl::ExitToHyperCard);
                    }
                }
                Err(_) => return Err(Ctl::ExitToHyperCard),
            }
        }
    }

    pub fn sleep(&self, d: Duration) -> R<()> {
        let end = Instant::now() + d;
        while Instant::now() < end {
            self.check_abort()?;
            std::thread::sleep((end - Instant::now()).min(Duration::from_millis(15)));
        }
        Ok(())
    }

    pub fn close_all_files(&mut self) {
        let names: Vec<String> = self.files.keys().cloned().collect();
        for n in names {
            let _ = self.close_file(&n);
        }
    }

    pub fn close_file(&mut self, name: &str) -> R<()> {
        if let Some(f) = self.files.remove(name)
            && f.dirty
            && let Err(e) = std::fs::write(name, f.content)
        {
            return err(format!("Can't write file \"{name}\": {e}"));
        }
        Ok(())
    }

    pub fn parse_object_text(&mut self, text: &str) -> R<ObjRef> {
        match parse_object(text) {
            Some(spec) => self.resolve(&spec),
            None => err(format!("No such object \"{text}\"")),
        }
    }
}

pub fn to_number(s: &str) -> R<f64> {
    if s.trim().is_empty() {
        // HyperCard treats empty as zero in arithmetic.
        return Ok(0.0);
    }
    parse_number(s).ok_or_else(|| {
        Ctl::Error(ScriptError { msg: format!("Expected a number here, not \"{s}\""), obj: None, line: None })
    })
}

pub fn to_int(s: &str) -> R<i64> {
    let n = to_number(s)?;
    Ok(n.trunc() as i64)
}

pub fn to_bool(s: &str) -> R<bool> {
    parse_bool(s).ok_or_else(|| {
        Ctl::Error(ScriptError { msg: format!("Expected true or false here, not \"{s}\""), obj: None, line: None })
    })
}
