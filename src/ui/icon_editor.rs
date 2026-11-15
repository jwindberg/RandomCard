//! The icon editor (Edit ▸ Icon…): a stack's icons magnified, to draw
//! pixel by pixel. HyperCard's icons were 32×32 and black and white;
//! RandomCard's are any size and color, so this paints with the Line
//! color, and a right-click makes a pixel clear.

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use crate::model::{Picture, Pixel};
use crate::runtime::world::lock;

use super::{App, windows};

/// The icon editor's window: which icon it shows.
pub struct IconEditor {
    pub icon: Option<i32>,
    /// The last pixel painted in this stroke, so a quick drag joins up.
    last: Option<(i32, i32)>,
}

/// How big the drawing area tries to be, in points.
const AREA: f32 = 384.0;

impl App {
    /// Open the icon editor on the selected button's icon, if it has one.
    pub(super) fn open_icon_editor(&mut self, icon: Option<i32>) {
        let icon = icon.or_else(|| {
            let w = self.world();
            let from_button = self.selected.and_then(|o| match o {
                crate::runtime::world::ObjRef::Part { layer, id, card } => w.stack.part(card, layer, id).map(|p| p.icon),
                _ => None,
            });
            from_button.filter(|i| w.stack.icons.contains_key(i)).or_else(|| w.stack.icons.keys().next().copied())
        });
        self.icon_editor = Some(IconEditor { icon, last: None });
    }

    pub(super) fn icon_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut ed) = self.icon_editor.take() else { return };
        let mut open = true;
        windows::editor_window(ctx, egui::Id::unique("icon editor"), "Icon Editor", [480.0, 600.0], &mut open, |ui| {
            self.icon_editor_ui(ui, &mut ed);
        });
        if open {
            self.icon_editor = Some(ed);
        }
    }

    fn icon_editor_ui(&mut self, ui: &mut egui::Ui, ed: &mut IconEditor) {
        let mut guard = lock(&self.rt.world);
        let w = &mut *guard;
        let ink = w.line_color;
        // Choosing an icon, and making and removing them.
        let label = |id: i32, name: &str| if name.is_empty() { id.to_string() } else { format!("{id} {name}") };
        let icons: Vec<(i32, String)> = w.stack.icons.iter().map(|(i, icon)| (*i, icon.name.clone())).collect();
        if ed.icon.is_some_and(|i| !w.stack.icons.contains_key(&i)) {
            ed.icon = icons.first().map(|(i, _)| *i);
        }
        ui.horizontal(|ui| {
            let shown = ed.icon.and_then(|i| icons.iter().find(|(j, _)| *j == i)).map_or("None".into(), |(i, n)| label(*i, n));
            egui::ComboBox::from_id_salt("icon editor icon").selected_text(shown).show_ui(ui, |ui| {
                for (i, n) in &icons {
                    ui.selectable_value(&mut ed.icon, Some(*i), label(*i, n));
                }
            });
            if ui.button("New").clicked() {
                ed.icon = Some(w.stack.add_icon("Untitled".into(), Picture::new(32, 32)));
                w.dirty = true;
            }
            if let Some(i) = ed.icon {
                if ui.button("Duplicate").clicked()
                    && let Some(icon) = w.stack.icons.get(&i).cloned()
                {
                    ed.icon = Some(w.stack.add_icon(format!("{} copy", icon.name), icon.picture));
                    w.dirty = true;
                }
                if ui.button("Delete").clicked() {
                    w.stack.icons.remove(&i);
                    ed.icon = w.stack.icons.keys().next().copied();
                    w.dirty = true;
                }
            }
        });
        let Some(id) = ed.icon else {
            ui.label("This stack has no icons yet. New makes one.");
            return;
        };
        let Some(icon) = w.stack.icons.get_mut(&id) else { return };
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui.text_edit_singleline(&mut icon.name).changed() {
                w.dirty = true;
            }
        });
        let (pw, ph) = (icon.picture.width.max(1), icon.picture.height.max(1));
        ui.label(RichText::new(format!("{pw} × {ph}. Click or drag to paint with the Line color; right-click makes a pixel clear.")).weak());
        // The icon magnified to fit, whole pixels at a time.
        let zoom = (AREA / pw.max(ph) as f32).floor().clamp(2.0, 16.0);
        let size = Vec2::new(pw as f32 * zoom, ph as f32 * zoom);
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let origin = resp.rect.min;
        // Clear pixels show as a checkerboard.
        for y in 0..ph as i32 {
            for x in 0..pw as i32 {
                let r = Rect::from_min_size(origin + Vec2::new(x as f32 * zoom, y as f32 * zoom), Vec2::splat(zoom));
                let px = icon.picture.get(x, y);
                let color = if px.is_clear() {
                    if (x + y) % 2 == 0 { Color32::from_gray(235) } else { Color32::from_gray(210) }
                } else {
                    let [r, g, b, a] = px.0;
                    Color32::from_rgba_unmultiplied(r, g, b, a)
                };
                painter.rect_filled(r, 0.0, color);
            }
        }
        if zoom >= 6.0 {
            let grid = Stroke::new(1.0, Color32::from_gray(190));
            for x in 0..=pw {
                let at = origin.x + x as f32 * zoom;
                painter.line_segment([Pos2::new(at, origin.y), Pos2::new(at, origin.y + size.y)], grid);
            }
            for y in 0..=ph {
                let at = origin.y + y as f32 * zoom;
                painter.line_segment([Pos2::new(origin.x, at), Pos2::new(origin.x + size.x, at)], grid);
            }
        }
        // Painting: a line from the last pixel, so a quick drag joins up.
        let (primary, secondary, pos) = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down(), i.pointer.latest_pos()));
        let held = (primary || secondary) && (resp.is_pointer_button_down_on() || ed.last.is_some());
        match pos.filter(|_| held) {
            Some(p) => {
                let at = (((p.x - origin.x) / zoom).floor() as i32, ((p.y - origin.y) / zoom).floor() as i32);
                let px = if secondary { Pixel::CLEAR } else { ink };
                let from = ed.last.unwrap_or(at);
                let steps = (at.0 - from.0).abs().max((at.1 - from.1).abs()).max(1);
                for k in 0..=steps {
                    let x = from.0 + (at.0 - from.0) * k / steps;
                    let y = from.1 + (at.1 - from.1) * k / steps;
                    let inside = (0..pw as i32).contains(&x) && (0..ph as i32).contains(&y);
                    if inside && icon.picture.get(x, y) != px {
                        icon.picture.set(x, y, px);
                        w.dirty = true;
                    }
                }
                ed.last = Some(at);
            }
            None => ed.last = None,
        }
    }
}
