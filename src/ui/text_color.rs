//! Style ▸ Text Color…: a color for the selected text in a field, kept in
//! its style runs, as `set the foreColor of word 2 of field 1` does.

use eframe::egui;

use crate::model::color::Rgb;
use crate::model::styles;
use crate::runtime::world::FieldRange;

use super::App;

pub struct TextColor {
    /// The text being colored, as it was selected when the window opened.
    range: FieldRange,
    color: egui::Color32,
}

impl App {
    pub(super) fn open_text_color(&mut self) {
        let w = self.world();
        let Some(range) = w.current_selection().filter(|r| r.start < r.end).cloned() else { return };
        // Start from the color of the selection's first character.
        let part = w.stack.part(range.card, range.layer, range.field);
        let runs = w.stack.field_styles(range.card, range.layer, range.field);
        let [r, g, b] = styles::style_at(&runs, range.start).color.or(part.and_then(|p| p.fore_color)).unwrap_or([0, 0, 0]);
        drop(w);
        self.text_color = Some(TextColor { range, color: egui::Color32::from_rgb(r, g, b) });
    }

    pub(super) fn text_color_window(&mut self, ctx: &egui::Context) {
        let Some(tc) = &mut self.text_color else { return };
        let (mut open, mut apply, mut done) = (true, None, false);
        egui::Window::new("Text Color").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            egui::color_picker::color_picker_color32(ui, &mut tc.color, egui::color_picker::Alpha::Opaque);
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    apply = Some(Some([tc.color.r(), tc.color.g(), tc.color.b()]));
                }
                if ui.button("Black and White").on_hover_text("The field's own color again").clicked() {
                    apply = Some(None);
                }
                done = ui.button("Done").clicked();
            });
        });
        if let Some(color) = apply {
            let range = tc.range.clone();
            self.color_text(&range, color);
        }
        if !open || done {
            self.text_color = None;
        }
    }

    /// Color the characters of `range`; None gives them the field's color.
    fn color_text(&mut self, r: &FieldRange, color: Option<Rgb>) {
        let mut w = self.world();
        let Some(own) = w.stack.part(r.card, r.layer, r.field).map(|p| p.fore_color) else { return };
        let text = w.stack.field_text(r.card, r.layer, r.field);
        let mut runs = w.stack.field_styles(r.card, r.layer, r.field);
        let len = text.chars().count();
        let color = color.filter(|c| Some(*c) != own);
        styles::apply(&mut runs, r.start.min(len), r.end.min(len), len, |s| s.color = color);
        w.stack.set_field_content(r.card, r.layer, r.field, text, runs);
        w.dirty = true;
    }
}
