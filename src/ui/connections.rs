//! The windows for a stack's connections. Stack Info ▸ Connections… is the
//! author's: which services the stack talks to and what each user gives
//! for them. File ▸ Connections… is the user's: their own values, saved on
//! their computer and never in the stack. The same setup form opens by
//! itself the first time a script uses a connection that isn't set up.

use eframe::egui::{self, RichText};

use super::App;
use crate::model::connections::{ADDRESS, Connection, Setting};
use crate::runtime::connections::{self, store};
use crate::runtime::world::{Dialog, DialogReply};

/// What the user is typing for one connection, before Save.
pub(super) struct SetupForm {
    connection: String,
    /// One for each setting, in order. A secret's box starts empty.
    values: Vec<String>,
    /// Which secrets are already kept, shown as "kept".
    kept: Vec<bool>,
    /// What happened on the last Save or file read.
    notes: Vec<String>,
    /// The cursor goes to the first box still needed, once.
    focus: bool,
}

impl SetupForm {
    fn new(uid: &str, conn: &Connection) -> SetupForm {
        let mut st = store();
        let values = conn.settings.iter().map(|s| if s.secret { String::new() } else { st.given(uid, conn, s) }).collect();
        let kept = conn.settings.iter().map(|s| s.secret && st.is_set(uid, conn, s)).collect();
        SetupForm { connection: conn.name.clone(), values, kept, notes: Vec::new(), focus: true }
    }

    /// Save what's typed. A secret's empty box keeps what's kept. Returns
    /// whether the connection is now set up.
    fn save(&mut self, uid: &str, conn: &Connection) -> bool {
        let mut st = store();
        self.notes.clear();
        for (i, s) in conn.settings.iter().enumerate() {
            let value = self.values.get(i).cloned().unwrap_or_default();
            if s.secret && value.is_empty() {
                continue;
            }
            if let Err(why) = st.set(uid, conn, s, value.trim()) {
                self.notes.push(why);
            }
        }
        // Secrets aren't shown again once saved.
        for (i, s) in conn.settings.iter().enumerate() {
            if s.secret {
                self.values[i].clear();
                self.kept[i] = st.is_set(uid, conn, s);
            }
        }
        let missing = st.missing(uid, conn);
        if !missing.is_empty() {
            self.notes.push(format!("Still needed: {}", missing.join(", ")));
        }
        missing.is_empty()
    }

    /// The boxes, one row for each setting.
    fn show(&mut self, ui: &mut egui::Ui, uid: &str, conn: &Connection) {
        if !conn.description.is_empty() {
            ui.label(&conn.description);
            ui.add_space(6.0);
        }
        let mut forget = None;
        egui::Grid::new(("connection-setup", &conn.name)).num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            for (i, s) in conn.settings.iter().enumerate() {
                if i >= self.values.len() {
                    break;
                }
                ui.label(format!("{}:", s.name));
                ui.vertical(|ui| {
                    let hint = if s.secret && self.kept[i] {
                        "kept".to_string()
                    } else if !s.default.is_empty() {
                        s.default.clone()
                    } else {
                        String::new()
                    };
                    ui.horizontal(|ui| {
                        let needed = self.values[i].is_empty() && !self.kept[i] && s.default.is_empty();
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.values[i])
                                .password(s.secret)
                                .hint_text(hint)
                                .desired_width(260.0),
                        );
                        if self.focus && needed {
                            resp.request_focus();
                            self.focus = false;
                        }
                        if s.secret
                            && self.kept[i]
                            && ui.small_button("Forget").on_hover_text("Remove it from this computer").clicked()
                        {
                            forget = Some(i);
                        }
                    });
                    if !s.hint.is_empty() {
                        ui.label(RichText::new(&s.hint).weak().small());
                    }
                });
                ui.end_row();
            }
        });
        if let Some(i) = forget {
            let mut st = store();
            if let Err(why) = st.set(uid, conn, &conn.settings[i], "") {
                self.notes = vec![why];
            }
            self.kept[i] = st.is_set(uid, conn, &conn.settings[i]);
        }
        for n in &self.notes {
            ui.label(RichText::new(n).color(ui.visuals().warn_fg_color));
        }
        ui.label(
            RichText::new("These are kept on this computer, not in the stack. Secrets go in the system's secure store.")
                .weak()
                .small(),
        );
    }
}

/// File ▸ Connections…: which connection is shown.
pub(super) struct SetupWindow {
    chosen: usize,
    form: Option<SetupForm>,
}

/// Stack Info ▸ Connections…: which connection is shown.
pub(super) struct DeclareWindow {
    chosen: usize,
}

/// A connection name the stack doesn't have yet.
fn new_name(conns: &[Connection]) -> String {
    (1..).map(|n| format!("Connection {n}")).find(|n| !conns.iter().any(|c| c.name.eq_ignore_ascii_case(n))).unwrap_or_default()
}

/// What the windows need of the stack, copied so the world isn't held.
struct Header {
    uid: String,
    name: String,
    connections: Vec<Connection>,
}

