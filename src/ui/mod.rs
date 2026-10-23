//! The RandomCard window: menus, tool palette, the card, and the message box.

mod editing;
mod painting;
mod render;
mod windows;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Color32, Key, KeyboardShortcut, Modifiers, PointerButton, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    TextureHandle, Vec2,
};

use crate::model::{Layer, Part, PartKind, PartStyle, Rect as CardRect, Stack};
use crate::runtime::world::{Event, KeyAction, ObjRef, Tool, UiRequest, World, lock};
use crate::runtime::{self, Runtime};
use render::{Scene, Textures};
use windows::{ErrorAlert, InfoTarget, InfoWindow, ScriptEditor};

const STACK_EXT: &str = "rcstack";

struct Anim {
    old: Scene,
    effect: String,
    seconds: f32,
    start: Instant,
}

/// A text field being edited in the browse tool.
struct Editing {
    obj: ObjRef,
    buffer: String,
    /// The field text as last written to (or read from) the world.
    synced: String,
    changed: bool,
    focus_requested: bool,
    /// Select all the text once focused (when tabbing into a field).
    select_all: bool,
}

fn field_edit_id(obj: ObjRef) -> egui::Id {
    egui::Id::unique(("field-edit", format!("{obj:?}")))
}

#[derive(Clone, Copy)]
enum DragKind {
    Move,
    Resize,
}

struct PartDrag {
    obj: ObjRef,
    kind: DragKind,
    start: (i32, i32),
    rect: CardRect,
}

use editing::{Clip, Order};
use painting::{Marquee, PaintStroke, TextEntry};

