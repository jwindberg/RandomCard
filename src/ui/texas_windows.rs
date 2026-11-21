//! Texas windows (from the Developer Info Assistant): a discussion shows
//! one message at a time, with arrows and a pop-up to move through the
//! thread and a Reply button; a message ready to send has Send and its
//! enclosure; feedback is text to write, with Send. The buttons and the
//! close box go to the script thread, which sends the stack Texas's
//! messages.

use eframe::egui::{self, RichText};

use super::App;
use crate::runtime::texas::{TexasAction, TexasStyle, messages};
use crate::runtime::world::{Event, ScriptWindow, lock};

impl App {
    pub(super) fn texas_window(&mut self, ctx: &egui::Context, t: &ScriptWindow, at: egui::Pos2, font: egui::FontId) {
        let Some(texas) = t.texas.as_ref() else { return };
        let style = texas.style;
        let all = messages(&t.text);
        let shown = texas.shown.min(all.len().saturating_sub(1));
        let msg = all.get(shown).cloned().unwrap_or_default();
        let enclosure = t.texas_property("enclosure").filter(|e| e != "FALSE");
        // "TRUE && name": the file's name.
        let enclosure_name = enclosure.as_deref().map(|e| e.split_whitespace().skip(2).collect::<Vec<_>>().join(" "));
        let titles: Vec<String> =
            all.iter().enumerate().map(|(i, m)| m.title().unwrap_or_else(|| format!("Message {}", i + 1))).collect();
        let mut open = true;
        let (mut action, mut go_to) = (None, None);
        let mut body = msg.body.clone();
        egui::Window::new(&t.name)
            .id(egui::Id::unique(("script-window", t.name.as_str())))
            .default_pos(at)
            .default_size(egui::vec2(t.rect.width().max(200) as f32, t.rect.height().max(120) as f32))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if style == TexasStyle::Disc {
                        if ui.add_enabled(shown > 0, egui::Button::new("◀")).on_hover_text("Previous message").clicked() {
                            go_to = Some(shown - 1);
                        }
                        egui::ComboBox::from_id_salt(("texas-messages", t.name.as_str()))
                            .selected_text(titles.get(shown).cloned().unwrap_or_default())
                            .show_ui(ui, |ui| {
                                for (i, title) in titles.iter().enumerate() {
                                    if ui.selectable_label(i == shown, title).clicked() {
                                        go_to = Some(i);
                                    }
                                }
                            });
                        if ui.add_enabled(shown + 1 < all.len(), egui::Button::new("▶")).on_hover_text("Next message").clicked() {
                            go_to = Some(shown + 1);
                        }
                        ui.separator();
                        if ui.button("Reply").clicked() {
                            action = Some(TexasAction::Reply);
                        }
                        if let Some(name) = &enclosure_name
                            && ui.button(format!("Included File: {name}")).on_hover_text("Unpack the enclosure").clicked()
                        {
                            action = Some(TexasAction::Enclosure);
                        }
                    } else {
                        if ui.button("Send").clicked() {
                            action = Some(TexasAction::Send);
                        }
                        if style == TexasStyle::New {
                            let label = match &enclosure_name {
                                Some(name) => format!("Included File: {name}"),
                                None => "No Enclosure".into(),
                            };
                            if ui.button(label).on_hover_text("Add an enclosure").clicked() {
                                action = Some(TexasAction::Enclosure);
                            }
                        }
                    }
                });
                // The header's lines that say something.
                let header: Vec<(String, String)> = msg
                    .header
                    .iter()
                    .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
                    .filter(|(k, v)| !v.is_empty() && !k.eq_ignore_ascii_case("Included File"))
                    .collect();
                if !header.is_empty() {
                    ui.separator();
                    egui::Grid::new(("texas-header", t.name.as_str())).num_columns(2).spacing([8.0, 1.0]).show(ui, |ui| {
                        for (k, v) in &header {
                            ui.label(RichText::new(format!("{k}:")).small().strong());
                            ui.label(RichText::new(v).small());
                            ui.end_row();
                        }
                    });
                }
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut body).font(font).desired_width(f32::INFINITY),
                    );
                });
            });
        // Only feedback is written in the window; the rest can be read and
        // copied from.
        if body != msg.body && !t.locked {
            let mut w = lock(&self.rt.world);
            if let Some(win) = w.script_windows.iter_mut().find(|x| x.name == t.name) {
                win.set_texas_property("text", &body);
                win.dirty = true;
            }
        }
        if let Some(n) = go_to {
            self.send(Event::TexasShow { name: t.name.clone(), n });
        }
        if let Some(action) = action {
            self.send(Event::Texas { name: t.name.clone(), action });
        }
        if !open {
            self.send(Event::Texas { name: t.name.clone(), action: TexasAction::Close });
        }
    }
}
