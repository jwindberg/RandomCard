//! Stack Info ▸ Connections…: the author's window for the services a stack
//! talks to: each connection's name, its type, and for each of its settings
//! whether the author's value is fixed, a default or left open. Users fill
//! in their values through the stack's own cards, which the stack's author
//! designs: RandomCard has no setup form of its own.

use eframe::egui::{self, RichText};

use super::App;
use crate::model::connections::{Connection, Element, Field, Given, Header as SentHeader, Type, reserved};

/// A key ignoring case, as element names are.
fn key(s: &str) -> String {
    s.trim().to_lowercase()
}


/// Stack Info ▸ Connections…: which connection is shown.
pub(super) struct DeclareWindow {
    chosen: usize,
}

/// A connection name the stack doesn't have yet.
fn new_name(conns: &[Connection]) -> String {
    (1..).map(|n| format!("Connection {n}")).find(|n| !conns.iter().any(|c| c.name.eq_ignore_ascii_case(n))).unwrap_or_default()
}

/// Who gives an element's value, and the author's value when it's theirs
/// to give. Returns whether anything changed.
fn given_editor(ui: &mut egui::Ui, conn_name: &str, e: &mut Element, f: &Field) -> bool {
    let mut changed = false;
    if f.secret {
        ui.label("Open").on_hover_text("A secret is each user's own, never in the stack");
        ui.label(RichText::new("Each user's secret").weak());
        return false;
    }
    egui::ComboBox::from_id_salt(("element-given", conn_name, &f.name)).selected_text(e.given.name()).show_ui(ui, |ui| {
        for g in Given::ALL {
            changed |= ui.selectable_value(&mut e.given, g, g.name()).changed();
        }
    });
    let theirs = e.given != Given::Open;
    if f.choices.is_empty() {
        changed |= ui
            .add_enabled_ui(theirs, |ui| ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut e.value)))
            .inner
            .changed();
    } else {
        ui.add_enabled_ui(theirs, |ui| {
            let shown = if e.value.is_empty() { f.fallback().to_string() } else { e.value.clone() };
            egui::ComboBox::from_id_salt(("element-value", conn_name, &f.name)).selected_text(shown).show_ui(ui, |ui| {
                for c in f.choices {
                    if ui.selectable_label(e.value.eq_ignore_ascii_case(c), *c).clicked() {
                        e.value = c.to_string();
                        changed = true;
                    }
                }
            });
        });
    }
    changed
}

/// The rows for a type's elements (or Type and URL): who gives each, the
/// author's value, and a hint of the author's own.
fn type_rows(ui: &mut egui::Ui, conn: &mut Connection, fields: &[Field]) -> bool {
    let mut changed = false;
    for f in fields {
        ui.label(&f.name);
        let name = conn.name.clone();
        let e = conn.element_mut(&f.name);
        changed |= given_editor(ui, &name, e, f);
        changed |= ui
            .add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut e.hint).hint_text(&f.hint))
            .on_hover_text("Shown beside its box")
            .changed();
        ui.label("");
        ui.end_row();
    }
    changed
}

