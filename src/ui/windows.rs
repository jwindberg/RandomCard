//! Floating windows: object info dialogs, the script editor, answer/ask
//! dialogs and script error alerts.

use eframe::egui::{self, Color32, RichText};

use crate::model::{Id, PartKind, PartStyle, TextAlign, shortcut};
use crate::runtime::world::{
    Dialog, DialogReply, Event, ObjRef, WindowKind, World, lock, same_script,
};

use super::App;
use super::script_find::{self, EditorCommand, FindOptions};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InfoTarget {
    Part(ObjRef),
    Card(Id),
    Background(Id),
    Stack,
}

pub struct InfoWindow {
    pub target: InfoTarget,
    pub open: bool,
}

pub struct ScriptEditor {
    pub obj: ObjRef,
    pub title: String,
    pub text: String,
    pub open: bool,
    pub error: Option<String>,
    /// Line to select when the editor is next drawn.
    pub goto_line: Option<usize>,
    /// The Find, Replace or Go to Line bar, when it's showing.
    pub bar: Option<Bar>,
    /// Put the keyboard in the bar when it's next drawn.
    pub bar_focus: bool,
    /// What the last Find or Replace found, such as "Not found".
    pub notice: Option<String>,
    /// A character to scroll into view when the editor is next drawn.
    pub scroll_to: Option<usize>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Bar {
    Find,
    Replace,
    /// Go to Line, and the line typed so far.
    Line(String),
}

/// What a click or Return in the bar asks for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BarAction {
    Find,
    Replace,
    ReplaceAll,
    GoToLine,
    Close,
}

/// The editor's selection, as character indexes from and to.
fn selection(ctx: &egui::Context, id: egui::Id) -> (usize, usize) {
    egui::text_edit::TextEditState::load(ctx, id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| {
            let [a, b] = r.sorted_cursors();
            (a.index.0, b.index.0)
        })
        .unwrap_or((0, 0))
}

/// Select characters in a text editor and give it the keyboard.
fn select(ctx: &egui::Context, id: egui::Id, (from, to): (usize, usize)) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
        egui::text::CCursor::new(from),
        egui::text::CCursor::new(to),
    )));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Room at the left of the script editor for line numbers and checkpoints.
const GUTTER: f32 = 40.0;

/// Where each line of a laid-out script is, relative to the galley; a line
/// that wraps covers several rows.
fn line_rows(galley: &egui::Galley) -> Vec<egui::Rect> {
    let mut lines: Vec<egui::Rect> = Vec::new();
    let mut new_line = true;
    for row in &galley.rows {
        let r = row.rect();
        match lines.last_mut() {
            Some(last) if !new_line => *last = last.union(r),
            _ => lines.push(r),
        }
        new_line = row.ends_with_newline;
    }
    lines
}

pub struct ErrorAlert {
    pub msg: String,
    pub obj: Option<ObjRef>,
    pub line: Option<usize>,
}

pub fn script_of(w: &World, obj: ObjRef) -> Option<String> {
    let s = &w.stack;
    Some(match obj {
        ObjRef::Stack => s.script.clone(),
        ObjRef::Background(id) => s.background(id)?.script.clone(),
        ObjRef::Card(id) => s.card(id)?.script.clone(),
        ObjRef::Part { layer, id, card } => s.part(card, layer, id)?.script.clone(),
        ObjRef::OtherStack(k) => w.other(k)?.script.clone(),
        _ => return None,
    })
}

fn set_script(w: &mut World, obj: ObjRef, text: String) {
    let s = &mut w.stack;
    match obj {
        ObjRef::Stack => s.script = text,
        ObjRef::Background(id) => {
            if let Some(b) = s.background_mut(id) {
                b.script = text;
            }
        }
        ObjRef::Card(id) => {
            if let Some(c) = s.card_mut(id) {
                c.script = text;
            }
        }
        ObjRef::Part { layer, id, card } => {
            if let Some(p) = s.part_mut(card, layer, id) {
                p.script = text;
            }
        }
        // Home or a stack in use: saved to its file right away.
        ObjRef::OtherStack(k) => {
            if let Some(s) = w.other_mut(k) {
                s.script = text;
                if let Some(path) = s.path.clone() {
                    let _ = s.save(&path);
                }
            }
            return;
        }
        _ => return,
    }
    w.dirty = true;
}

/// Re-indent a script the way HyperCard's editor does.
pub fn format_script(src: &str) -> String {
    let mut out = Vec::new();
    let mut level: i32 = 0;
    for raw in src.lines() {
        let line = raw.trim();
        let code = line.split("--").next().unwrap_or("").trim().to_ascii_lowercase();
        let first = code.split_whitespace().next().unwrap_or("");
        if first == "end" || first == "else" {
            level -= 1;
        }
        level = level.max(0);
        let indent = "  ".repeat(level as usize);
        out.push(if line.is_empty() { String::new() } else { format!("{indent}{line}") });
        match first {
            "on" | "function" | "repeat" => level += 1,
            "if" if code.ends_with("then") => level += 1,
            "else" if code == "else" || code.ends_with("then") => level += 1,
            _ => {}
        }
    }
    let mut s = out.join("\n");
    if src.ends_with('\n') {
        s.push('\n');
    }
    s
}

impl App {
    pub fn open_info(&mut self, target: InfoTarget) {
        if let Some(w) = self.infos.iter_mut().find(|w| w.target == target) {
            w.open = true;
            return;
        }
        self.infos.push(InfoWindow { target, open: true });
    }

    pub fn open_script(&mut self, obj: ObjRef, line: Option<usize>) {
        if let Some(e) = self.editors.iter_mut().find(|e| e.obj == obj) {
            e.open = true;
            e.goto_line = line;
            return;
        }
        let (text, title) = {
            let w = lock(&self.rt.world);
            (script_of(&w, obj).unwrap_or_default(), object_title(&w, obj))
        };
        self.editors.push(ScriptEditor {
            obj,
            title,
            text,
            open: true,
            error: None,
            goto_line: line,
            bar: None,
            bar_focus: false,
            notice: None,
            scroll_to: None,
        });
    }

