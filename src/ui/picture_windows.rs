//! Picture windows, from the `picture` command (HyperCard's Picture XCMD):
//! a picture in a window framed in one of the XCMD's styles. A document
//! or zoom window has a title bar with its name and a close box, a windoid
//! a thin bar like a palette's; the others have only a border, so only a
//! script moves or closes them.

use std::sync::Arc;

use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use super::App;
use crate::runtime::world::{Event, PictureStyle, PictureView, ScriptWindow, lock};

/// A document window's title bar, and a windoid's.
const TITLE_BAR: f32 = 19.0;
const WINDOID_BAR: f32 = 11.0;

impl App {
    pub(super) fn picture_window(&mut self, ctx: &egui::Context, t: &ScriptWindow, at: egui::Pos2, style: PictureStyle, view: PictureView) {
        let Some(image) = t.image.clone() else { return };
        let fresh = self.picture_tex.get(&t.name).is_none_or(|(p, _)| !Arc::ptr_eq(p, &image));
        if fresh {
            let tex = ctx.load_texture(format!("picture-{}", t.name), super::render::picture_image(&image), egui::TextureOptions::NEAREST);
            self.picture_tex.insert(t.name.clone(), (image.clone(), tex));
        }
        let tex = self.picture_tex[&t.name].1.clone();
        let bar = match style {
            PictureStyle::Zoom | PictureStyle::Document => TITLE_BAR,
            PictureStyle::Windoid => WINDOID_BAR,
            _ => 0.0,
        };
        // A dialog's frame is double: a line, a gap, and a heavier line.
        let border = match style {
            PictureStyle::Borderless => 0.0,
            PictureStyle::Dialog => 5.0,
            _ => 1.0,
        };
        let shadow = if style == PictureStyle::Shadow { 2.0 } else { 0.0 };
        let size = vec2(t.rect.width().max(4) as f32, t.rect.height().max(4) as f32);
        let order = if matches!(style, PictureStyle::Windoid | PictureStyle::Shadow | PictureStyle::Rect) {
            egui::Order::Foreground
        } else {
            egui::Order::Middle
        };
        let id = egui::Id::unique(("picture", t.name.as_str()));
        let (mut moved, mut close, mut zoom, mut click) = (None, false, false, None);
        egui::Area::new(id).order(order).fixed_pos(at - vec2(border, bar + border)).show(ctx, |ui| {
            let outer_size = size + vec2(2.0 * border, bar + 2.0 * border);
            let (alloc, _) = ui.allocate_exact_size(outer_size + vec2(shadow, shadow), Sense::hover());
            let outer = Rect::from_min_size(alloc.min, outer_size);
            let painter = ui.painter();
            let line = Stroke::new(1.0, Color32::BLACK);
            if shadow > 0.0 {
                painter.rect_filled(outer.translate(vec2(shadow, shadow)), 0.0, Color32::BLACK);
            }
            let rounding = if style == PictureStyle::RoundRect { 8.0 } else { 0.0 };
            painter.rect_filled(outer, rounding, Color32::WHITE);
            if border > 0.0 {
                painter.rect_stroke(outer, rounding, line, StrokeKind::Inside);
            }
            if style == PictureStyle::Dialog {
                painter.rect_stroke(outer.shrink(3.0), 0.0, Stroke::new(2.0, Color32::BLACK), StrokeKind::Inside);
            }
            let content = Rect::from_min_size(outer.min + vec2(border, border + bar), size);
            // The part of the picture the window shows: from its scroll, at
            // its scale. Past the picture's edge is white.
            let f = view.factor();
            let scroll = pos2(view.scroll.0 as f32, view.scroll.1 as f32);
            let pic = vec2(image.width as f32, image.height as f32);
            let shown = Rect::from_min_size(scroll, size / f).intersect(Rect::from_min_size(egui::Pos2::ZERO, pic));
            let drawn = Rect::from_min_size(content.min, shown.size() * f);
            painter.image(tex.id(), drawn, super::render::pixels_uv(&tex, shown), Color32::WHITE);
            // Clicks, at their point in the picture as if it weren't scaled.
            let resp = ui.interact(content, id.with("picture"), Sense::click_and_drag());
            let to_picture = |p: egui::Pos2| {
                let q = scroll + (p - content.min) / f;
                (q.x.floor() as i32, q.y.floor() as i32)
            };
            if resp.is_pointer_button_down_on() && ctx.input(|i| i.pointer.primary_pressed()) {
                click = resp.interact_pointer_pos().map(|p| (true, to_picture(p)));
            } else if (resp.clicked() || resp.drag_stopped()) && let Some(p) = ctx.input(|i| i.pointer.latest_pos()) {
                click = Some((false, to_picture(p)));
            }
            if bar > 0.0 {
                let bar_rect = Rect::from_min_size(outer.min + vec2(1.0, 1.0), vec2(outer.width() - 2.0, bar));
                for y in (3..bar as i32 - 2).step_by(2) {
                    painter.hline(bar_rect.left() + 1.0..=bar_rect.right() - 1.0, bar_rect.top() + y as f32 + 0.5, line);
                }
                painter.hline(bar_rect.x_range(), bar_rect.bottom() - 0.5, line);
                let box_size = if bar == TITLE_BAR { 11.0 } else { 9.0 };
                let close_box = Rect::from_min_size(
                    bar_rect.min + vec2(8.0, (bar - box_size) / 2.0),
                    vec2(box_size, box_size),
                );
                painter.rect_filled(close_box.expand(1.0), 0.0, Color32::WHITE);
                painter.rect_stroke(close_box, 0.0, line, StrokeKind::Inside);
                if bar == TITLE_BAR {
                    // The name, on a white strip across the stripes.
                    let font = egui::FontId::proportional(12.0);
                    let galley = painter.layout_no_wrap(t.name.clone(), font, Color32::BLACK);
                    let w = galley.size().x.min(bar_rect.width() - 60.0);
                    let strip = Rect::from_center_size(bar_rect.center(), vec2(w + 12.0, bar - 2.0));
                    painter.rect_filled(strip, 0.0, Color32::WHITE);
                    painter.with_clip_rect(strip).galley(strip.center() - galley.size() / 2.0, galley, Color32::BLACK);
                    if style == PictureStyle::Zoom {
                        let zoom_box = Rect::from_min_size(
                            pos2(bar_rect.right() - 8.0 - box_size, close_box.top()),
                            vec2(box_size, box_size),
                        );
                        painter.rect_filled(zoom_box.expand(1.0), 0.0, Color32::WHITE);
                        painter.rect_stroke(zoom_box, 0.0, line, StrokeKind::Inside);
                        painter.rect_stroke(Rect::from_min_size(zoom_box.min, vec2(6.0, 6.0)), 0.0, line, StrokeKind::Inside);
                        let r = ui.interact(zoom_box.expand(2.0), id.with("zoom"), Sense::click());
                        zoom = r.clicked();
                    }
                }
                let resp = ui.interact(bar_rect, id.with("bar"), Sense::click_and_drag());
                if resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| close_box.expand(2.0).contains(p)) {
                    close = true;
                } else if resp.dragged() {
                    moved = Some(resp.drag_delta());
                }
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
        } else if zoom {
            self.send(Event::PictureZoom(t.name.clone()));
        }
        if let Some((down, at)) = click {
            self.send(Event::PictureMouse { name: t.name.clone(), down, at });
        }
    }
}
