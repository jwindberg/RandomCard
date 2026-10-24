//! The Recent dialog (Go ▸ Recent, Ctrl+R): miniatures of the cards
//! visited lately, as HyperCard showed them. Click one to go there.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::runtime::world::{Event, lock};

use super::App;
use super::render::{self, Scene};

/// Miniatures per row, and how wide each is.
const COLUMNS: usize = 7;
const THUMB_WIDTH: f32 = 96.0;

impl App {
    pub(super) fn recent_window(&mut self, ctx: &egui::Context) {
        if !self.recent_open {
            return;
        }
        let (scenes, current, card_size) = {
            let w = lock(&self.rt.world);
            let scenes: Vec<Scene> = w
                .recent
                .iter()
                .rev()
                .filter(|id| w.stack.card(**id).is_some())
                .map(|id| Scene::capture_card(&w, *id, false))
                .collect();
            for s in &scenes {
                self.tex.refresh(ctx, &w, s);
            }
            (scenes, w.card_id, Vec2::new(w.stack.width as f32, w.stack.height as f32))
        };
        let scale = THUMB_WIDTH / card_size.x;
        let thumb = card_size * scale;
        let mut open = true;
        let mut go = None;
        egui::Window::new("Recent")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("recent-cards").spacing([8.0, 8.0]).show(ui, |ui| {
                    for (i, scene) in scenes.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(thumb, Sense::click());
                        // Draw the card at full size on a layer of its own, scaled down.
                        let layer = egui::LayerId::new(ui.layer_id().order, ui.scope_id().with(("recent", i)));
                        ctx.set_sublayer(ui.layer_id(), layer);
                        ctx.set_transform_layer(layer, egui::emath::TSTransform::new(rect.min.to_vec2(), scale));
                        let painter = egui::Painter::new(ctx.clone(), layer, Rect::from_min_size(Pos2::ZERO, card_size));
                        render::draw_scene(&painter, &self.tex, scene, Pos2::ZERO, None, false);
                        let (width, color) = if scene.card == current {
                            (2.0, Color32::BLACK)
                        } else if resp.hovered() {
                            (2.0, Color32::from_gray(120))
                        } else {
                            (1.0, Color32::from_gray(150))
                        };
                        ui.painter().rect_stroke(rect, 0.0, Stroke::new(width, color), StrokeKind::Outside);
                        if resp.clicked() {
                            go = Some(scene.card);
                        }
                        if i % COLUMNS == COLUMNS - 1 {
                            ui.end_row();
                        }
                    }
                });
            });
        if let Some(id) = go {
            self.send(Event::MsgBox(format!("go to card id {id}")));
            open = false;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        self.recent_open = open;
    }
}
