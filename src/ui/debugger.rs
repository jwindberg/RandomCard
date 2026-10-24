//! The debugger's windows: the controls shown in the script editor while a
//! script is stopped, the variable watcher and the message watcher.

use eframe::egui::{self, Color32, RichText};

use crate::runtime::debugger::FREQUENT_MESSAGES;
use crate::runtime::world::{DebugCmd, DebugPause, ObjRef, lock, same_script};

use super::App;

impl App {
    pub(super) fn show_message_watcher(&mut self, show: bool) {
        self.message_watcher = show;
        lock(&self.rt.world).watch_messages = show;
    }

    fn send_debug(&self, cmd: DebugCmd) {
        let _ = self.rt.debug.send(cmd);
    }

    /// Stop the running handlers, stopped or tracing, as Command-period did.
    fn abort_scripts(&self) {
        self.rt.abort.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn debug_pause(&self) -> Option<DebugPause> {
        lock(&self.rt.world).debug.clone()
    }

    /// Each frame: bring up the script where a script has stopped, take the
    /// debugger's keys, and draw the watchers.
    pub(super) fn debugger(&mut self, ctx: &egui::Context) {
        match self.debug_pause() {
            Some(p) if p.paused => {
                if self.debug_seen != Some((p.obj, p.line)) {
                    self.debug_seen = Some((p.obj, p.line));
                    self.open_script(p.obj, None);
                }
                let key = |k| ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
                if key(egui::Key::F10) {
                    self.send_debug(DebugCmd::Step);
                } else if key(egui::Key::F11) {
                    self.send_debug(DebugCmd::StepInto);
                } else if key(egui::Key::F5) {
                    self.send_debug(DebugCmd::Go);
                } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F5)) {
                    self.abort_scripts();
                }
            }
            Some(p) => self.debug_seen = Some((p.obj, p.line)),
            None => self.debug_seen = None,
        }
        if self.variable_watcher {
            self.variable_watcher_window(ctx);
        }
        if self.message_watcher {
            self.message_watcher_window(ctx);
        }
    }

    /// The debugger's controls, in the editor of the script that's stopped.
    /// Returns the line it's stopped at.
    pub(super) fn debug_bar(&mut self, ui: &mut egui::Ui, obj: ObjRef) -> Option<usize> {
        let p = self.debug_pause().filter(|p| same_script(p.obj, obj))?;
        ui.horizontal_wrapped(|ui| {
            let paused = p.paused;
            let button = |ui: &mut egui::Ui, label: &str, key: &str| {
                ui.add_enabled(paused, egui::Button::new(label)).on_hover_text(key).clicked()
            };
            if button(ui, "Step", "F10: run this line, stopping at the next one in this handler") {
                self.send_debug(DebugCmd::Step);
            }
            if button(ui, "Step Into", "F11: stop at the next line run, even in a handler this line calls") {
                self.send_debug(DebugCmd::StepInto);
            }
            if button(ui, "Trace", "Run on, showing each line (the traceDelay sets the pace)") {
                self.send_debug(DebugCmd::Trace);
            }
            if button(ui, "Go", "F5: run on to the next checkpoint") {
                self.send_debug(DebugCmd::Go);
            }
            if ui.button("Abort").on_hover_text("Shift+F5: stop all handlers").clicked() {
                self.abort_scripts();
            }
            if ui.button("Variables").clicked() {
                self.variable_watcher = true;
            }
            let state = if paused { "Stopped" } else { "Running" };
            ui.label(
                RichText::new(format!("{state} in {} at line {} · {}", p.handler, p.line, p.calls.join(" → ")))
                    .color(Color32::from_rgb(150, 90, 0)),
            );
        });
        Some(p.line)
    }

    fn variable_watcher_window(&mut self, ctx: &egui::Context) {
        let (globals, pause, busy) = {
            let w = lock(&self.rt.world);
            (w.globals_view.clone(), w.debug.clone(), w.busy)
        };
        let mut open = true;
        egui::Window::new("Variable Watcher").open(&mut open).default_size([320.0, 260.0]).show(ctx, |ui| {
            // Values can change while stopped, or between handlers.
            let editable = pause.as_ref().is_some_and(|p| p.paused) || !busy;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if let Some(p) = pause.as_ref().filter(|p| !p.locals.is_empty()) {
                    ui.label(RichText::new(format!("Locals of {}", p.handler)).strong());
                    self.variable_rows(ui, "locals", &p.locals, editable);
                    ui.add_space(6.0);
                }
                ui.label(RichText::new("Globals").strong());
                if globals.is_empty() {
                    ui.label(RichText::new("No globals yet").weak());
                }
                self.variable_rows(ui, "globals", &globals, editable);
            });
            ui.label(RichText::new("Click a value to change it; Enter sets it.").weak().small());
        });
        if !open {
            self.variable_watcher = false;
        }
    }

    fn variable_rows(&mut self, ui: &mut egui::Ui, grid: &str, vars: &[(String, String)], editable: bool) {
        egui::Grid::new(grid).num_columns(2).striped(true).show(ui, |ui| {
            for (name, value) in vars {
                ui.label(RichText::new(name).monospace());
                match &mut self.var_edit {
                    Some((n, text)) if n == name && editable => {
                        let r = ui.add(egui::TextEdit::singleline(text).desired_width(200.0));
                        r.request_focus();
                        if r.lost_focus() {
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let cmd = DebugCmd::SetVar { name: name.clone(), value: text.clone() };
                                let _ = self.rt.debug.send(cmd);
                            }
                            self.var_edit = None;
                        }
                    }
                    _ => {
                        let shown = if value.len() > 200 { format!("{}…", &value[..value.floor_char_boundary(200)]) } else { value.clone() };
                        let r = ui.add(egui::Label::new(RichText::new(shown.replace('\n', "⏎")).monospace()).sense(egui::Sense::click()));
                        if editable && r.clicked() {
                            self.var_edit = Some((name.clone(), value.clone()));
                        }
                    }
                }
                ui.end_row();
            }
        });
    }

    fn message_watcher_window(&mut self, ctx: &egui::Context) {
        let log: Vec<_> = lock(&self.rt.world).message_log.iter().cloned().collect();
        let mut open = true;
        egui::Window::new("Message Watcher").open(&mut open).default_size([260.0, 320.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.hide_frequent_messages, "Hide idle");
                ui.checkbox(&mut self.hide_unhandled_messages, "Hide unused");
                if ui.button("Clear").clicked() {
                    lock(&self.rt.world).message_log.clear();
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                for m in &log {
                    let frequent = FREQUENT_MESSAGES.iter().any(|f| m.name.eq_ignore_ascii_case(f));
                    if (self.hide_frequent_messages && frequent) || (self.hide_unhandled_messages && !m.handled) {
                        continue;
                    }
                    let text = RichText::new(format!("{}{}", "  ".repeat(m.depth), m.name)).monospace();
                    ui.label(if m.handled { text } else { text.weak() });
                }
            });
        });
        if !open {
            self.show_message_watcher(false);
        }
    }
}