impl App {
    pub(super) fn open_connection_declare(&mut self) {
        self.connection_declare = Some(DeclareWindow { chosen: 0 });
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
        super::windows::editor_window(ctx, id, &title, [720.0, 460.0], &mut open, |ui| {
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
                ui.label(RichText::new("Who gives each setting").strong());
                ui.label(
                    RichText::new(
                        "Fixed: your value, for everyone. Default: your value, which each user may change. \
                         Open: each user gives it. Secrets are always each user's own.",
                    )
                    .weak()
                    .small(),
                );
                // Type and URL; then each type's own, for every type a user
                // may choose.
                let base: Vec<Field> = conn.fields(Type::None);
                let types: Vec<Type> = conn.types().into_iter().filter(|t| *t != Type::None).collect();
                egui::Grid::new(("connection-declare-elements", &conn.name)).num_columns(4).striped(true).show(ui, |ui| {
                    ui.label("Setting");
                    ui.label("Given");
                    ui.label("Your value");
                    ui.label("Hint");
                    ui.end_row();
                    let fixed: Vec<Field> = base.iter().filter(|f| !f.extra).cloned().collect();
                    changed |= type_rows(ui, conn, &fixed);
                    for t in &types {
                        ui.label(RichText::new(t.name()).strong());
                        ui.end_row();
                        let own: Vec<Field> = conn.fields(*t).into_iter().filter(|f| t.elements().iter().any(|d| d.name == f.name)).collect();
                        changed |= type_rows(ui, conn, &own);
                    }
                });
                if conn.fixed_type().is_none() {
                    ui.label(RichText::new("While the type isn't fixed, each user picks one, and gives what it needs.").weak().small());
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Settings of your own").strong());
                let mut remove = None;
                let extras: Vec<usize> = (0..conn.elements.len()).filter(|i| !reserved(&conn.elements[*i].name)).collect();
                if extras.is_empty() {
                    ui.label(RichText::new("None: such as a tenant, or which of a user's systems to show.").weak());
                } else {
                    let names: Vec<String> = conn.elements.iter().map(|e| key(&e.name)).collect();
                    let conn_name = conn.name.clone();
                    egui::Grid::new(("connection-declare-extras", &conn.name)).num_columns(7).striped(true).show(ui, |ui| {
                        ui.label("Setting");
                        ui.label("Given");
                        ui.label("Your value");
                        ui.label("Hint");
                        ui.label("Secret");
                        ui.label("Optional");
                        ui.label("");
                        ui.end_row();
                        for &i in &extras {
                            let e = &mut conn.elements[i];
                            let mut name = e.name.clone();
                            if ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut name)).changed()
                                && !name.trim().is_empty()
                                && !reserved(&name)
                                && !names.iter().enumerate().any(|(j, n)| j != i && *n == key(&name))
                            {
                                e.name = name;
                                changed = true;
                            }
                            let f = Field {
                                name: e.name.clone(),
                                hint: String::new(),
                                secret: e.secret,
                                optional: e.optional,
                                choices: &[],
                                given: e.given,
                                value: e.value.clone(),
                                extra: true,
                            };
                            if f.secret {
                                ui.label("Open");
                                ui.label("");
                            } else {
                                changed |= given_editor(ui, &conn_name, e, &f);
                            }
                            changed |= ui.add_sized([160.0, 20.0], egui::TextEdit::singleline(&mut e.hint)).changed();
                            if ui.checkbox(&mut e.secret, "").changed() {
                                // A secret's value would be in the stack for anyone.
                                if e.secret {
                                    e.value.clear();
                                    e.given = Given::Open;
                                }
                                changed = true;
                            }
                            changed |= ui.checkbox(&mut e.optional, "").on_hover_text("Set up without it").changed();
                            if ui.small_button("Remove").clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                }
                if let Some(i) = remove {
                    conn.elements.remove(i);
                    changed = true;
                }
                if ui.button("Add Setting").clicked() {
                    let name = (1..).map(|n| format!("Setting {n}")).find(|n| conn.element(n).is_none()).unwrap_or_default();
                    conn.elements.push(Element { name, ..Element::default() });
                    changed = true;
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Headers on every request").strong());
                let mut remove = None;
                // Headers carry no security: only settings that aren't secret.
                let plain: Vec<String> =
                    conn.fields(Type::None).into_iter().chain(conn.extras()).filter(|f| !f.secret).map(|f| f.name).collect();
                egui::Grid::new(("connection-declare-headers", &conn.name)).num_columns(4).striped(true).show(ui, |ui| {
                    for (i, h) in conn.headers.iter_mut().enumerate() {
                        changed |= ui
                            .add_sized([140.0, 20.0], egui::TextEdit::singleline(&mut h.name).hint_text("Accept"))
                            .changed();
                        let shown = if h.element.is_empty() { "Fixed value".to_string() } else { format!("Setting “{}”", h.element) };
                        egui::ComboBox::from_id_salt(("header-from", i)).selected_text(shown).show_ui(ui, |ui| {
                            changed |= ui.selectable_value(&mut h.element, String::new(), "Fixed value").changed();
                            for n in &plain {
                                changed |= ui.selectable_value(&mut h.element, n.clone(), format!("Setting “{n}”")).changed();
                            }
                        });
                        changed |= ui
                            .add_enabled_ui(h.element.is_empty(), |ui| {
                                ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut h.value).hint_text("application/json"))
                            })
                            .inner
                            .changed();
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = remove {
                    conn.headers.remove(i);
                    changed = true;
                }
                if ui.button("Add Header").clicked() {
                    conn.headers.push(SentHeader::default());
                    changed = true;
                }
                ui.label(
                    RichText::new(
                        "Your values are saved in the stack, so a secret has none. The type signs each request \
                         under the URL, and nowhere else. Renaming a connection or a setting loses what people \
                         have set up for it.",
                    )
                    .weak()
                    .small(),
                );
                ui.add_space(8.0);
                if ui.button("Delete Connection").clicked() {
                    delete = true;
                }
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
}
