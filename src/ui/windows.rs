//! Floating windows: object info dialogs, the script editor, answer/ask
//! dialogs and script error alerts.

use eframe::egui::{self, Color32, RichText};

use crate::model::{Id, PartKind, PartStyle, TextAlign};
use crate::runtime::world::{Dialog, DialogReply, Event, ObjRef, World, lock};

use super::App;

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
        self.editors.push(ScriptEditor { obj, title, text, open: true, error: None, goto_line: line });
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
        for ed in &mut editors {
            self.script_window(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.editors = editors;
    }

    /// Returns an object whose script should be opened.
    fn info_contents(&mut self, ui: &mut egui::Ui, target: InfoTarget) -> Option<ObjRef> {
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
                        ui.label(format!("{} characters", text.chars().count()));
                    }
                    ui.end_row();
                });
                if p.kind == PartKind::Button {
                    ui.label("Contents:");
                    changed |= ui.add(egui::TextEdit::multiline(&mut p.contents).desired_rows(2)).changed();
                }
                ui.separator();
                let r = p.rect;
                ui.label(RichText::new(format!("rect: {},{},{},{}", r.left, r.top, r.right, r.bottom)).weak());
                if ui.button("Script…").clicked() {
                    script = Some(obj);
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
                if ui.button("Script…").clicked() {
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
                if ui.button("Script…").clicked() {
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
                if ui.button("Script…").clicked() {
                    script = Some(ObjRef::Stack);
                }
            }
        }
        if changed {
            w.dirty = true;
        }
        script
    }

    fn script_window(&mut self, ctx: &egui::Context, ed: &mut ScriptEditor) {
        let mut open = ed.open;
        let mut close = false;
        let id = egui::Id::unique(("script", format!("{:?}", ed.obj)));
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
                    ui.label(RichText::new("Ctrl+S saves · Tab indents").weak());
                });
                if let Some(err) = &ed.error {
                    ui.colored_label(Color32::from_rgb(170, 0, 0), err);
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let edit_id = id.with("text");
                    if let Some(line) = ed.goto_line.take() {
                        let start: usize = ed.text.lines().take(line.saturating_sub(1)).map(|l| l.chars().count() + 1).sum();
                        let len = ed.text.lines().nth(line.saturating_sub(1)).map(|l| l.chars().count()).unwrap_or(0);
                        let mut state = egui::text_edit::TextEditState::load(ctx, edit_id).unwrap_or_default();
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(start),
                            egui::text::CCursor::new(start + len),
                        )));
                        state.store(ctx, edit_id);
                        ctx.memory_mut(|m| m.request_focus(edit_id));
                    }
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut ed.text)
                            .id(edit_id)
                            .code_editor()
                            .lock_focus(true)
                            .desired_width(f32::INFINITY)
                            .desired_rows(18),
                    );
                    if resp.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
                        self.save_script(ed);
                    }
                });
            });
        if close {
            open = false;
        } else if !open {
            // Closing the window keeps the edits, as HyperCard's editor did when you said yes.
            self.save_script(ed);
        }
        ed.open = open;
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

    pub fn show_dialog(&mut self, ctx: &egui::Context) {
        let dialog = lock(&self.rt.world).dialog.clone();
        let Some(d) = dialog else {
            self.dialog_init = false;
            return;
        };
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
                if obj.is_some() && ui.button(RichText::new("Script").strong()).clicked() {
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