    pub fn show_windows(&mut self, ctx: &egui::Context) {
        let mut open_script = None;
        let mut infos = std::mem::take(&mut self.infos);
        for win in &mut infos {
            let mut open = win.open;
            let title = {
                let w = lock(&self.rt.world);
                match win.target {
                    InfoTarget::Part(o) => format!("{} Info", capitalize(&object_title(&w, o))),
                    InfoTarget::Card(_) => "Card Info".to_string(),
                    InfoTarget::Background(_) => "Background Info".to_string(),
                    InfoTarget::Stack => "Stack Info".to_string(),
                }
            };
            egui::Window::new(title)
                .id(egui::Id::unique(("info", info_key(win.target))))
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    if let Some(obj) = self.info_contents(ui, win.target) {
                        open_script = Some(obj);
                    }
                });
            win.open = open;
        }
        infos.retain(|w| w.open);
        self.infos = infos;
        if let Some(obj) = open_script {
            self.open_script(obj, None);
        }

        let mut editors = std::mem::take(&mut self.editors);
        let mut focus = None;
        for ed in &mut editors {
            if self.script_window(ctx, ed) && ed.open {
                focus = Some(ed.obj);
            }
        }
        // The editor with the keyboard takes its Command keys next frame.
        self.script_focus = focus;
        editors.retain(|e| e.open);
        self.editors = editors;
    }

    /// Returns an object whose script should be opened.
    fn info_contents(&mut self, ui: &mut egui::Ui, target: InfoTarget) -> Option<ObjRef> {
        // Scripts belong to the Scripting level.
        let scripting = self.level() >= 5;
        let mut w = lock(&self.rt.world);
        let mut changed = false;
        let mut script = None;
        match target {
            InfoTarget::Part(obj) => {
                let ObjRef::Part { layer, id, card } = obj else { return None };
                let number = {
                    let parts = w.stack.parts(card, layer);
                    let kind = parts.iter().find(|p| p.id == id).map(|p| p.kind);
                    parts.iter().filter(|p| Some(p.kind) == kind).position(|p| p.id == id).map(|n| n + 1)
                };
                let hilite = w.stack.hilite(card, layer, id);
                let icons: Vec<(i32, String)> = w.stack.icons.iter().map(|(i, icon)| (*i, icon.name.clone())).collect();
                // Then HyperCard's own icons, as its icon picker offered them.
                let builtin: Vec<(i32, String)> = crate::import::hypercard_icons()
                    .iter()
                    .filter(|(i, _)| !w.stack.icons.contains_key(i))
                    .map(|(i, icon)| (*i, icon.name.clone()))
                    .collect();
                let text = w.stack.field_text(card, layer, id);
                let Some(p) = w.stack.part_mut(card, layer, id) else {
                    ui.label("This object no longer exists.");
                    return None;
                };
                egui::Grid::new("part-info").num_columns(2).show(ui, |ui| {
                    ui.label(format!("{} Name:", capitalize(p.kind.name())));
                    changed |= ui.text_edit_singleline(&mut p.name).changed();
                    ui.end_row();
                    ui.label("Layer:");
                    ui.label(if layer == crate::model::Layer::Card { "Card" } else { "Background" });
                    ui.end_row();
                    ui.label(format!("{} Number:", capitalize(p.kind.name())));
                    ui.label(number.map(|n| n.to_string()).unwrap_or_default());
                    ui.end_row();
                    ui.label("ID:");
                    ui.label(p.id.to_string());
                    ui.end_row();
                    ui.label("Style:");
                    let styles =
                        if p.kind == PartKind::Button { PartStyle::BUTTON_STYLES } else { PartStyle::FIELD_STYLES };
                    egui::ComboBox::from_id_salt("style").selected_text(p.style.name()).show_ui(ui, |ui| {
                        for s in styles {
                            changed |= ui.selectable_value(&mut p.style, *s, s.name()).changed();
                        }
                    });
                    ui.end_row();
                    ui.label("Text:");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("font").selected_text(&p.text_font).width(90.0).show_ui(
                            ui,
                            |ui| {
                                for f in ["Geneva", "Chicago", "Helvetica", "Times", "Courier", "Monaco"] {
                                    changed |= ui.selectable_value(&mut p.text_font, f.to_string(), f).changed();
                                }
                            },
                        );
                        changed |= ui.add(egui::DragValue::new(&mut p.text_size).range(6..=72).suffix(" pt")).changed();
                        egui::ComboBox::from_id_salt("align").selected_text(p.text_align.name()).width(70.0).show_ui(
                            ui,
                            |ui| {
                                for a in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
                                    changed |= ui.selectable_value(&mut p.text_align, a, a.name()).changed();
                                }
                            },
                        );
                    });
                    ui.end_row();
                    ui.label("Text style:");
                    ui.horizontal(|ui| {
                        for st in ["bold", "italic", "underline"] {
                            let mut on = p.text_style.to_ascii_lowercase().contains(st);
                            if ui.checkbox(&mut on, capitalize(st)).changed() {
                                let mut styles: Vec<String> = p
                                    .text_style
                                    .split(',')
                                    .map(|s| s.trim().to_ascii_lowercase())
                                    .filter(|s| !s.is_empty() && s != "plain" && s != st)
                                    .collect();
                                if on {
                                    styles.push(st.to_string());
                                }
                                p.text_style = if styles.is_empty() { "plain".into() } else { styles.join(",") };
                                changed = true;
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Colors:");
                    ui.horizontal(|ui| {
                        changed |= color_choice(ui, "Text", &mut p.fore_color, [0, 0, 0]);
                        changed |= color_choice(ui, "Fill", &mut p.back_color, [255, 255, 255]);
                        changed |= color_choice(ui, "Frame", &mut p.border_color, [0, 0, 0]);
                    });
                    ui.end_row();
                });
                ui.separator();
                egui::Grid::new("part-flags").num_columns(2).show(ui, |ui| {
                    changed |= ui.checkbox(&mut p.visible, "Visible").changed();
                    if p.kind == PartKind::Button {
                        changed |= ui.checkbox(&mut p.show_name, "Show Name").changed();
                        ui.end_row();
                        changed |= ui.checkbox(&mut p.auto_hilite, "Auto Hilite").changed();
                        changed |= ui.checkbox(&mut p.enabled, "Enabled").changed();
                        ui.end_row();
                        if layer == crate::model::Layer::Background {
                            changed |= ui.checkbox(&mut p.shared_hilite, "Shared Hilite").changed();
                        }
                        ui.horizontal(|ui| {
                            ui.label("Family:");
                            changed |= ui.add(egui::DragValue::new(&mut p.family).range(0..=15)).changed();
                        });
                        ui.end_row();
                        if p.style == PartStyle::Popup {
                            // A popup has a title where other buttons have an icon.
                            ui.label("Title Width:");
                            let width = (p.rect.right - p.rect.left).max(0) as u32;
                            changed |= ui.add(egui::DragValue::new(&mut p.title_width).range(0..=width)).changed();
                        } else {
                            ui.label("Icon:");
                            let label = |id: i32| match icons.iter().chain(&builtin).find(|(i, _)| *i == id) {
                                _ if id == 0 => "None".to_string(),
                                Some((_, name)) if !name.is_empty() => format!("{id} {name}"),
                                _ => id.to_string(),
                            };
                            egui::ComboBox::from_id_salt("icon").selected_text(label(p.icon)).show_ui(ui, |ui| {
                                changed |= ui.selectable_value(&mut p.icon, 0, "None").changed();
                                for (i, _) in &icons {
                                    changed |= ui.selectable_value(&mut p.icon, *i, label(*i)).changed();
                                }
                                if !builtin.is_empty() {
                                    ui.separator();
                                    ui.label(RichText::new("HyperCard's icons").weak());
                                    for (i, _) in &builtin {
                                        changed |= ui.selectable_value(&mut p.icon, *i, label(*i)).changed();
                                    }
                                }
                            });
                        }
                        ui.end_row();
                        ui.label(format!("Hilite: {hilite}"));
                    } else {
                        changed |= ui.checkbox(&mut p.lock_text, "Lock Text").changed();
                        ui.end_row();
                        changed |= ui.checkbox(&mut p.dont_wrap, "Don't Wrap").changed();
                        changed |= ui.checkbox(&mut p.show_lines, "Show Lines").changed();
                        ui.end_row();
                        changed |= ui.checkbox(&mut p.wide_margins, "Wide Margins").changed();
                        changed |= ui.checkbox(&mut p.dont_search, "Don't Search").changed();
                        ui.end_row();
                        changed |= ui.checkbox(&mut p.auto_tab, "Auto Tab").changed();
                        if layer == crate::model::Layer::Background {
                            changed |= ui.checkbox(&mut p.shared_text, "Shared Text").changed();
                        }
                        ui.end_row();
                        changed |= ui.checkbox(&mut p.fixed_line_height, "Fixed Line Height").changed();
                        changed |= ui.checkbox(&mut p.dont_save, "Don't Save")
                            .on_hover_text("Its text isn't saved: it opens empty")
                            .changed();
                        ui.end_row();
                        ui.label(format!("{} characters", text.chars().count()));
                    }
                    ui.end_row();
                });
                if p.kind == PartKind::Button {
                    let popup = p.style == PartStyle::Popup;
                    ui.label(if popup { "Menu items, one per line:" } else { "Contents:" });
                    let rows = if popup { 5 } else { 2 };
                    changed |= ui.add(egui::TextEdit::multiline(&mut p.contents).desired_rows(rows)).changed();
                }
                ui.separator();
                let r = p.rect;
                ui.label(RichText::new(format!("rect: {},{},{},{}", r.left, r.top, r.right, r.bottom)).weak());
                if scripting && ui.button("Script…").clicked() {
                    script = Some(obj);
                }
                // A HyperCard icon chosen for the button joins the stack's.
                let icon = p.icon;
                if icon != 0
                    && !w.stack.icons.contains_key(&icon)
                    && let Some(i) = crate::import::hypercard_icons().get(&icon)
                {
                    w.stack.icons.insert(icon, i.clone());
                }
            }
            InfoTarget::Card(id) => {
                let number = w.stack.card_index(id).map(|i| i + 1).unwrap_or(0);
                let count = w.stack.cards.len();
                let c = w.stack.card_mut(id)?;
                egui::Grid::new("card-info").num_columns(2).show(ui, |ui| {
                    ui.label("Card Name:");
                    changed |= ui.text_edit_singleline(&mut c.name).changed();
                    ui.end_row();
                    ui.label("Card Number:");
                    ui.label(format!("{number} out of {count}"));
                    ui.end_row();
                    ui.label("Card ID:");
                    ui.label(c.id.to_string());
                    ui.end_row();
                    ui.label("Contains:");
                    let (b, f) = (
                        c.parts.iter().filter(|p| p.kind == PartKind::Button).count(),
                        c.parts.iter().filter(|p| p.kind == PartKind::Field).count(),
                    );
                    ui.label(format!("{b} card buttons, {f} card fields"));
                    ui.end_row();
                });
                changed |= ui.checkbox(&mut c.marked, "Card Marked").changed();
                changed |= ui.checkbox(&mut c.dont_search, "Don't Search Card").changed();
                changed |= ui.checkbox(&mut c.cant_delete, "Can't Delete Card").changed();
                changed |= ui.checkbox(&mut c.show_pict, "Show Picture").changed();
                if scripting && ui.button("Script…").clicked() {
                    script = Some(ObjRef::Card(id));
                }
            }
            InfoTarget::Background(id) => {
                let cards = w.stack.cards_of_bg(id).len();
                let number = w.stack.bg_index(id).map(|i| i + 1).unwrap_or(0);
                let b = w.stack.background_mut(id)?;
                egui::Grid::new("bg-info").num_columns(2).show(ui, |ui| {
                    ui.label("Background Name:");
                    changed |= ui.text_edit_singleline(&mut b.name).changed();
                    ui.end_row();
                    ui.label("Background Number:");
                    ui.label(number.to_string());
                    ui.end_row();
                    ui.label("Background ID:");
                    ui.label(b.id.to_string());
                    ui.end_row();
                    ui.label("Shared by:");
                    ui.label(format!("{cards} cards"));
                    ui.end_row();
                });
                changed |= ui.checkbox(&mut b.dont_search, "Don't Search Background").changed();
                changed |= ui.checkbox(&mut b.cant_delete, "Can't Delete Background").changed();
                changed |= ui.checkbox(&mut b.show_pict, "Show Picture").changed();
                if scripting && ui.button("Script…").clicked() {
                    script = Some(ObjRef::Background(id));
                }
            }
            InfoTarget::Stack => {
                let path = w.stack.path.as_ref().map(|p| p.display().to_string()).unwrap_or("(not saved)".into());
                let (cards, bgs, width, height) =
                    (w.stack.cards.len(), w.stack.backgrounds.len(), w.stack.width, w.stack.height);
                egui::Grid::new("stack-info").num_columns(2).show(ui, |ui| {
                    ui.label("Stack Name:");
                    changed |= ui.text_edit_singleline(&mut w.stack.name).changed();
                    ui.end_row();
                    ui.label("Where:");
                    ui.label(path);
                    ui.end_row();
                    ui.label("Contains:");
                    ui.label(format!("{cards} cards, {bgs} backgrounds"));
                    ui.end_row();
                    ui.label("Card size:");
                    ui.label(format!("{width} × {height}"));
                    ui.end_row();
                });
                changed |= ui.checkbox(&mut w.stack.cant_modify, "Can't Modify Stack").changed();
                if scripting && ui.button("Script…").clicked() {
                    script = Some(ObjRef::Stack);
                }
            }
        }
        if changed {
            w.dirty = true;
        }
        script
    }

    /// Returns whether the editor, or its bar, has the keyboard.
    fn script_window(&mut self, ctx: &egui::Context, ed: &mut ScriptEditor) -> bool {
        let mut open = ed.open;
        let mut close = false;
        let id = egui::Id::unique(("script", format!("{:?}", ed.obj)));
        let edit_id = id.with("text");
        let (find_id, replace_id, line_id) = (id.with("find"), id.with("replace"), id.with("line"));
        let command = self.editor_command.take_if(|(o, _)| *o == ed.obj).map(|(_, c)| c);
        let mut action = None;
        match command {
            Some(EditorCommand::Find) => (ed.bar, ed.bar_focus) = (Some(Bar::Find), true),
            Some(EditorCommand::Replace) => (ed.bar, ed.bar_focus) = (Some(Bar::Replace), true),
            Some(EditorCommand::GoToLine) => (ed.bar, ed.bar_focus) = (Some(Bar::Line(String::new())), true),
            Some(EditorCommand::FindAgain) => action = Some(BarAction::Find),
            Some(EditorCommand::ReplaceAgain) => action = Some(BarAction::Replace),
            Some(EditorCommand::FindSelection) => {
                let found = script_find::slice(&ed.text, selection(ctx, edit_id));
                if !found.is_empty() {
                    self.script_find.find = found;
                    action = Some(BarAction::Find);
                }
            }
            Some(EditorCommand::Save) => self.save_script(ed),
            Some(EditorCommand::Checkpoint) | None => {}
        }
        egui::Window::new(format!("Script of {}", ed.title))
            .id(id)
            .open(&mut open)
            .default_size([520.0, 380.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.save_script(ed);
                    }
                    if ui.button("Format").clicked() {
                        ed.text = format_script(&ed.text);
                    }
                    if ui.button("Revert").clicked() {
                        ed.text = script_of(&lock(&self.rt.world), ed.obj).unwrap_or_default();
                        ed.error = None;
                    }
                    if ui.button("Close").clicked() {
                        self.save_script(ed);
                        close = true;
                    }
                    if ui.button("Find").clicked() {
                        (ed.bar, ed.bar_focus) = (Some(Bar::Find), true);
                    }
                    if ui.button("Go to Line").clicked() {
                        (ed.bar, ed.bar_focus) = (Some(Bar::Line(String::new())), true);
                    }
                    ui.label(
                        RichText::new(format!(
                            "{} saves · {} sets a checkpoint · {} finds",
                            shortcut("S"),
                            shortcut("D"),
                            shortcut("F")
                        ))
                        .weak(),
                    );
                });
                if let Some(a) = self.find_bar(ui, ed, [find_id, replace_id, line_id]) {
                    action = Some(a);
                }
                let stopped_at = self.debug_bar(ui, ed.obj);
                if let Some(err) = &ed.error {
                    ui.colored_label(Color32::from_rgb(170, 0, 0), err);
                }
                // While stopped here, the variables sit beside the script, so
                // values show as you step.
                if stopped_at.is_some() {
                    let pane = egui::Panel::right(id.with("variables")).resizable(true);
                    pane.default_size(200.0).min_size(120.0).show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("variables")
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.variables(ui, "editor"));
                    });
                }
                if let Some(a) = action {
                    self.find_action(ctx, ed, edit_id, a);
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(line) = ed.goto_line.take() {
                        let range = script_find::line_range(&ed.text, line);
                        select(ctx, edit_id, range);
                        ed.scroll_to = Some(range.0);
                    }
                    let out = egui::TextEdit::multiline(&mut ed.text)
                        .id(edit_id)
                        .code_editor()
                        .lock_focus(true)
                        .margin(egui::Margin { left: GUTTER as i8, right: 4, top: 2, bottom: 2 })
                        .desired_width(f32::INFINITY)
                        .desired_rows(18)
                        .show(ui);
                    let resp = &out.response;
                    if let Some(at) = ed.scroll_to.take() {
                        let r = out.galley.pos_from_cursor(egui::text::CCursor::new(at));
                        ui.scroll_to_rect(r.translate(out.galley_pos.to_vec2()).expand(24.0), Some(egui::Align::Center));
                    }
                    let lines = line_rows(&out.galley);
                    let row_rect = |line: usize| {
                        lines.get(line.wrapping_sub(1)).map(|r| r.translate(out.galley_pos.to_vec2()))
                    };
                    // Ctrl+D, or a click in the gutter, sets or clears a checkpoint.
                    let mut toggle = None;
                    if command == Some(EditorCommand::Checkpoint) {
                        toggle = Some(script_find::line_of(&ed.text, selection(ctx, edit_id).0));
                    }
                    if resp.clicked()
                        && let Some(p) = resp.interact_pointer_pos().filter(|p| p.x < resp.rect.left() + GUTTER)
                    {
                        toggle = (1..=lines.len()).find(|&l| row_rect(l).is_some_and(|r| p.y >= r.top() && p.y < r.bottom()));
                    }
                    let checkpoints: Vec<usize> = {
                        let mut w = lock(&self.rt.world);
                        if let Some(line) = toggle {
                            match w.checkpoints.iter().position(|(o, l)| same_script(*o, ed.obj) && *l == line) {
                                Some(i) => {
                                    w.checkpoints.remove(i);
                                }
                                None => w.checkpoints.push((ed.obj, line)),
                            }
                        }
                        w.checkpoints.iter().filter(|(o, _)| same_script(*o, ed.obj)).map(|(_, l)| *l).collect()
                    };
                    let mut shapes = Vec::new();
                    let left = resp.rect.left();
                    if let Some(r) = stopped_at.and_then(row_rect) {
                        let row = egui::Rect::from_x_y_ranges(left..=resp.rect.right(), r.y_range());
                        shapes.push(egui::Shape::rect_filled(row, 0.0, Color32::from_rgba_unmultiplied(255, 200, 0, 70)));
                    }
                    let gutter = egui::Rect::from_x_y_ranges(left..=left + GUTTER - 6.0, resp.rect.y_range());
                    shapes.push(egui::Shape::rect_filled(gutter, 0.0, ui.visuals().faint_bg_color));
                    let font = egui::FontId::monospace(10.0);
                    let number_color = ui.visuals().weak_text_color();
                    for line in 1..=lines.len() {
                        let Some(r) = row_rect(line) else { continue };
                        if checkpoints.contains(&line) {
                            let c = egui::pos2(left + 7.0, r.center().y);
                            shapes.push(egui::Shape::circle_filled(c, 4.5, Color32::from_rgb(200, 30, 30)));
                        }
                        let pos = egui::pos2(left + GUTTER - 9.0, r.center().y);
                        ui.fonts_mut(|f| {
                            shapes.push(egui::Shape::text(f, pos, egui::Align2::RIGHT_CENTER, line, font.clone(), number_color));
                        });
                    }
                    // Over the editor's background: the gutter, and the line the
                    // debugger is stopped at.
                    ui.painter().extend(shapes);
                });
            });
        if close {
            open = false;
        } else if !open {
            // Closing the window keeps the edits, as HyperCard's editor did when you said yes.
            self.save_script(ed);
        }
        if ed.open && !open {
            self.script_closed(ed.obj);
        }
        ed.open = open;
        ctx.memory(|m| [edit_id, find_id, replace_id, line_id].iter().any(|i| m.has_focus(*i)))
    }

    /// The bar under the editor's buttons: Find, Replace, or Go to Line.
    fn find_bar(&mut self, ui: &mut egui::Ui, ed: &mut ScriptEditor, ids: [egui::Id; 3]) -> Option<BarAction> {
        let [find_id, replace_id, line_id] = ids;
        let bar = ed.bar.clone()?;
        let mut action = None;
        // Tab in Find goes on to "Replace with", past the buttons between.
        let tab_to_replace = bar == Bar::Replace
            && ui.input(|i| i.key_pressed(egui::Key::Tab) && !i.modifiers.shift)
            && ui.memory(|m| m.has_focus(find_id));
        let o = &mut self.script_find;
        // Return in a field does its button; Escape closes the bar.
        // The key is used up here, or the script would get it next.
        let key = |ui: &egui::Ui, k: egui::Key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
        let entered = |ui: &egui::Ui, r: &egui::Response| r.lost_focus() && key(ui, egui::Key::Enter);
        let escaped = |ui: &egui::Ui, r: &egui::Response| (r.has_focus() || r.lost_focus()) && key(ui, egui::Key::Escape);
        let focus = std::mem::take(&mut ed.bar_focus);
        let field = |ui: &mut egui::Ui, text: &mut String, id: egui::Id, width: f32| {
            let r = ui.add(egui::TextEdit::singleline(text).id(id).desired_width(width));
            if focus && [find_id, line_id].contains(&id) {
                // Start with everything selected, so typing replaces it.
                select(ui.ctx(), id, (0, text.chars().count()));
            }
            r
        };
        match bar {
            Bar::Find | Bar::Replace => {
                ui.horizontal(|ui| {
                    ui.label("Find");
                    let r = field(ui, &mut o.find, find_id, 180.0);
                    if entered(ui, &r) {
                        action = Some(BarAction::Find);
                    }
                    if escaped(ui, &r) {
                        action = Some(BarAction::Close);
                    }
                    if ui.button("Find").clicked() {
                        action = Some(BarAction::Find);
                    }
                    if bar == Bar::Find && ui.button("Replace…").clicked() {
                        ed.bar = Some(Bar::Replace);
                    }
                    if ui.small_button("✕").on_hover_text("Close").clicked() {
                        action = Some(BarAction::Close);
                    }
                });
                if bar == Bar::Replace {
                    ui.horizontal(|ui| {
                        ui.label("Replace with");
                        let r = field(ui, &mut o.replace, replace_id, 180.0);
                        if entered(ui, &r) {
                            action = Some(BarAction::Replace);
                        }
                        if escaped(ui, &r) {
                            action = Some(BarAction::Close);
                        }
                        if ui.button("Replace").on_hover_text(shortcut("T")).clicked() {
                            action = Some(BarAction::Replace);
                        }
                        if ui.button("Replace All").clicked() {
                            action = Some(BarAction::ReplaceAll);
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.checkbox(&mut o.whole_word, "Whole Word");
                    ui.checkbox(&mut o.case_sensitive, "Case Sensitive");
                    ui.checkbox(&mut o.wraparound, "Wraparound Search");
                    if let Some(n) = &ed.notice {
                        ui.label(RichText::new(n).weak());
                    }
                });
            }
            Bar::Line(mut line) => {
                ui.horizontal(|ui| {
                    ui.label("Go to line");
                    let r = field(ui, &mut line, line_id, 60.0);
                    if entered(ui, &r) {
                        action = Some(BarAction::GoToLine);
                    }
                    if escaped(ui, &r) {
                        action = Some(BarAction::Close);
                    }
                    if ui.button("Go").clicked() {
                        action = Some(BarAction::GoToLine);
                    }
                    if ui.small_button("✕").on_hover_text("Close").clicked() {
                        action = Some(BarAction::Close);
                    }
                    if let Some(n) = &ed.notice {
                        ui.label(RichText::new(n).weak());
                    }
                });
                if ed.bar.is_some() {
                    ed.bar = Some(Bar::Line(line));
                }
            }
        }
        if tab_to_replace {
            ui.memory_mut(|m| m.request_focus(replace_id));
        }
        action
    }

    /// Find, Replace, Replace All or Go to Line in the script. The
    /// selection shows what was found or changed.
    fn find_action(&mut self, ctx: &egui::Context, ed: &mut ScriptEditor, edit_id: egui::Id, action: BarAction) {
        let o = self.script_find.clone();
        let (from, to) = selection(ctx, edit_id);
        // The Return that did this (in a field, or on a button) isn't typing.
        ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let show = |ed: &mut ScriptEditor, range: (usize, usize)| {
            select(ctx, edit_id, range);
            ed.scroll_to = Some(range.0);
        };
        ed.notice = None;
        match action {
            BarAction::Close => {
                ed.bar = None;
                ctx.memory_mut(|m| m.request_focus(edit_id));
            }
            BarAction::Find => match script_find::find(&ed.text, to, &o) {
                Some(found) => show(ed, found),
                None if o.find.is_empty() => {}
                None => ed.notice = Some("Not found".into()),
            },
            BarAction::Replace => {
                // A selection that's already a match is the one replaced;
                // otherwise the next one after it.
                let selected_match = script_find::find(&ed.text, from, &FindOptions { wraparound: false, ..o.clone() })
                    == Some((from, to));
                let start = if selected_match { from } else { to };
                match script_find::replace_next(&ed.text, start, &o) {
                    Some((text, replaced)) => {
                        ed.text = text;
                        show(ed, replaced);
                    }
                    None if o.find.is_empty() => {}
                    None => ed.notice = Some("Not found".into()),
                }
            }
            BarAction::ReplaceAll => {
                let (text, count) = script_find::replace_all(&ed.text, &o);
                if count > 0 {
                    ed.text = text;
                    let end = ed.text.chars().count();
                    show(ed, (from.min(end), from.min(end)));
                }
                ed.notice = Some(match count {
                    0 => "Not found".to_string(),
                    1 => "Replaced 1".to_string(),
                    n => format!("Replaced {n}"),
                });
            }
            BarAction::GoToLine => {
                let Some(Bar::Line(line)) = &ed.bar else { return };
                match line.trim().parse::<usize>() {
                    Ok(n) if n >= 1 && n <= ed.text.split('\n').count() => {
                        ed.bar = None;
                        let range = script_find::line_range(&ed.text, n);
                        show(ed, range);
                    }
                    _ => ed.notice = Some(format!("Lines go from 1 to {}", ed.text.split('\n').count())),
                }
            }
        }
    }

    fn save_script(&mut self, ed: &mut ScriptEditor) {
        let mut w = lock(&self.rt.world);
        if script_of(&w, ed.obj).as_deref() != Some(ed.text.as_str()) {
            set_script(&mut w, ed.obj, ed.text.clone());
        }
        ed.error = if ed.text.trim().is_empty() {
            None
        } else {
            crate::hypertalk::parser::parse_script(&ed.text).err()
        };
    }

    /// Windows scripts opened (HyperCard 2's external windows): text
    /// windows (TEXWindow), which the user edits and closing one tells the
    /// card, and progress windows (ProgressWindoid), which scripts move and
    /// fill.
    pub fn script_windows(&mut self, ctx: &egui::Context) {
        let windows = lock(&self.rt.world).script_windows.clone();
        for t in windows.into_iter().filter(|t| t.visible) {
            let at = self.card_origin + egui::vec2(t.rect.left as f32, t.rect.top as f32);
            let size = t.size as f32;
            let fixed = ["courier", "monaco"].contains(&t.font.to_lowercase().as_str());
            let font = if fixed {
                egui::FontId::monospace(size)
            } else {
                egui::FontId::proportional(size)
            };
            match t.kind {
                WindowKind::Progress(fraction) => {
                    // A windoid: the script places it and sets its fraction.
                    egui::Window::new(&t.name)
                        .id(egui::Id::unique(("script-window", t.name.as_str())))
                        .current_pos(at)
                        .fixed_size(egui::vec2(t.rect.width().max(140) as f32 - 12.0, 0.0))
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.label(RichText::new(&t.text).font(font));
                            ui.add(egui::ProgressBar::new(fraction as f32));
                        });
                }
                WindowKind::Palette { hilited, .. } => self.palette_window(ctx, &t, at, hilited),
                WindowKind::Mouse | WindowKind::Info => self.live_window(ctx, &t, at, font),
                WindowKind::Text => {
                    let mut open = true;
                    let mut text = t.text.clone();
                    let window_size = egui::vec2(
                        t.rect.width().max(120) as f32,
                        t.rect.height().max(60) as f32,
                    );
                    egui::Window::new(&t.name)
                        .id(egui::Id::unique(("script-window", t.name.as_str())))
                        .default_pos(at)
                        .default_size(window_size)
                        .open(&mut open)
                        .show(ctx, |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink(false)
                                .show(ui, |ui| {
                                    ui.add_sized(
                                        ui.available_size(),
                                        egui::TextEdit::multiline(&mut text)
                                            .font(font)
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                        });
                    // A locked window can be read and copied from, not changed.
                    if text != t.text && !t.locked {
                        let mut w = lock(&self.rt.world);
                        if let Some(tw) = w.script_windows.iter_mut().find(|x| x.name == t.name) {
                            tw.text = text;
                            tw.dirty = true;
                        }
                    }
                    if !open {
                        self.send(Event::CloseWindow(t.name.clone()));
                    }
                }
            }
        }
    }

    /// Mousoid's and Infoid's windoids: the mouse's position, or what's
    /// under it, kept up to date. They can be dragged and closed.
    fn live_window(&mut self, ctx: &egui::Context, t: &crate::runtime::world::ScriptWindow, at: egui::Pos2, font: egui::FontId) {
        let text = {
            let w = lock(&self.rt.world);
            let (x, y) = (w.mouse.x, w.mouse.y);
            if t.kind == WindowKind::Mouse {
                let (wx, wy) = w.window_origin;
                format!(
                    "Card: {x},{y}\nScreen: {},{}\nButton: {}",
                    x + wx,
                    y + wy,
                    if w.mouse.down { "down" } else { "up" }
                )
            } else {
                live_info(&w, x, y)
            }
        };
        let mut open = true;
        let resp = egui::Window::new(&t.name)
            .id(egui::Id::unique(("script-window", t.name.as_str())))
            .fixed_pos(at)
            .fixed_size(egui::vec2(t.rect.width() as f32 - 12.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| ui.label(RichText::new(text).font(font)));
        // It sits where the script put it; dragging its title bar moves it.
        if let Some(r) = resp {
            let rect = r.response.rect;
            let title = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() - 28.0, 26.0));
            let (pressed, down, delta, at_press) = ctx.input(|i| {
                (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.delta(), i.pointer.press_origin())
            });
            // Moves count from the frame after the press: the press's own
            // delta is how the pointer got there.
            if pressed && at_press.is_some_and(|p| title.contains(p)) {
                self.live_drag = Some(t.name.clone());
            } else if !down {
                self.live_drag = None;
            } else if self.live_drag.as_deref() == Some(t.name.as_str()) && delta != egui::Vec2::ZERO {
                let mut w = lock(&self.rt.world);
                if let Some(win) = w.script_windows.iter_mut().find(|x| x.name == t.name) {
                    win.rect = win.rect.translate(delta.x.round() as i32, delta.y.round() as i32);
                }
            }
        }
        // The pointer keeps moving: keep the numbers fresh.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        if !open {
            self.send(Event::CloseWindow(t.name.clone()));
        }
    }

    /// A palette: a windoid with a striped title bar and close box, showing
    /// its picture. A button inverts while the mouse is down on it and sends
    /// its message when the mouse comes up inside it.
    fn palette_window(&mut self, ctx: &egui::Context, t: &crate::runtime::world::ScriptWindow, at: egui::Pos2, hilited: i32) {
        use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};
        let Some(palette) = t.palette.clone() else { return };
        let fresh = self.palette_tex.get(&t.name).is_none_or(|(p, _, _)| !std::sync::Arc::ptr_eq(p, &palette));
        if fresh {
            let normal = super::render::picture_image(&palette.picture);
            let mut inverted = normal.clone();
            for px in &mut inverted.pixels {
                *px = if *px == Color32::BLACK { Color32::WHITE } else { Color32::BLACK };
            }
            let opts = egui::TextureOptions::NEAREST;
            let n = ctx.load_texture(format!("palette-{}", t.name), normal, opts);
            let i = ctx.load_texture(format!("palette-{}-inv", t.name), inverted, opts);
            self.palette_tex.insert(t.name.clone(), (palette.clone(), n, i));
        }
        let (_, normal, inverted) = self.palette_tex[&t.name].clone();
        const BAR: f32 = 11.0;
        let size = vec2(palette.picture.width as f32, palette.picture.height as f32);
        let id = egui::Id::unique(("palette", t.name.as_str()));
        let mut moved = None;
        let mut close = false;
        let mut clicked = None;
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(at - vec2(1.0, BAR + 1.0))
            .show(ctx, |ui| {
                let (outer, _) = ui.allocate_exact_size(size + vec2(2.0, BAR + 2.0), Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(outer, 0.0, Color32::WHITE);
                painter.rect_stroke(outer, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
                let bar = Rect::from_min_size(outer.min + vec2(1.0, 1.0), vec2(size.x, BAR));
                let line = Stroke::new(1.0, Color32::BLACK);
                for y in (2..BAR as i32 - 1).step_by(2) {
                    let y = bar.top() + y as f32 + 0.5;
                    painter.hline(bar.left() + 1.0..=bar.right() - 1.0, y, line);
                }
                painter.hline(bar.x_range(), bar.bottom() - 0.5, line);
                let close_box = Rect::from_min_size(bar.min + vec2(6.0, 1.0), vec2(9.0, 9.0));
                painter.rect_filled(close_box.expand(1.0), 0.0, Color32::WHITE);
                painter.rect_stroke(close_box, 0.0, line, StrokeKind::Inside);
                let picture = Rect::from_min_size(pos2(bar.left(), bar.bottom()), size);
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.image(normal.id(), picture, uv, Color32::WHITE);

                let bar_resp = ui.interact(bar, id.with("bar"), Sense::click_and_drag());
                if bar_resp.clicked() && bar_resp.interact_pointer_pos().is_some_and(|p| close_box.expand(2.0).contains(p)) {
                    close = true;
                } else if bar_resp.dragged() {
                    moved = Some(bar_resp.drag_delta());
                }
                let resp = ui.interact(picture, id.with("picture"), Sense::click_and_drag());
                let pointer = ctx.input(|i| i.pointer.latest_pos());
                let button_at = |p: egui::Pos2| {
                    let (x, y) = ((p.x - picture.left()) as i32, (p.y - picture.top()) as i32);
                    palette.buttons.iter().position(|b| b.rect.contains(x, y))
                };
                if resp.is_pointer_button_down_on() && ctx.input(|i| i.pointer.primary_pressed()) {
                    self.palette_press = pointer.and_then(button_at).map(|n| (t.name.clone(), n));
                }
                let pressed = self.palette_press.as_ref().filter(|(w, _)| *w == t.name).map(|(_, n)| *n);
                let over = pressed.filter(|n| pointer.and_then(button_at) == Some(*n));
                if ctx.input(|i| i.pointer.primary_released()) && pressed.is_some() {
                    clicked = over;
                    self.palette_press = None;
                }
                // The pressed button, and the one a script highlighted.
                let shown = [over, usize::try_from(hilited - 1).ok()];
                for n in shown.into_iter().flatten() {
                    let Some(b) = palette.buttons.get(n) else { continue };
                    let r = b.rect;
                    let cell = Rect::from_min_max(
                        picture.min + vec2(r.left as f32, r.top as f32),
                        picture.min + vec2(r.right as f32, r.bottom as f32),
                    )
                    .intersect(picture);
                    let uv = Rect::from_min_max(
                        pos2((cell.left() - picture.left()) / size.x, (cell.top() - picture.top()) / size.y),
                        pos2((cell.right() - picture.left()) / size.x, (cell.bottom() - picture.top()) / size.y),
                    );
                    ui.painter().image(inverted.id(), cell, uv, Color32::WHITE);
                }
            });
        if let Some(d) = moved {
            let mut w = lock(&self.rt.world);
            if let Some(win) = w.script_windows.iter_mut().find(|x| x.name == t.name) {
                win.rect = win.rect.translate(d.x.round() as i32, d.y.round() as i32);
            }
        }
        if close {
            self.send(Event::CloseWindow(t.name.clone()));
        } else if let Some(n) = clicked {
            self.send(Event::PaletteButton(t.name.clone(), n));
        }
    }

    pub fn show_dialog(&mut self, ctx: &egui::Context) {
        let dialog = lock(&self.rt.world).dialog.clone();
        let Some(d) = dialog else {
            self.dialog_init = false;
            return;
        };
        if let Dialog::Menu { items, left, top, checked } = &d {
            // Shown as a popup menu, which sends the reply.
            if self.popup.is_none() {
                let at = egui::Pos2::new(*left as f32, *top as f32) + self.card_origin.to_vec2();
                let width = 40.0 + 7.0 * items.iter().map(|i| i.chars().count()).max().unwrap_or(0) as f32;
                self.popup = Some(super::popup::PopupMenu {
                    obj: None,
                    at: egui::Rect::from_min_size(at, egui::vec2(width.max(100.0), 0.0)),
                    items: items.clone(),
                    selected: *checked,
                    dragging: ctx.input(|i| i.pointer.primary_down()),
                });
            }
            return;
        }
        if let Dialog::List(list) = &d {
            if !self.dialog_init {
                self.dialog_init = true;
                self.list = super::list_dialog::ListState::new(list);
            }
            if let Some(r) = self.list_dialog(ctx, list) {
                lock(&self.rt.world).dialog = None;
                self.dialog_init = false;
                let _ = self.rt.dialog_replies.send(r);
            }
            return;
        }
        if !self.dialog_init {
            self.dialog_init = true;
            if let Dialog::Ask { default, .. } = &d {
                self.ask_text = default.clone();
            }
        }
        let mut reply = None;
        egui::Modal::new(egui::Id::unique("script-dialog")).show(ctx, |ui| {
            ui.set_min_width(320.0);
            match &d {
                Dialog::Answer { prompt, buttons } => {
                    ui.label(RichText::new(prompt).size(14.0));
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for (i, b) in buttons.iter().enumerate().rev() {
                            let mut text = RichText::new(b);
                            if i == buttons.len() - 1 {
                                text = text.strong();
                            }
                            if ui.button(text).clicked() {
                                reply = Some(DialogReply::Button(b.clone()));
                            }
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && let Some(b) = buttons.last()
                    {
                        reply = Some(DialogReply::Button(b.clone()));
                    }
                }
                Dialog::Menu { .. } | Dialog::List(_) => {}
                Dialog::Ask { prompt, password, .. } => {
                    ui.label(RichText::new(prompt).size(14.0));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.ask_text).password(*password).desired_width(300.0),
                    );
                    if !resp.has_focus() && !resp.lost_focus() {
                        resp.request_focus();
                    }
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("OK").strong()).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            reply = Some(DialogReply::Text(Some(self.ask_text.clone())));
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            reply = Some(DialogReply::Text(None));
                        }
                    });
                }
            }
        });
        if let Some(r) = reply {
            lock(&self.rt.world).dialog = None;
            self.dialog_init = false;
            let _ = self.rt.dialog_replies.send(r);
        }
    }

    pub fn show_error(&mut self, ctx: &egui::Context) {
        let scripting = self.level() >= 5;
        let Some(alert) = &self.error else { return };
        let (msg, obj, line) = (alert.msg.clone(), alert.obj, alert.line);
        let mut action = None;
        egui::Modal::new(egui::Id::unique("script-error")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new("Script Error").strong().size(15.0));
            ui.add_space(4.0);
            ui.label(&msg);
            if let Some(l) = line {
                ui.label(RichText::new(format!("at line {l}")).weak());
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    action = Some(false);
                }
                if obj.is_some() && scripting && ui.button(RichText::new("Script").strong()).clicked() {
                    action = Some(true);
                }
            });
        });
        match action {
            Some(true) => {
                self.error = None;
                if let Some(o) = obj {
                    self.open_script(o, line);
                }
            }
            Some(false) => self.error = None,
            None => {}
        }
    }

    pub fn send(&self, ev: Event) {
        let _ = self.rt.events.send(ev);
    }
}