impl App {
    fn header(&self) -> Header {
        let w = self.world();
        Header { uid: w.stack.uid.clone(), name: w.stack.name.clone(), connections: w.stack.connections.clone() }
    }

    /// Read a settings file into the stack's connections, saying what it did.
    fn read_settings_file(&self) -> Option<Vec<String>> {
        let path = rfd::FileDialog::new()
            .set_title("Open a settings file")
            .add_filter("Settings file", &["rcsettings", "json"])
            .pick_file()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return Some(vec![format!("Can't read {}: {e}", path.display())]),
        };
        let w = self.world();
        Some(match connections::import(&mut store(), &w.stack, &text) {
            Ok(r) if r.is_empty() => vec!["The file has no settings for this stack".into()],
            Ok(r) => r,
            Err(why) => vec![why],
        })
    }
}

impl App {
    pub(super) fn open_connection_setup(&mut self) {
        self.connection_setup = Some(SetupWindow { chosen: 0, form: None });
    }

    pub(super) fn open_connection_declare(&mut self) {
        self.connection_declare = Some(DeclareWindow { chosen: 0 });
    }

    /// The setup form, as a script's dialog, the first time a script uses
    /// a connection that isn't set up. Replies Save or Cancel.
    pub(super) fn connection_dialog(&mut self, ctx: &egui::Context, name: &str) {
        let stack = self.header();
        let conn = stack.connections.iter().find(|c| c.name.eq_ignore_ascii_case(name)).cloned();
        let Some(conn) = conn else {
            self.reply_dialog(DialogReply::Button("Cancel".into()));
            return;
        };
        let uid = stack.uid.clone();
        if self.connection_form.as_ref().is_none_or(|f| f.connection != conn.name) {
            self.connection_form = Some(SetupForm::new(&uid, &conn));
        }
        let mut reply = None;
        let mut read_file = false;
        egui::Modal::new(egui::Id::unique("connection-dialog")).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.label(RichText::new(format!("Set up “{}”", conn.name)).size(15.0).strong());
            ui.label(format!("“{}” talks to this service, and needs these from you.", stack.name));
            ui.add_space(8.0);
            let Some(form) = self.connection_form.as_mut() else { return };
            form.show(ui, &uid, &conn);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Open Settings File…").on_hover_text("Fill these in from a file you were given").clicked() {
                    read_file = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if (ui.button(RichText::new("Save").strong()).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        && form.save(&uid, &conn)
                        && form.notes.is_empty()
                    {
                        reply = Some("Save");
                    }
                    if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        reply = Some("Cancel");
                    }
                });
            });
        });
        if read_file && let Some(report) = self.read_settings_file() {
            let mut form = SetupForm::new(&uid, &conn);
            form.notes = report;
            self.connection_form = Some(form);
        }
        if let Some(r) = reply {
            self.connection_form = None;
            self.reply_dialog(DialogReply::Button(r.into()));
        }
    }

    fn reply_dialog(&mut self, r: DialogReply) {
        self.world().dialog = None;
        self.dialog_init = false;
        let _ = self.rt.dialog_replies.send(r);
    }

    /// File ▸ Connections…: the user's values for each of the stack's
    /// connections.
    pub(super) fn show_connection_setup(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.connection_setup.take() else { return };
        let stack = self.header();
        let uid = stack.uid.clone();
        let mut open = true;
        let mut read_file = false;
        egui::Window::new("Connections").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.set_min_width(400.0);
            if stack.connections.is_empty() {
                ui.label(format!("“{}” doesn't talk to any services.", stack.name));
                return;
            }
            win.chosen = win.chosen.min(stack.connections.len() - 1);
            if stack.connections.len() > 1 {
                ui.horizontal_wrapped(|ui| {
                    for (i, c) in stack.connections.iter().enumerate() {
                        if ui.selectable_label(win.chosen == i, &c.name).clicked() && win.chosen != i {
                            win.chosen = i;
                            win.form = None;
                        }
                    }
                });
                ui.separator();
            }
            let conn = &stack.connections[win.chosen];
            if win.form.as_ref().is_none_or(|f| f.connection != conn.name) {
                win.form = Some(SetupForm::new(&uid, conn));
            }
            let Some(form) = win.form.as_mut() else { return };
            ui.label(RichText::new(&conn.name).strong());
            form.show(ui, &uid, conn);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Open Settings File…").on_hover_text("Fill these in from a file you were given").clicked() {
                    read_file = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("Save").strong()).clicked() && form.save(&uid, conn) && form.notes.is_empty() {
                        form.notes.push("Saved".into());
                    }
                });
            });
        });
        if read_file
            && let Some(report) = self.read_settings_file()
            && let Some(conn) = stack.connections.get(win.chosen)
        {
            let mut form = SetupForm::new(&uid, conn);
            form.notes = report;
            win.form = Some(form);
        }
        if open {
            self.connection_setup = Some(win);
        }
    }

    /// Stack Info ▸ Connections…: the services the stack talks to, and
    /// what each user gives for them. Changes go straight into the stack.
    pub(super) fn show_connection_declare(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.connection_declare.take() else { return };
        let mut open = true;
        let mut changed = false;
        let mut w = self.world();
        let title = format!("Connections of “{}”", w.stack.name);
        // A window of its own, beside Stack Info, as Info windows are.
        let id = egui::Id::unique("connection-declare");
        super::windows::editor_window(ctx, id, &title, [660.0, 440.0], &mut open, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let conns = &mut w.stack.connections;
                ui.horizontal_wrapped(|ui| {
                    for (i, c) in conns.iter().enumerate() {
                        if ui.selectable_label(win.chosen == i, &c.name).clicked() {
                            win.chosen = i;
                        }
                    }
                    if ui.button("New Connection").clicked() {
                        conns.push(Connection::new(&new_name(conns)));
                        win.chosen = conns.len() - 1;
                        changed = true;
                    }
                });
                if conns.is_empty() {
                    ui.label(
                        RichText::new("A connection is a service the stack talks to: each person who uses the stack sets up their own.")
                            .weak(),
                    );
                    return;
                }
                win.chosen = win.chosen.min(conns.len() - 1);
                ui.separator();
                let chosen = win.chosen;
                let others: Vec<String> =
                    conns.iter().enumerate().filter(|(i, _)| *i != chosen).map(|(_, c)| c.name.to_lowercase()).collect();
                let conn = &mut conns[chosen];
                let mut delete = false;
                egui::Grid::new("connection-declare-head").num_columns(2).show(ui, |ui| {
                    ui.label("Name:");
                    let mut name = conn.name.clone();
                    if ui
                        .add(egui::TextEdit::singleline(&mut name).desired_width(240.0))
                        .on_hover_text("How scripts name it: with connection \"VideoDB\"")
                        .changed()
                        && !name.trim().is_empty()
                        && !others.contains(&name.trim().to_lowercase())
                    {
                        conn.name = name;
                        changed = true;
                    }
                    ui.end_row();
                    ui.label("Description:");
                    changed |= ui
                        .add(egui::TextEdit::multiline(&mut conn.description).desired_rows(2).desired_width(400.0))
                        .on_hover_text("Shown to each person who sets it up")
                        .changed();
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.label(RichText::new("What each person gives").strong());
                let mut remove = None;
                egui::Grid::new("connection-declare-settings").num_columns(5).striped(true).show(ui, |ui| {
                    ui.label("Setting");
                    ui.label("Hint");
                    ui.label("Default");
                    ui.label("Secret");
                    ui.label("");
                    ui.end_row();
                    let names: Vec<String> = conn.settings.iter().map(|s| s.name.to_lowercase()).collect();
                    for (i, s) in conn.settings.iter_mut().enumerate() {
                        let address = s.name == ADDRESS && i == 0;
                        if address {
                            ui.label(ADDRESS);
                        } else {
                            let mut name = s.name.clone();
                            if ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut name)).changed()
                                && !name.trim().is_empty()
                                && !names.iter().enumerate().any(|(j, n)| j != i && *n == name.trim().to_lowercase())
                            {
                                s.name = name;
                                changed = true;
                            }
                        }
                        changed |= ui.add_sized([220.0, 20.0], egui::TextEdit::singleline(&mut s.hint)).changed();
                        // A secret's default would be in the stack for anyone.
                        changed |= ui
                            .add_enabled_ui(!s.secret, |ui| ui.add_sized([170.0, 20.0], egui::TextEdit::singleline(&mut s.default)))
                            .inner
                            .changed();
                        if ui.add_enabled(!address, egui::Checkbox::without_text(&mut s.secret)).changed() {
                            if s.secret {
                                s.default.clear();
                            }
                            changed = true;
                        }
                        if !address && ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = remove {
                    conn.settings.remove(i);
                    changed = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("Add Setting").clicked() {
                        let names: Vec<String> = conn.settings.iter().map(|s| s.name.to_lowercase()).collect();
                        let name = (1..)
                            .map(|n| format!("Setting {n}"))
                            .find(|n| !names.contains(&n.to_lowercase()))
                            .unwrap_or_default();
                        conn.settings.push(Setting { name, ..Setting::default() });
                        changed = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Delete Connection").clicked() {
                            delete = true;
                        }
                    });
                });
                ui.label(
                    RichText::new(
                        "Defaults are saved in the stack, so a secret has none. Renaming a connection or a setting \
                         loses what people have set up for it.",
                    )
                    .weak()
                    .small(),
                );
                if delete {
                    conns.remove(chosen);
                    changed = true;
                }
            });
        });
        if changed {
            w.dirty = true;
        }
        drop(w);
        if open {
            self.connection_declare = Some(win);
        }
    }

    /// For the script dialog's match: the setup form when it's a connection.
    pub(super) fn show_connection_dialog(&mut self, ctx: &egui::Context, d: &Dialog) -> bool {
        if let Dialog::Connection(name) = d {
            self.connection_dialog(ctx, name);
            return true;
        }
        false
    }
}
