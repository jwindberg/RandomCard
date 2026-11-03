//! ListSelect's dialog: a list to choose lines from, under an optional
//! prompt, with up to two buttons. As Frédéric Rinaldi's did, it takes the
//! arrow keys (Shift or Command to extend, Option for the ends), typing to
//! jump to a line, Command-A for all, Return or Enter to choose, Escape or
//! Command-Z to cancel and Tab to leave.

use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};

use super::App;
use crate::runtime::world::{DialogReply, ListDialog, lock};

/// What the open list dialog has chosen so far.
#[derive(Default)]
pub struct ListState {
    selected: Vec<usize>,
    /// Where Shift-click and Shift-arrow ranges start.
    anchor: usize,
    /// Letters typed, to jump to the line they start, and when.
    typed: String,
    typed_at: Option<Instant>,
    scroll_to: Option<usize>,
}

impl ListState {
    pub fn new(d: &ListDialog) -> Self {
        let selected: Vec<usize> = if d.mode == 0 { vec![] } else { d.selected.iter().copied().filter(|&i| i < d.items.len()).collect() };
        let selected = if d.mode == 1 { selected.into_iter().take(1).collect() } else { selected };
        let anchor = selected.first().copied().unwrap_or(0);
        ListState { selected, anchor, scroll_to: Some(anchor), ..Default::default() }
    }

    /// A click on line i, or an arrow key's move to it.
    fn choose(&mut self, d: &ListDialog, i: usize, extend: bool, toggle: bool) {
        match d.mode {
            0 => return,
            2 | 3 if extend => {
                let (a, b) = (self.anchor.min(i), self.anchor.max(i));
                self.selected = (a..=b).collect();
            }
            3 if toggle => {
                if let Some(at) = self.selected.iter().position(|&s| s == i) {
                    self.selected.remove(at);
                } else {
                    self.selected.push(i);
                }
                self.anchor = i;
            }
            _ => {
                self.selected = vec![i];
                self.anchor = i;
            }
        }
        self.scroll_to = Some(i);
    }

    fn reply(&self, exit: &str) -> DialogReply {
        let mut chosen = self.selected.clone();
        chosen.sort_unstable();
        chosen.dedup();
        DialogReply::List(chosen, exit.into())
    }
}

impl App {
    /// Draw the list dialog; Some(reply) once the user leaves it.
    pub(super) fn list_dialog(&mut self, ctx: &egui::Context, d: &ListDialog) -> Option<DialogReply> {
        let font = match &d.font {
            Some((name, size)) => super::render::font_id(name, *size as u32),
            None => egui::FontId::proportional(13.0),
        };
        let row = ctx.fonts_mut(|f| f.row_height(&font)) + 4.0;
        let longest = d.items.iter().map(|i| i.chars().count()).max().unwrap_or(0) as f32;
        let (width, list_height) = match d.size {
            Some((w, h)) => (w as f32 - 24.0, (h as f32 - 70.0).max(row * 3.0)),
            None => ((longest * font.size * 0.6 + 40.0).clamp(200.0, 460.0), row * d.items.len().clamp(3, 12) as f32 + 2.0),
        };
        let mut reply = None;

        // Keys first, so a move shows in this frame.
        let n = d.items.len();
        let (up, down, enter, escape, tab, all, undo, mods, text) = ctx.input(|i| {
            let text: String = i
                .events
                .iter()
                .filter_map(|e| if let egui::Event::Text(t) = e { Some(t.as_str()) } else { None })
                .collect();
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Tab),
                i.modifiers.command && i.key_pressed(egui::Key::A),
                i.modifiers.command && i.key_pressed(egui::Key::Z),
                i.modifiers,
                text,
            )
        });
        let st = &mut self.list;
        if n > 0 && (up || down) {
            let last = st.scroll_to.unwrap_or(st.anchor);
            let to = if mods.alt {
                if up { 0 } else { n - 1 }
            } else if up {
                last.saturating_sub(1)
            } else {
                (last + 1).min(n - 1)
            };
            let extend = mods.shift || mods.command;
            if extend && d.mode >= 2 {
                // Extend from the anchor to the new line.
                let (a, b) = (st.anchor.min(to), st.anchor.max(to));
                st.selected = (a..=b).collect();
                st.scroll_to = Some(to);
            } else {
                st.choose(d, to, false, false);
            }
        }
        if all && d.mode >= 2 {
            st.selected = (0..n).collect();
        }
        if !text.is_empty() && !mods.command {
            if st.typed_at.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
                st.typed.clear();
            }
            st.typed.push_str(&text.to_lowercase());
            st.typed_at = Some(Instant::now());
            if let Some(i) = d.items.iter().position(|item| item.to_lowercase().starts_with(&st.typed)) {
                st.choose(d, i, false, false);
            }
        }
        if enter {
            reply = Some(st.reply("Validated"));
        } else if escape || undo {
            reply = Some(DialogReply::List(vec![], "Cancelled".into()));
        } else if tab {
            reply = Some(st.reply("TabKey"));
        }

        // Where the script put it, or centered.
        let id = egui::Id::unique("list-select");
        let mut area = egui::Modal::default_area(id);
        if let Some((x, y)) = d.at {
            let (wx, wy) = lock(&self.rt.world).window_origin;
            let pos = self.card_origin + egui::vec2((x - wx) as f32, (y - wy) as f32);
            let screen = ctx.content_rect();
            let pos = pos.clamp(screen.min, (screen.max - egui::vec2(width + 24.0, list_height + 70.0)).max(screen.min));
            area = area.anchor(egui::Align2::LEFT_TOP, pos.to_vec2() - screen.min.to_vec2());
        }
        let st = &mut self.list;
        egui::Modal::new(id).area(area).show(ctx, |ui| {
            ui.set_width(width);
            if !d.prompt.is_empty() {
                ui.label(RichText::new(&d.prompt).size(13.0));
                ui.add_space(4.0);
            }
            egui::Frame::new().stroke(ui.visuals().widgets.noninteractive.bg_stroke).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(list_height).auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, item) in d.items.iter().enumerate() {
                        let selected = st.selected.contains(&i);
                        // A row as the Mac drew one: left-aligned, filled when chosen.
                        let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row), egui::Sense::click());
                        let v = ui.visuals();
                        let color = if selected {
                            ui.painter().rect_filled(rect, 0.0, v.selection.bg_fill);
                            v.strong_text_color()
                        } else {
                            v.text_color()
                        };
                        ui.painter().text(rect.left_center() + egui::vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, item, font.clone(), color);
                        if st.scroll_to == Some(i) {
                            r.scroll_to_me(None);
                        }
                        if r.clicked() {
                            let m = ui.input(|i| i.modifiers);
                            st.choose(d, i, m.shift, m.command);
                        }
                        if r.double_clicked() && d.double_click && d.mode != 0 {
                            st.choose(d, i, false, false);
                            reply = Some(st.reply("Validated"));
                        }
                    }
                });
            });
            st.scroll_to = None;
            if !d.ok.is_empty() || !d.cancel.is_empty() {
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !d.ok.is_empty() && ui.button(RichText::new(&d.ok).strong()).clicked() {
                        reply = Some(st.reply("Validated"));
                    }
                    if !d.cancel.is_empty() && ui.button(&d.cancel).clicked() {
                        reply = Some(DialogReply::List(vec![], "Cancelled".into()));
                    }
                });
            }
        });
        reply
    }
}