pub struct App {
    rt: Runtime,
    tex: Textures,
    last_scene: Option<Scene>,
    frozen: Option<Scene>,
    anim: Option<Anim>,
    seen_transition: u64,
    hover: Option<ObjRef>,
    pressed: Option<ObjRef>,
    pressed_inside: bool,
    /// Each scrolling field's `scroll`, as last given to or read from its
    /// scroll area, to tell script changes from the user's scrolling.
    synced_scroll: std::collections::HashMap<ObjRef, i32>,
    /// Where scrolling fields' scroll bars were last frame; clicks there
    /// belong to the scroll bar, not the card.
    scroll_bar_zones: Vec<Rect>,
    last_repeat_msg: Instant,
    editing: Option<Editing>,
    selected: Option<ObjRef>,
    part_drag: Option<PartDrag>,
    stroke: Option<PaintStroke>,
    clipboard: Option<Clip>,
    marquee: Option<Marquee>,
    /// Polygon tool: the corners placed so far.
    polygon: Option<Vec<(i32, i32)>>,
    /// Text tool: text being typed, not yet painted.
    text_entry: Option<TextEntry>,
    /// Swatch textures for the stack's patterns.
    pattern_textures: Option<(Vec<[u8; 8]>, Vec<TextureHandle>)>,
    infos: Vec<InfoWindow>,
    editors: Vec<ScriptEditor>,
    error: Option<ErrorAlert>,
    ask_text: String,
    dialog_init: bool,
    msg_buffer: String,
    msg_has_focus: bool,
    /// Give the message box keyboard focus (after blind typing).
    focus_msg_box: bool,
    beeps_seen: u32,
    flash_until: Option<Instant>,
    last_autosave: Instant,
    quit_prompt: bool,
    quitting: bool,
    title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, stack: Stack) -> App {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        App {
            rt: runtime::start(stack),
            tex: Textures::default(),
            last_scene: None,
            frozen: None,
            anim: None,
            seen_transition: 0,
            hover: None,
            pressed: None,
            pressed_inside: false,
            synced_scroll: Default::default(),
            scroll_bar_zones: Vec::new(),
            last_repeat_msg: Instant::now(),
            editing: None,
            selected: None,
            part_drag: None,
            stroke: None,
            clipboard: None,
            marquee: None,
            polygon: None,
            text_entry: None,
            pattern_textures: None,
            infos: Vec::new(),
            editors: Vec::new(),
            error: None,
            ask_text: String::new(),
            dialog_init: false,
            msg_buffer: String::new(),
            msg_has_focus: false,
            focus_msg_box: false,
            beeps_seen: 0,
            flash_until: None,
            last_autosave: Instant::now(),
            quit_prompt: false,
            quitting: false,
            title: String::new(),
        }
    }

    fn world(&self) -> std::sync::MutexGuard<'_, World> {
        lock(&self.rt.world)
    }

    fn do_menu(&self, item: &str) {
        self.send(Event::DoMenu(item.to_string()));
    }

    // ----- menus and palette -----

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let (edit_bg, tool, filled, msg_visible) = {
            let w = self.world();
            (w.edit_bg, w.tool, w.fill_shapes, w.msg_visible)
        };
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                for item in ["New Stack…", "Open Stack…", "Import HyperCard Stack…", "Save Stack", "Save a Copy…"] {
                    if ui.button(item).clicked() {
                        self.do_menu(item);
                    }
                }
                ui.separator();
                if ui.button("Quit RandomCard").clicked() {
                    self.do_menu("Quit RandomCard");
                }
            });
            ui.menu_button("Edit", |ui| {
                let (can_undo, can_redo) = {
                    let w = self.world();
                    (!w.undo.is_empty(), !w.redo.is_empty())
                };
                if ui.add_enabled(can_undo, egui::Button::new("Undo")).clicked() {
                    self.undo();
                }
                if ui.add_enabled(can_redo, egui::Button::new("Redo")).clicked() {
                    self.redo();
                }
                ui.separator();
                // Cut, Copy and Clear name what they act on, as in HyperCard.
                let target = self.edit_target_name();
                for verb in ["Cut", "Copy", "Clear"] {
                    let label = match target {
                        Some(t) if !(verb == "Clear" && t == "Picture") => format!("{verb} {t}"),
                        _ => verb.to_string(),
                    };
                    if ui.add_enabled(target.is_some(), egui::Button::new(&label)).clicked() {
                        self.do_menu(&label);
                    }
                }
                let paste = self.paste_name();
                let label = paste.map(|t| format!("Paste {t}")).unwrap_or_else(|| "Paste".into());
                if ui.add_enabled(paste.is_some(), egui::Button::new(&label)).clicked() {
                    self.do_menu(&label);
                }
                ui.separator();
                for item in ["New Card", "Delete Card", "Cut Card", "Copy Card", "Paste Card"] {
                    if ui.button(item).clicked() {
                        self.do_menu(item);
                    }
                }
                ui.separator();
                if ui.button(if edit_bg { "✔ Background" } else { "Background" }).clicked() {
                    self.do_menu("Background");
                }
                if ui.button("Clear Picture").clicked() {
                    self.do_menu("Clear Picture");
                }
            });
            ui.menu_button("Go", |ui| {
                for item in ["Back", "First", "Prev", "Next", "Last"] {
                    if ui.button(item).clicked() {
                        self.do_menu(item);
                    }
                }
                ui.separator();
                if ui.button(if msg_visible { "✔ Message" } else { "Message" }).clicked() {
                    self.do_menu("Message");
                }
            });
            ui.menu_button("Tools", |ui| {
                for t in Tool::ALL {
                    let label = windows::capitalize(t.name());
                    if ui.selectable_label(tool == *t, label).clicked() {
                        self.choose_tool(*t);
                    }
                }
                ui.separator();
                let mut f = filled;
                if ui.checkbox(&mut f, "Draw Filled").changed() {
                    self.world().fill_shapes = f;
                }
            });
            ui.menu_button("Paint", |ui| {
                if ui.button("Select All").clicked() {
                    self.do_menu("Select All");
                }
                ui.separator();
                for op in crate::paint::PaintOp::ALL {
                    if ui.button(op.name()).clicked() {
                        self.do_menu(op.name());
                    }
                }
            });
            ui.menu_button("Objects", |ui| {
                for item in ["Button Info…", "Field Info…", "Card Info…", "Bkgnd Info…", "Stack Info…"] {
                    if ui.button(item).clicked() {
                        self.do_menu(item);
                    }
                }
                ui.separator();
                for item in ["Bring Closer", "Send Farther", "Bring to Front", "Send to Back"] {
                    if ui.button(item).clicked() {
                        self.do_menu(item);
                    }
                }
                ui.separator();
                for item in ["New Button", "New Field", "New Background"] {
                    if ui.button(item).clicked() {
                        self.do_menu(item);
                    }
                }
                ui.separator();
                for item in ["Card Script", "Bkgnd Script", "Stack Script"] {
                    if ui.button(item).clicked() {
                        self.do_menu(item);
                    }
                }
            });
        });
    }

    fn choose_tool(&mut self, t: Tool) {
        self.end_editing();
        self.commit_marquee();
        self.commit_text();
        self.polygon = None;
        self.selected = None;
        let mut w = self.world();
        w.tool = t;
        w.focus = None;
    }

    // ----- requests from scripts -----

    fn handle_requests(&mut self, ctx: &egui::Context) {
        let requests: Vec<UiRequest> = std::mem::take(&mut self.world().requests);
        for r in requests {
            match r {
                UiRequest::OpenStack => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("RandomCard stack", &[STACK_EXT])
                        .add_filter("All files", &["*"])
                        .set_title("Open Stack")
                        .pick_file()
                    {
                        self.open_stack_path(&path);
                    }
                }
                UiRequest::ImportStack => {
                    // HyperCard stacks usually have no extension, so offer every file.
                    if let Some(path) = rfd::FileDialog::new().set_title("Import HyperCard Stack").pick_file() {
                        self.open_stack_path(&path);
                    }
                }
                UiRequest::NewStack => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("RandomCard stack", &[STACK_EXT])
                        .set_title("New Stack")
                        .set_file_name("Untitled.rcstack")
                        .save_file()
                    {
                        let path = with_ext(path);
                        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        let mut stack = Stack::new(&name);
                        if let Err(e) = stack.save(&path) {
                            self.alert(e);
                            continue;
                        }
                        stack.path = Some(path);
                        self.autosave();
                        self.reset_view();
                        self.send(Event::OpenStack(Box::new(stack)));
                    }
                }
                UiRequest::SaveStackAs(name) => {
                    let mut dialog = rfd::FileDialog::new().add_filter("RandomCard stack", &[STACK_EXT]);
                    let (stack_name, imported_dir) = {
                        let w = self.world();
                        let dir = w.stack.imported_from.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));
                        (w.stack.name.clone(), dir)
                    };
                    if let Some(dir) = imported_dir {
                        dialog = dialog.set_directory(dir);
                    }
                    let default = name.unwrap_or_else(|| format!("{stack_name}.{STACK_EXT}"));
                    dialog = dialog.set_file_name(default).set_title("Save Stack As");
                    if let Some(path) = dialog.save_file() {
                        let path = with_ext(path);
                        let mut w = self.world();
                        match w.stack.save(&path) {
                            Ok(()) => {
                                w.stack.path = Some(path);
                                w.dirty = false;
                            }
                            Err(e) => {
                                drop(w);
                                self.alert(e);
                            }
                        }
                    }
                }
                UiRequest::Quit => self.request_quit(ctx),
                UiRequest::SelectedPartInfo => match self.selected {
                    Some(o) => self.open_info(InfoTarget::Part(o)),
                    None => self.alert("Select a button or field first, with the Button or Field tool.".into()),
                },
                UiRequest::CardInfo => {
                    let id = self.world().card_id;
                    self.open_info(InfoTarget::Card(id));
                }
                UiRequest::BgInfo => {
                    let id = self.world().bg_id();
                    self.open_info(InfoTarget::Background(id));
                }
                UiRequest::StackInfo => self.open_info(InfoTarget::Stack),
                UiRequest::EditScript(o) => self.open_script(o, None),
                UiRequest::ScriptError { msg, obj, line } => {
                    self.error = Some(ErrorAlert { msg, obj, line });
                }
                UiRequest::BringCloser => self.reorder_selected(Order::Closer),
                UiRequest::SendFarther => self.reorder_selected(Order::Farther),
                UiRequest::BringToFront => self.reorder_selected(Order::Front),
                UiRequest::SendToBack => self.reorder_selected(Order::Back),
                UiRequest::Edit(op) => self.edit_command(op),
                UiRequest::Redo => self.redo(),
                UiRequest::Undo => self.undo(),
                UiRequest::SelectPart(o) => self.selected = Some(o),
                UiRequest::Key(target, action) => self.key_action(ctx, target, action),
                UiRequest::PaintOp(op) => self.apply_paint_op(op),
                UiRequest::SelectAll => self.select_all(),
            }
        }
    }

    fn alert(&mut self, msg: String) {
        self.error = Some(ErrorAlert { msg, obj: None, line: None });
    }

    fn open_stack_path(&mut self, path: &std::path::Path) {
        match crate::import::load_any(path) {
            Ok(stack) => {
                self.autosave();
                self.reset_view();
                self.send(Event::OpenStack(Box::new(stack)));
            }
            Err(e) => self.alert(e),
        }
    }

    fn reset_view(&mut self) {
        self.end_editing();
        self.selected = None;
        self.marquee = None;
        self.polygon = None;
        self.text_entry = None;
        self.infos.clear();
        self.editors.clear();
    }

    fn request_quit(&mut self, ctx: &egui::Context) {
        let (dirty, has_path) = {
            let w = self.world();
            (w.dirty, w.stack.path.is_some())
        };
        if dirty && !has_path {
            self.quit_prompt = true;
            return;
        }
        self.autosave();
        self.quitting = true;
        self.send(Event::Shutdown);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// HyperCard saves changes automatically; so do we, once a stack has a file.
    fn autosave(&mut self) {
        let json = {
            let mut w = self.world();
            if !w.dirty || w.busy && self.last_autosave.elapsed() < Duration::from_secs(30) {
                return;
            }
            let Some(path) = w.stack.path.clone() else { return };
            w.dirty = false;
            serde_json::to_string_pretty(&w.stack).map(|j| (path, j))
        };
        self.last_autosave = Instant::now();
        if let Ok((path, json)) = json
            && let Err(e) = std::fs::write(&path, json)
        {
            self.world().dirty = true;
            self.alert(format!("Can't save {}: {e}", path.display()));
        }
    }

    // ----- keyboard -----

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |k: Key| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_alt = |k: Key| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, k);
        // Command-period always stops scripts; it can't be intercepted.
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::Period))) {
            self.rt.abort.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        // While a dialog is up, keys belong to it.
        if self.modal_open() {
            return;
        }
        // Command keys: offered to commandKeyDown handlers, else a menu command.
        let menu = |s: &str| KeyAction::Menu(s.to_string());
        let command_keys = [
            (cmd_alt(Key::C), "c", menu("Card Script")),
            (cmd_alt(Key::B), "b", menu("Bkgnd Script")),
            (cmd_alt(Key::S), "s", menu("Stack Script")),
            (cmd(Key::N), "n", menu("New Card")),
            (cmd(Key::Num1), "1", menu("First")),
            (cmd(Key::Num2), "2", menu("Prev")),
            (cmd(Key::Num3), "3", menu("Next")),
            (cmd(Key::Num4), "4", menu("Last")),
            (cmd(Key::M), "m", menu("Message")),
            (cmd(Key::B), "b", menu("Background")),
            (cmd(Key::O), "o", menu("Open Stack…")),
            (cmd(Key::Q), "q", menu("Quit RandomCard")),
            (cmd(Key::S), "s", menu("Save Stack")),
        ];
        for (shortcut, ch, action) in command_keys {
            if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
                self.command_key(ctx, ch, action);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::Tab))) {
            self.choose_tool(Tool::Browse);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::Space))) {
            let mut w = self.world();
            w.menubar_visible = !w.menubar_visible;
        }
        // Undo, redo and the clipboard, when not typing (text fields have their own).
        if !ctx.egui_wants_keyboard_input() {
            let redo = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
            let edit_keys = [
                (redo, "Z", KeyAction::Redo),
                (cmd(Key::Z), "z", KeyAction::Undo),
                (cmd(Key::X), "x", KeyAction::Menu("Cut".into())),
                (cmd(Key::C), "c", KeyAction::Menu("Copy".into())),
                (cmd(Key::V), "v", KeyAction::Menu("Paste".into())),
            ];
            for (shortcut, ch, action) in edit_keys {
                if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
                    self.command_key(ctx, ch, action);
                }
            }
            // The platform layer usually delivers Ctrl+X/C/V as clipboard events.
            let clipboard_ops: Vec<(&str, &str)> = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Cut => Some(("x", "Cut")),
                        egui::Event::Copy => Some(("c", "Copy")),
                        egui::Event::Paste(_) => Some(("v", "Paste")),
                        _ => None,
                    })
                    .collect()
            });
            for (ch, item) in clipboard_ops {
                self.command_key(ctx, ch, KeyAction::Menu(item.into()));
            }
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let tool = self.world().tool;
        let delete = ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));
        if delete {
            if let (Tool::Button | Tool::Field, Some(obj)) = (tool, self.selected) {
                self.delete_part(obj);
            } else if let Some(Marquee::Selected { region }) = self.marquee.take() {
                self.push_undo();
                self.with_picture(|p| region.lift(p));
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.commit_marquee();
            self.marquee = None;
            self.selected = None;
            self.polygon = None;
        }
        if tool == Tool::Browse {
            for (k, dir) in [(Key::ArrowLeft, "left"), (Key::ArrowRight, "right"), (Key::ArrowUp, "up"), (Key::ArrowDown, "down")] {
                if ctx.input(|i| i.key_pressed(k)) {
                    let card = ObjRef::Card(self.world().card_id);
                    self.send(Event::Send { target: card, message: "arrowKey".into(), args: vec![dir.into()] });
                }
            }
        }
    }

    /// Whether any script on `target`'s message path mentions a handler.
    /// Keys are only held back for scripts when this is true.
    fn handles(&self, target: ObjRef, message: &str) -> bool {
        let w = self.world();
        let s = &w.stack;
        let mut scripts: Vec<&str> = vec![&s.script];
        let card = match target {
            ObjRef::Part { layer, id, card } => {
                if let Some(p) = s.part(card, layer, id) {
                    scripts.push(&p.script);
                }
                card
            }
            ObjRef::Card(card) => card,
            _ => w.card_id,
        };
        if let Some(c) = s.card(card) {
            scripts.push(&c.script);
            if let Some(b) = s.background(c.background) {
                scripts.push(&b.script);
            }
        }
        scripts.iter().any(|t| t.to_ascii_lowercase().contains(message))
    }

    fn command_key(&mut self, ctx: &egui::Context, ch: &str, action: KeyAction) {
        let target = self.key_target();
        if self.world().tool == Tool::Browse && self.handles(target, "commandkeydown") {
            let message = "commandKeyDown".to_string();
            self.send(Event::Key { target, message, args: vec![ch.into()], default: action });
        } else {
            self.key_action(ctx, target, action);
        }
    }

    /// An answer/ask dialog, script error or quit prompt is showing.
    fn modal_open(&self) -> bool {
        self.world().dialog.is_some() || self.error.is_some() || self.quit_prompt
    }

    /// Keys go to the field being edited, or else the current card.
    fn key_target(&self) -> ObjRef {
        match &self.editing {
            Some(e) => e.obj,
            None => ObjRef::Card(self.world().card_id),
        }
    }

    /// HyperCard's keyboard messages. Keys are only held back from the text
    /// editor for scripts that handle the message; otherwise typing stays
    /// immediate.
    fn keyboard(&mut self, ctx: &egui::Context) {
        if self.world().tool != Tool::Browse || self.modal_open() {
            return;
        }
        let target = self.key_target();
        let editing = self.editing.is_some();
        if !editing && ctx.egui_wants_keyboard_input() {
            // Typing in the message box or a dialog.
            return;
        }
        let names = if editing {
            ["keydown", "returninfield", "enterinfield", "tabkey"]
        } else {
            ["keydown", "returnkey", "enterkey", "tabkey"]
        };
        let [key_down, ret, enter, tab] = names.map(|n| self.handles(target, n));
        let function = !editing && self.handles(target, "functionkey");
        let auto_tab = editing && self.auto_tab_at_end(ctx);

        // (message, params, default) to send to scripts, or defaults to do now.
        let mut sends: Vec<(&str, Vec<String>, KeyAction)> = Vec::new();
        let mut local: Vec<KeyAction> = Vec::new();
        let mut route = |handled: bool, message: &'static str, params: Vec<String>, default: KeyAction| {
            if handled {
                sends.push((message, params, default));
            } else {
                local.push(default);
            }
        };
        ctx.input_mut(|i| {
            i.events.retain(|ev| match ev {
                egui::Event::Text(t) if !editing || key_down => {
                    for c in t.chars() {
                        let default = if editing { KeyAction::Insert(c.into()) } else { KeyAction::BlindType(c.into()) };
                        route(key_down, "keyDown", vec![c.into()], default);
                    }
                    false
                }
                egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.command => {
                    // Shift+Return stands in for the Enter key.
                    match (editing, modifiers.shift) {
                        (true, true) => route(enter, "enterInField", vec![], KeyAction::CloseField),
                        (true, false) if ret || auto_tab => {
                            let default = if auto_tab { KeyAction::NextField } else { KeyAction::Insert("\n".into()) };
                            route(ret, "returnInField", vec![], default);
                        }
                        // A plain Return in a field types a newline as usual.
                        (true, false) => return true,
                        (false, true) => route(enter, "enterKey", vec![], KeyAction::RunMessage),
                        (false, false) => route(ret, "returnKey", vec![], KeyAction::RunMessage),
                    }
                    false
                }
                egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.command => {
                    route(tab, "tabKey", vec![], KeyAction::NextField);
                    false
                }
                egui::Event::Key { key, pressed: true, .. } if !editing && function_key(*key).is_some() => {
                    let n = function_key(*key).unwrap();
                    let default = if n == 1 { KeyAction::Undo } else { KeyAction::Nothing };
                    route(function, "functionKey", vec![n.to_string()], default);
                    false
                }
                _ => true,
            })
        });
        for (message, args, default) in sends {
            self.send(Event::Key { target, message: message.into(), args, default });
        }
        for action in local {
            self.key_action(ctx, target, action);
        }
    }

    /// In an autoTab field, Return on the last line moves to the next field.
    fn auto_tab_at_end(&self, ctx: &egui::Context) -> bool {
        let Some(e) = &self.editing else { return false };
        let auto_tab = self.last_scene.as_ref().is_some_and(|s| {
            s.parts.iter().any(|pv| pv.obj == e.obj && pv.part.auto_tab)
        });
        if !auto_tab {
            return false;
        }
        let cursor = egui::text_edit::TextEditState::load(ctx, field_edit_id(e.obj))
            .and_then(|s| s.cursor.char_range())
            .map(|r| r.primary.index.0)
            .unwrap_or(0);
        !e.buffer.chars().skip(cursor).any(|c| c == '\n')
    }

    /// Do what a key does when no script took its message.
    fn key_action(&mut self, ctx: &egui::Context, target: ObjRef, action: KeyAction) {
        match action {
            KeyAction::Insert(text) => self.insert_in_field(ctx, target, &text),
            KeyAction::NextField => self.next_field(),
            KeyAction::CloseField => self.end_editing(),
            KeyAction::BlindType(text) => {
                // The first key starts a new message; later ones continue it.
                let continuing = self.msg_has_focus || self.focus_msg_box;
                let mut w = self.world();
                if !continuing {
                    w.msg.clear();
                }
                w.msg.push_str(&text);
                w.msg_visible = true;
                drop(w);
                self.focus_msg_box = true;
            }
            KeyAction::RunMessage => {
                let msg = self.world().msg.clone();
                if !msg.trim().is_empty() {
                    self.send(Event::MsgBox(msg));
                }
            }
            KeyAction::Menu(item) => self.do_menu(&item),
            KeyAction::Undo => self.undo(),
            KeyAction::Redo => self.redo(),
            KeyAction::Nothing => {}
        }
    }

    /// Type text at the cursor of the field being edited, replacing any selection.
    fn insert_in_field(&mut self, ctx: &egui::Context, target: ObjRef, text: &str) {
        let Some(e) = &mut self.editing else { return };
        if e.obj != target {
            return;
        }
        let id = field_edit_id(e.obj);
        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        let len = e.buffer.chars().count();
        let (a, b) = state
            .cursor
            .char_range()
            .map(|r| (r.primary.index.0, r.secondary.index.0))
            .map(|(p, q)| (p.min(q), p.max(q)))
            .unwrap_or((len, len));
        let byte = |i: usize| e.buffer.char_indices().nth(i).map(|(b, _)| b).unwrap_or(e.buffer.len());
        let (ba, bb) = (byte(a.min(len)), byte(b.min(len)));
        e.buffer.replace_range(ba..bb, text);
        let at = a.min(len) + text.chars().count();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(at))));
        state.store(ctx, id);
        e.changed = true;
        e.synced = e.buffer.clone();
        if let ObjRef::Part { layer, id, card } = e.obj {
            let mut w = lock(&self.rt.world);
            w.stack.set_field_text(card, layer, id, e.buffer.clone());
            w.dirty = true;
        }
    }

    /// Tab: move to the next unlocked, visible field (background fields
    /// first, then card fields), selecting its text, as HyperCard does.
    fn next_field(&mut self) {
        let Some(scene) = &self.last_scene else { return };
        let fields: Vec<(ObjRef, String)> = scene
            .parts
            .iter()
            .filter(|pv| pv.part.kind == PartKind::Field && pv.part.visible && !pv.part.lock_text)
            .map(|pv| (pv.obj, pv.text.clone()))
            .collect();
        if fields.is_empty() {
            return;
        }
        let current = self.editing.as_ref().and_then(|e| fields.iter().position(|(o, _)| *o == e.obj));
        let (obj, text) = fields[current.map(|i| (i + 1) % fields.len()).unwrap_or(0)].clone();
        self.start_editing(obj, &text);
        if let Some(e) = &mut self.editing {
            e.select_all = true;
        }
    }

    fn delete_part(&mut self, obj: ObjRef) {
        let ObjRef::Part { layer, id, card } = obj else { return };
        let mut w = self.world();
        w.record_parts(layer);
        let bg = w.bg_id();
        let parts = match layer {
            Layer::Card => w.stack.card_mut(card).map(|c| &mut c.parts),
            Layer::Background => w.stack.background_mut(bg).map(|b| &mut b.parts),
        };
        if let Some(parts) = parts {
            parts.retain(|p| p.id != id);
        }
        w.dirty = true;
        drop(w);
        self.selected = None;
        self.infos.retain(|i| i.target != InfoTarget::Part(obj));
        self.editors.retain(|e| e.obj != obj);
    }

    // ----- painting -----

    // ----- button and field tools -----

    fn edit_tool_input(&mut self, resp: &egui::Response, origin: Pos2, tool: Tool, scene: &Scene) {
        let kind = if tool == Tool::Button { PartKind::Button } else { PartKind::Field };
        let to_card = |q: Pos2| ((q.x - origin.x) as i32, (q.y - origin.y) as i32);
        if resp.double_clicked()
            && let Some(o) = self.selected
        {
            self.open_info(InfoTarget::Part(o));
            return;
        }
        let pressed = resp.drag_started_by(PointerButton::Primary) || resp.clicked();
        if pressed && let Some(q) = resp.interact_pointer_pos() {
            let p = to_card(q);
            let hit = scene.parts.iter().rev().find(|pv| pv.part.kind == kind && pv.part.rect.contains(p.0, p.1));
            match hit {
                Some(pv) => {
                    self.selected = Some(pv.obj);
                    let r = pv.part.rect;
                    let near_corner = (p.0 - r.right).abs() <= 6 && (p.1 - r.bottom).abs() <= 6;
                    let kind = if near_corner { DragKind::Resize } else { DragKind::Move };
                    if resp.drag_started_by(PointerButton::Primary) {
                        if let ObjRef::Part { layer, .. } = pv.obj {
                            self.world().record_parts(layer);
                        }
                        self.part_drag = Some(PartDrag { obj: pv.obj, kind, start: p, rect: r });
                    }
                }
                None if resp.drag_started_by(PointerButton::Primary) && resp.ctx.input(|i| i.modifiers.command) => {
                    // Command-drag creates a new part, as in HyperCard.
                    let obj = self.create_part(kind, CardRect::new(p.0, p.1, p.0 + 1, p.1 + 1));
                    self.selected = Some(obj);
                    self.part_drag =
                        Some(PartDrag { obj, kind: DragKind::Resize, start: p, rect: CardRect::new(p.0, p.1, p.0, p.1) });
                }
                None => self.selected = None,
            }
        }
        if resp.dragged()
            && let (Some(d), Some(q)) = (&self.part_drag, resp.interact_pointer_pos())
        {
            let p = to_card(q);
            let (dx, dy) = (p.0 - d.start.0, p.1 - d.start.1);
            let new = match d.kind {
                DragKind::Move => d.rect.translate(dx, dy),
                DragKind::Resize => {
                    CardRect::new(d.rect.left, d.rect.top, (d.rect.right + dx).max(d.rect.left + 8), (d.rect.bottom + dy).max(d.rect.top + 8))
                }
            };
            if let ObjRef::Part { layer, id, card } = d.obj {
                let mut w = self.world();
                if let Some(part) = w.stack.part_mut(card, layer, id) {
                    part.rect = new;
                }
                w.dirty = true;
            }
        }
        if resp.drag_stopped() {
            self.part_drag = None;
        }
    }

    fn create_part(&mut self, kind: PartKind, rect: CardRect) -> ObjRef {
        let mut w = self.world();
        let layer = w.current_layer();
        w.record_parts(layer);
        let id = w.stack.alloc_id();
        let (card, bg) = (w.card_id, w.bg_id());
        let n = w.stack.parts(card, layer).iter().filter(|p| p.kind == kind).count() + 1;
        let part = match kind {
            PartKind::Button => Part::new_button(id, &format!("Button {n}"), rect),
            PartKind::Field => Part::new_field(id, "", rect),
        };
        match layer {
            Layer::Card => {
                if let Some(c) = w.stack.card_mut(card) {
                    c.parts.push(part);
                }
            }
            Layer::Background => {
                if let Some(b) = w.stack.background_mut(bg) {
                    b.parts.push(part);
                }
            }
        }
        w.dirty = true;
        ObjRef::Part { layer, id, card }
    }

    // ----- browse tool -----

    fn browse_input(&mut self, ctx: &egui::Context, resp: &egui::Response, origin: Pos2, scene: &Scene) {
        let busy = self.world().busy;
        let pos = ctx.input(|i| i.pointer.latest_pos());
        let card_pos = pos.map(|q| ((q.x - origin.x) as i32, (q.y - origin.y) as i32));
        // Over the card itself: not over a window in front of it, nor over a
        // scrolling field's scroll bar (scroll areas take their own clicks).
        let over_card = pos.is_some_and(|p| {
            resp.rect.contains(p)
                && ctx.layer_id_at(p) == Some(resp.layer_id)
                && !self.scroll_bar_zones.iter().any(|z| z.contains(p))
        });
        let over = if over_card { card_pos.and_then(|(x, y)| scene.part_at(x, y)) } else { None };
        let over_obj = over.map(|pv| pv.obj);
        let card = ObjRef::Card(scene.card);

        // mouseEnter / mouseLeave / mouseWithin
        if over_obj != self.hover {
            if let Some(old) = self.hover {
                self.send(Event::Send { target: old, message: "mouseLeave".into(), args: vec![] });
            }
            if let Some(new) = over_obj {
                self.send(Event::Send { target: new, message: "mouseEnter".into(), args: vec![] });
            }
            self.hover = over_obj;
        }
        let repeat_due = !busy && self.last_repeat_msg.elapsed() > Duration::from_millis(120);
        if let Some(o) = over_obj
            && repeat_due
            && self.pressed.is_none()
        {
            self.last_repeat_msg = Instant::now();
            self.send(Event::Send { target: o, message: "mouseWithin".into(), args: vec![] });
        }

        let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
        let primary_released = ctx.input(|i| i.pointer.primary_released());
        if primary_pressed && over_card {
            {
                let mut w = self.world();
                if let Some((x, y)) = card_pos {
                    w.mouse.click_x = x;
                    w.mouse.click_y = y;
                }
                w.mouse.clicked = true;
            }
            let target = over;
            match target {
                Some(pv) if pv.part.kind == PartKind::Field && !pv.part.lock_text => {
                    if self.editing.as_ref().is_none_or(|e| e.obj != pv.obj) {
                        self.start_editing(pv.obj, &pv.text);
                    }
                    return;
                }
                Some(pv) if pv.part.kind == PartKind::Button && !pv.part.enabled => return,
                _ => {}
            }
            self.end_editing();
            let t = target.map(|pv| pv.obj).unwrap_or(card);
            self.pressed = Some(t);
            self.pressed_inside = true;
            self.send(Event::Send { target: t, message: "mouseDown".into(), args: vec![] });
        }
        if let Some(t) = self.pressed {
            self.pressed_inside = match t {
                ObjRef::Part { .. } => over_obj == Some(t),
                _ => true,
            };
            if !primary_released && repeat_due && ctx.input(|i| i.pointer.primary_down()) {
                self.last_repeat_msg = Instant::now();
                self.send(Event::Send { target: t, message: "mouseStillDown".into(), args: vec![] });
            }
            if primary_released {
                self.pressed = None;
                if self.pressed_inside {
                    self.toggle_auto_hilite(t, scene);
                    self.send(Event::Send { target: t, message: "mouseUp".into(), args: vec![] });
                    if resp.double_clicked() {
                        self.send(Event::Send { target: t, message: "mouseDoubleClick".into(), args: vec![] });
                    }
                }
            }
        }
    }

    fn toggle_auto_hilite(&self, t: ObjRef, scene: &Scene) {
        let Some(pv) = scene.parts.iter().find(|p| p.obj == t) else { return };
        let p = &pv.part;
        if p.kind != PartKind::Button || !p.auto_hilite {
            return;
        }
        let ObjRef::Part { layer, id, card } = t else { return };
        let mut w = self.world();
        match p.style {
            PartStyle::CheckBox => w.stack.set_hilite(card, layer, id, !pv.hilite),
            PartStyle::Radio => w.stack.set_hilite(card, layer, id, true),
            _ => {}
        }
    }

    fn start_editing(&mut self, obj: ObjRef, text: &str) {
        self.end_editing();
        if let ObjRef::Part { layer, id, .. } = obj {
            self.world().focus = Some((layer, id));
        }
        self.editing =
            Some(Editing {
                obj,
                buffer: text.to_string(),
                synced: text.to_string(),
                changed: false,
                focus_requested: false,
                select_all: false,
            });
        self.send(Event::Send { target: obj, message: "openField".into(), args: vec![] });
    }

    fn end_editing(&mut self) {
        let Some(e) = self.editing.take() else { return };
        self.world().focus = None;
        let msg = if e.changed { "closeField" } else { "exitField" };
        self.send(Event::Send { target: e.obj, message: msg.into(), args: vec![] });
    }

    /// The in-place text editor for the field being edited.
    fn field_editor(&mut self, ui: &mut egui::Ui, origin: Pos2, scene: &Scene) {
        let Some(e) = &mut self.editing else { return };
        let Some(pv) = scene.parts.iter().find(|p| p.obj == e.obj) else {
            self.editing = None;
            return;
        };
        // A script may have changed the text underneath us.
        if pv.text != e.synced {
            e.buffer = pv.text.clone();
            e.synced = pv.text.clone();
        }
        let rect = render::part_rect(&pv.part, origin);
        render::draw_field_frame(ui.painter(), &pv.part, rect);
        let text_rect = render::field_text_rect(&pv.part, rect);
        let part = pv.part.clone();
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
            let mut job = render::field_job(&part, buf.as_str(), wrap, None);
            // The editor positions rows itself, so lay out left-aligned while typing.
            job.halign = egui::Align::LEFT;
            ui.painter().layout_job(job)
        };
        let edit = egui::TextEdit::multiline(&mut e.buffer)
            .id(field_edit_id(e.obj))
            .frame(egui::Frame::NONE)
            .margin(Vec2::ZERO)
            .desired_width(text_rect.width())
            .text_color(Color32::BLACK)
            .layouter(&mut layouter);
        let pv = pv.clone();
        let resp = if pv.part.style == PartStyle::Scrolling {
            field_scroll_area(ui, &pv, origin, &self.rt.world, &mut self.synced_scroll, &mut self.scroll_bar_zones, |ui| {
                ui.add(edit)
            })
        } else {
            ui.put(text_rect, edit)
        };
        let Some(e) = &mut self.editing else { return };
        if !e.focus_requested {
            resp.request_focus();
            e.focus_requested = true;
            if e.select_all {
                let id = field_edit_id(e.obj);
                let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
                let all = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(e.buffer.chars().count()),
                );
                state.cursor.set_char_range(Some(all));
                state.store(ui.ctx(), id);
            }
        }
        if resp.changed()
            && let ObjRef::Part { layer, id, card } = e.obj
        {
            e.changed = true;
            e.synced = e.buffer.clone();
            let mut w = self.rt.world.lock().unwrap_or_else(|p| p.into_inner());
            w.stack.set_field_text(card, layer, id, e.buffer.clone());
            w.dirty = true;
        }
        let clicked_elsewhere = ui.input(|i| i.pointer.primary_pressed())
            && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| !rect.contains(p));
        if (resp.lost_focus() && !resp.has_focus()) || clicked_elsewhere {
            self.end_editing();
        }
    }

    // ----- the card -----

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let (live, tool, lock_screen, transition, size) = {
            let w = lock(&self.rt.world);
            let scene = Scene::capture(&w);
            self.tex.refresh(&ctx, &w, &scene);
            if let Some(s) = &self.last_scene {
                self.tex.refresh(&ctx, &w, s);
            }
            (scene, w.tool, w.lock_screen, w.transition.clone(), Vec2::new(w.stack.width as f32, w.stack.height as f32))
        };
        // Lock screen keeps showing what was there; a new transition starts from it.
        if lock_screen && self.frozen.is_none() {
            self.frozen = self.last_scene.clone();
        } else if !lock_screen {
            self.frozen = None;
        }
        if let Some(t) = transition
            && t.seq != self.seen_transition
        {
            self.seen_transition = t.seq;
            if let Some(old) = self.last_scene.clone() {
                self.anim = Some(Anim { old, effect: t.effect, seconds: t.seconds, start: Instant::now() });
            }
        }
        let shown = self.frozen.clone().unwrap_or_else(|| live.clone());

        let avail = ui.available_size();
        // Whole-pixel placement keeps 1-pixel picture lines crisp.
        let pad = Vec2::new(((avail.x - size.x) / 2.0).max(8.0).round(), ((avail.y - size.y) / 2.0).max(8.0).round());
        ui.add_space(pad.y);
        let (resp, painter) = ui
            .horizontal(|ui| {
                ui.add_space(pad.x);
                ui.allocate_painter(size, Sense::click_and_drag())
            })
            .inner;
        let origin = resp.rect.min.round();
        let painter = painter.with_clip_rect(resp.rect);

        // Frame around the card; striped when editing the background.
        let frame_rect = resp.rect.expand(2.0);
        if live.edit_bg {
            for i in 0..4 {
                let r = resp.rect.expand(2.0 + i as f32 * 2.0);
                let c = if i % 2 == 0 { Color32::BLACK } else { Color32::WHITE };
                ui.painter().rect_stroke(r, 0.0, Stroke::new(2.0, c), StrokeKind::Outside);
            }
        } else {
            ui.painter().rect_stroke(frame_rect, 0.0, Stroke::new(2.0, Color32::BLACK), StrokeKind::Inside);
        }

        let pressed = if self.pressed_inside { self.pressed } else { None };
        let pressed_button = pressed.filter(|o| {
            shown.parts.iter().any(|pv| pv.obj == *o && pv.part.kind == PartKind::Button && pv.part.auto_hilite)
        });
        let animating = self.anim.as_ref().is_some_and(|a| a.start.elapsed().as_secs_f32() < a.seconds);
        // Scrolling fields are live scroll areas while browsing a card that's
        // on screen (not mid-transition or behind lock screen).
        let live_fields = tool == Tool::Browse && !animating && self.frozen.is_none();
        match &self.anim {
            Some(a) if animating => {
                let t = a.start.elapsed().as_secs_f32() / a.seconds;
                render::draw_transition(&painter, &self.tex, &a.old, &shown, origin, &a.effect, t);
            }
            _ => {
                self.anim = None;
                render::draw_scene(&painter, &self.tex, &shown, origin, pressed_button, live_fields);
            }
        }
        self.scroll_bar_zones.clear();
        if live_fields {
            let editing = self.editing.as_ref().map(|e| e.obj);
            for pv in shown.parts.iter().filter(|pv| {
                pv.part.visible
                    && pv.part.kind == PartKind::Field
                    && pv.part.style == PartStyle::Scrolling
                    && Some(pv.obj) != editing
            }) {
                let found = shown.found.as_ref().filter(|f| pv.obj == (ObjRef::Part { layer: f.layer, id: f.field, card: f.card }));
                let found = found.map(|f| (f.start, f.end));
                field_scroll_area(ui, pv, origin, &self.rt.world, &mut self.synced_scroll, &mut self.scroll_bar_zones, |ui| {
                    show_field_text(ui, pv, origin, found)
                });
            }
        }

        // Update the mouse state scripts can query.
        {
            let (pos, down, mods) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down(), i.modifiers));
            let mut w = self.world();
            if let Some(p) = pos {
                w.mouse.x = (p.x - origin.x) as i32;
                w.mouse.y = (p.y - origin.y) as i32;
            }
            w.mouse.down = down;
            w.mouse.shift = mods.shift;
            w.mouse.option = mods.alt;
            w.mouse.command = mods.command;
        }

        // Script-driven focus changes (`select text of field 1`).
        let focus = self.world().focus;
        if tool == Tool::Browse {
            match (focus, &self.editing) {
                (Some((layer, id)), None) => {
                    let obj = ObjRef::Part { layer, id, card: live.card };
                    if let Some(pv) = live.parts.iter().find(|p| p.obj == obj) {
                        let text = pv.text.clone();
                        self.start_editing(obj, &text);
                    }
                }
                (None, Some(_)) => self.editing = None,
                _ => {}
            }
        }

        match tool {
            Tool::Browse => {
                self.browse_input(&ctx, &resp, origin, &live);
                self.field_editor(ui, origin, &live);
            }
            Tool::Button | Tool::Field => {
                self.edit_tool_input(&resp, origin, tool, &live);
                let kind = if tool == Tool::Button { PartKind::Button } else { PartKind::Field };
                for pv in live.parts.iter().filter(|p| p.part.kind == kind) {
                    let r = render::part_rect(&pv.part, origin);
                    let color = if pv.part.visible { Color32::BLACK } else { Color32::from_gray(160) };
                    render::dashed_rect(&painter, r, color);
                    if Some(pv.obj) == self.selected {
                        render::selection_handles(&painter, r);
                    }
                }
            }
            _ => {
                self.paint_input(&ctx, &resp, origin, tool);
                self.paint_overlay(&painter, origin, tool, &ctx);
                if tool == Tool::Text {
                    self.text_tool_editor(ui, origin);
                }
            }
        }

        if let Some(until) = self.flash_until {
            if Instant::now() < until {
                painter.rect_filled(resp.rect, 0.0, Color32::from_black_alpha(160));
            } else {
                self.flash_until = None;
            }
        }
        if tool.is_paint() || matches!(tool, Tool::Button | Tool::Field) {
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        self.last_scene = Some(shown);
    }

    fn message_box(&mut self, ui: &mut egui::Ui) {
        let world_msg = self.world().msg.clone();
        if !self.msg_has_focus {
            self.msg_buffer = world_msg;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Message").strong());
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.msg_buffer)
                    .id(egui::Id::unique("message-box"))
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .hint_text("Type HyperTalk here and press Return, e.g.  go next card"),
            );
            if self.focus_msg_box {
                // Blind typing: continue in the message box, at the end.
                self.focus_msg_box = false;
                resp.request_focus();
                let id = resp.id;
                let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
                let end = egui::text::CCursor::new(self.msg_buffer.chars().count());
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), id);
            }
            self.msg_has_focus = resp.has_focus();
            if resp.changed() {
                self.world().msg = self.msg_buffer.clone();
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let text = self.msg_buffer.clone();
                if !text.trim().is_empty() {
                    self.send(Event::MsgBox(text));
                }
                resp.request_focus();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let (text, busy) = {
            let w = self.world();
            let idx = w.card_index() + 1;
            let n = w.stack.cards.len();
            let card = w.stack.card(w.card_id);
            let name = card.map(|c| c.name.clone()).filter(|n| !n.is_empty());
            let name = name.map(|n| format!(" \"{n}\"")).unwrap_or_default();
            let layer = if w.edit_bg { " · editing background" } else { "" };
            (format!("Card {idx} of {n}{name} · {} tool{layer}", w.tool.name()), w.busy)
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(text).small());
            if busy {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Script running — Ctrl+. to stop").small().color(Color32::DARK_RED));
                });
            }
        });
    }

    fn quit_prompt(&mut self, ctx: &egui::Context) {
        if !self.quit_prompt {
            return;
        }
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("quit-prompt")).show(ctx, |ui| {
            ui.label("This stack has never been saved. Save it before quitting?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save…").strong()).clicked() {
                    choice = Some(0);
                }
                if ui.button("Don't Save").clicked() {
                    choice = Some(1);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.quit_prompt = false;
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("RandomCard stack", &[STACK_EXT])
                    .set_file_name(format!("{}.{STACK_EXT}", self.world().stack.name))
                    .save_file()
                {
                    let path = with_ext(path);
                    let r = self.world().stack.save(&path);
                    match r {
                        Ok(()) => {
                            self.world().dirty = false;
                            self.request_quit(ctx);
                        }
                        Err(e) => self.alert(e),
                    }
                }
            }
            Some(1) => {
                self.quit_prompt = false;
                self.world().dirty = false;
                self.request_quit(ctx);
            }
            Some(2) => self.quit_prompt = false,
            _ => {}
        }
    }
}