fn info_key(t: InfoTarget) -> String {
    match t {
        InfoTarget::Part(o) => format!("{o:?}"),
        InfoTarget::Card(id) => format!("card{id}"),
        InfoTarget::Background(id) => format!("bg{id}"),
        InfoTarget::Stack => "stack".into(),
    }
}

pub fn object_title(w: &World, obj: ObjRef) -> String {
    let s = &w.stack;
    match obj {
        ObjRef::Stack => format!("stack \"{}\"", s.name),
        ObjRef::OtherStack(k) => format!("stack \"{}\"", w.other(k).map(|o| o.name.as_str()).unwrap_or("?")),
        ObjRef::Card(id) => match s.card(id) {
            Some(c) if !c.name.is_empty() => format!("card \"{}\"", c.name),
            _ => format!("card id {id}"),
        },
        ObjRef::Background(id) => match s.background(id) {
            Some(b) if !b.name.is_empty() => format!("bkgnd \"{}\"", b.name),
            _ => format!("bkgnd id {id}"),
        },
        ObjRef::Part { layer, id, card } => match s.part(card, layer, id) {
            Some(p) if !p.name.is_empty() => format!("{} {} \"{}\"", layer.name(), p.kind.name(), p.name),
            Some(p) => format!("{} {} id {}", layer.name(), p.kind.name(), id),
            None => "deleted object".into(),
        },
        ObjRef::MsgBox => "message box".into(),
        ObjRef::HyperCard => "RandomCard".into(),
    }
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Infoid's text: the button or field under the point, or the card.
fn live_info(w: &World, x: i32, y: i32) -> String {
    use crate::model::Layer;
    let card = w.card_id;
    for layer in [Layer::Card, Layer::Background] {
        let parts = w.stack.parts(card, layer);
        if let Some((n, p)) = parts.iter().enumerate().rev().find(|(_, p)| p.visible && p.rect.contains(x, y)) {
            let kind = if p.kind == PartKind::Button { "button" } else { "field" };
            let number = parts[..=n].iter().filter(|q| q.kind == p.kind).count();
            let layer = if layer == Layer::Card { "card" } else { "bkgnd" };
            let r = p.rect;
            return format!(
                "{layer} {kind} {number} \"{}\"\nid {}\nrect {},{},{},{}\nstyle {}",
                p.name,
                p.id,
                r.left,
                r.top,
                r.right,
                r.bottom,
                p.style.name()
            );
        }
    }
    let name = w.stack.card(card).map(|c| c.name.clone()).unwrap_or_default();
    let number = w.stack.cards.iter().position(|c| c.id == card).map_or(0, |i| i + 1);
    format!("card {number} \"{name}\"\nid {card}\nof stack \"{}\"", w.stack.name)
}

/// One of a part's colors in its Info window: a swatch to pick it, and ×
/// to go back to black and white. Returns whether it changed.
fn color_choice(ui: &mut egui::Ui, label: &str, color: &mut Option<crate::model::color::Rgb>, default: crate::model::color::Rgb) -> bool {
    let mut changed = false;
    ui.label(label);
    let mut rgb = color.unwrap_or(default);
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        *color = Some(rgb);
        changed = true;
    }
    if color.is_some() && ui.small_button("×").on_hover_text("Back to black and white").clicked() {
        *color = None;
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::format_script;

    #[test]
    fn formats_scripts() {
        let src = "on mouseUp\nif x then\nbeep\nelse\nrepeat 3\nadd 1 to y\nend repeat\nend if\nend mouseUp";
        let want = "on mouseUp\n  if x then\n    beep\n  else\n    repeat 3\n      add 1 to y\n    end repeat\n  end if\nend mouseUp";
        assert_eq!(format_script(src), want);
    }
}