/// Show a scrolling field's contents in a standard scroll area, keeping the
/// field's `scroll` property (pixels scrolled) and the scroll area in step
/// both ways.
fn field_scroll_area<R>(
    ui: &mut egui::Ui,
    pv: &render::PartView,
    origin: Pos2,
    world: &crate::runtime::world::Shared,
    synced: &mut std::collections::HashMap<ObjRef, i32>,
    bar_zones: &mut Vec<Rect>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let rect = render::part_rect(&pv.part, origin);
    let area = render::scroll_view_rect(&pv.part, rect);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let mut scroll_area = egui::ScrollArea::vertical()
        .id_salt(("field-scroll", format!("{:?}", pv.obj)))
        .auto_shrink([false, false])
        .max_height(area.height())
        .scroll_source(egui::containers::scroll_area::ScrollSource {
            scroll_bar: true,
            drag: egui::containers::scroll_area::DragScroll::Never,
            mouse_wheel: true,
        });
    // A script (or a new card) changed the scroll: move the scroll area.
    if synced.get(&pv.obj) != Some(&pv.part.scroll) {
        scroll_area = scroll_area.vertical_scroll_offset(pv.part.scroll as f32);
    }
    let out = scroll_area.show(&mut child, add);
    let now = out.state.clamped_offset().y.round() as i32;
    synced.insert(pv.obj, now);
    if now != pv.part.scroll
        && let ObjRef::Part { layer, id, card } = pv.obj
    {
        // The user scrolled: publish it as the field's scroll.
        if let Some(p) = lock(world).stack.part_mut(card, layer, id) {
            p.scroll = now;
        }
    }
    bar_zones.push(Rect::from_min_max(Pos2::new(area.right() - render::SCROLLBAR, area.top()), area.max));
    out.inner
}

/// The text of a scrolling field that isn't being edited.
fn show_field_text(ui: &mut egui::Ui, pv: &render::PartView, origin: Pos2, found: Option<(usize, usize)>) {
    let text_rect = render::field_text_rect(&pv.part, render::part_rect(&pv.part, origin));
    let galley = ui.painter().layout_job(render::field_job(&pv.part, &pv.text, text_rect.width(), found));
    let (r, _) = ui.allocate_exact_size(Vec2::new(text_rect.width(), galley.size().y), Sense::hover());
    let pos = Pos2::new(render::aligned_x(&pv.part, r), r.top());
    ui.painter().galley(pos, galley.clone(), Color32::BLACK);
    if pv.part.text_style.to_ascii_lowercase().contains("bold") {
        ui.painter().galley(pos + Vec2::new(0.6, 0.0), galley, Color32::BLACK);
    }
}

/// F1–F12 as HyperCard function key numbers.
fn function_key(key: Key) -> Option<u32> {
    let keys = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
    ];
    keys.iter().position(|k| *k == key).map(|i| i as u32 + 1)
}

fn with_ext(p: PathBuf) -> PathBuf {
    if p.extension().is_none() { p.with_extension(STACK_EXT) } else { p }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_quit(&ctx);
        }
        self.handle_requests(&ctx);
        self.shortcuts(&ctx);
        self.keyboard(&ctx);

        let (title, beeps, busy, menubar, msg_visible) = {
            let w = self.world();
            let dirty = if w.dirty && w.stack.path.is_none() { " (unsaved)" } else { "" };
            (format!("RandomCard — {}{dirty}", w.stack.name), w.beeps, w.busy, w.menubar_visible, w.msg_visible)
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if beeps > self.beeps_seen {
            self.beeps_seen = beeps;
            eprint!("\x07");
            self.flash_until = Some(Instant::now() + Duration::from_millis(90));
        }
        if self.last_autosave.elapsed() > Duration::from_secs(3) {
            self.autosave();
        }

        if menubar {
            egui::Panel::top("menubar").show(ui, |ui| self.menu_bar(ui));
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            if msg_visible {
                self.message_box(ui);
            }
            self.status_bar(ui);
        });
        egui::Panel::left("palette").resizable(false).exact_size(128.0).show(ui, |ui| self.palette(ui));
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_gray(200))).show(ui, |ui| {
            self.canvas(ui);
        });

        self.show_windows(&ctx);
        self.show_dialog(&ctx);
        self.show_error(&ctx);
        self.quit_prompt(&ctx);

        if busy || self.anim.is_some() || self.pressed.is_some() || self.flash_until.is_some() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
