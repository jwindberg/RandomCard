//! The paint tools, pattern palette, selections, Paint menu and paint undo.

use eframe::egui::{
    self, Color32, FontFamily, FontId, PointerButton, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle,
    Vec2,
};

use crate::model::{Picture, Pixel};
use crate::paint::{self, Ink, PaintOp, Region};
use crate::runtime::world::Tool;

use super::{App, render, windows};

pub struct PaintStroke {
    start: (i32, i32),
    last: (i32, i32),
    /// The pencil's colour, fixed for the whole stroke.
    color: Pixel,
}

pub enum Marquee {
    Selecting { start: (i32, i32), end: (i32, i32) },
    Lassoing { points: Vec<(i32, i32)> },
    Selected { region: Region },
    /// A selection lifted off the picture and being dragged.
    Moving { region: Region, pixels: Picture, press: (i32, i32), offset: (i32, i32), tex: TextureHandle },
}

pub struct TextEntry {
    pos: (i32, i32),
    text: String,
    focus: bool,
}

fn translated(region: &Region, dx: i32, dy: i32) -> Region {
    let (l, t, r, b) = region.rect;
    Region {
        rect: (l + dx, t + dy, r + dx, b + dy),
        lasso: region.lasso.as_ref().map(|pts| pts.iter().map(|(x, y)| (x + dx, y + dy)).collect()),
    }
}

fn tool_icon(t: Tool) -> &'static str {
    match t {
        Tool::Browse => "☝",
        Tool::Button => "▭",
        Tool::Field => "▤",
        Tool::Select => "⬚",
        Tool::Lasso => "➰",
        Tool::Pencil => "✏",
        Tool::Brush => "🖌",
        Tool::Eraser => "⌫",
        Tool::Line => "╱",
        Tool::Spray => "∴",
        Tool::Rect => "□",
        Tool::RoundRect => "▢",
        Tool::Bucket => "◧",
        Tool::Oval => "○",
        Tool::Curve => "∿",
        Tool::Text => "A",
        Tool::RegularPolygon => "⬡",
        Tool::Polygon => "⬟",
    }
}

/// Tools that paint continuously as the mouse moves.
fn freehand(t: Tool) -> bool {
    matches!(t, Tool::Pencil | Tool::Brush | Tool::Eraser | Tool::Spray | Tool::Curve)
}

impl App {
    // ----- palette -----

    pub(super) fn palette(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let (tool, filled, line_size, edit_bg, pattern, brush, sides, patterns) = {
            let w = self.world();
            (
                w.tool,
                w.fill_shapes,
                w.line_size,
                w.edit_bg,
                w.pattern,
                w.brush,
                w.poly_sides,
                w.stack.patterns.clone(),
            )
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(2.0);
            ui.label(RichText::new("Tools").strong());
            egui::Grid::new("tools").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                for (i, t) in Tool::ALL.iter().enumerate() {
                    let icon = RichText::new(tool_icon(*t)).size(16.0);
                    let resp = ui
                        .add_sized([36.0, 26.0], egui::Button::selectable(tool == *t, icon))
                        .on_hover_text(windows::capitalize(t.name()));
                    if resp.clicked() {
                        self.choose_tool(*t);
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            ui.separator();
            ui.label(RichText::new("Pattern").strong());
            let textures = self.pattern_textures(&ctx, &patterns);
            egui::Grid::new("patterns").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                for (i, tex) in textures.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(20.0, 14.0), Sense::click());
                    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(20.0 / 8.0, 14.0 / 8.0));
                    ui.painter().image(*tex, rect, uv, Color32::WHITE);
                    let (width, color) =
                        if pattern == i + 1 { (2.0, Color32::from_rgb(0, 90, 200)) } else { (1.0, Color32::GRAY) };
                    ui.painter().rect_stroke(rect, 0.0, Stroke::new(width, color), StrokeKind::Outside);
                    if resp.on_hover_text(format!("Pattern {}", i + 1)).clicked() {
                        self.world().pattern = i + 1;
                    }
                    if i % 5 == 4 {
                        ui.end_row();
                    }
                }
            });
            ui.separator();
            let mut f = filled;
            if ui.checkbox(&mut f, "Filled").changed() {
                self.world().fill_shapes = f;
            }
            egui::Grid::new("paint-settings").num_columns(2).show(ui, |ui| {
                ui.label("Line");
                let mut ls = line_size;
                if ui.add(egui::DragValue::new(&mut ls).range(1..=8)).changed() {
                    self.world().line_size = ls;
                }
                ui.end_row();
                ui.label("Sides");
                let mut s = sides;
                if ui.add(egui::DragValue::new(&mut s).range(3..=50)).on_hover_text("Regular polygon sides").changed() {
                    self.world().poly_sides = s;
                }
                ui.end_row();
            });
            let brush_index = ((brush.max(1) - 1) as usize) % paint::BRUSH_NAMES.len();
            egui::ComboBox::from_id_salt("brush").selected_text(paint::BRUSH_NAMES[brush_index]).width(110.0).show_ui(
                ui,
                |ui| {
                    for (i, name) in paint::BRUSH_NAMES.iter().enumerate() {
                        if ui.selectable_label(i == brush_index, *name).clicked() {
                            self.world().brush = i as u32 + 1;
                        }
                    }
                },
            );
            ui.separator();
            let mut bg = edit_bg;
            if ui.checkbox(&mut bg, "Background").changed() {
                self.do_menu("Background");
            }
        });
    }

    /// Repeating 8×8 textures for pattern swatches, rebuilt when the stack's
    /// patterns change.
    fn pattern_textures(&mut self, ctx: &egui::Context, patterns: &[[u8; 8]]) -> Vec<egui::TextureId> {
        if self.pattern_textures.as_ref().is_none_or(|(cached, _)| cached != patterns) {
            let textures = patterns
                .iter()
                .enumerate()
                .map(|(i, rows)| {
                    let mut img = egui::ColorImage::filled([8, 8], Color32::WHITE);
                    for (y, row) in rows.iter().enumerate() {
                        for x in 0..8 {
                            if row & (0x80 >> x) != 0 {
                                img.pixels[y * 8 + x] = Color32::BLACK;
                            }
                        }
                    }
                    ctx.load_texture(format!("pattern-{i}"), img, egui::TextureOptions::NEAREST_REPEAT)
                })
                .collect();
            self.pattern_textures = Some((patterns.to_vec(), textures));
        }
        self.pattern_textures.as_ref().map(|(_, t)| t.iter().map(|h| h.id()).collect()).unwrap_or_default()
    }

    // ----- pictures and undo -----

    pub(super) fn with_picture<T>(&self, f: impl FnOnce(&mut Picture) -> T) -> Option<T> {
        let mut w = self.world();
        let r = f(w.layer_picture_mut()?);
        w.picture_rev += 1;
        w.dirty = true;
        Some(r)
    }

    /// Remember the current picture before changing it.
    pub(super) fn push_undo(&mut self) {
        self.world().record_picture();
    }

    // ----- tools -----

    pub(super) fn paint_input(&mut self, ctx: &egui::Context, resp: &egui::Response, origin: Pos2, tool: Tool) {
        let to_card = |q: Pos2| ((q.x - origin.x).floor() as i32, (q.y - origin.y).floor() as i32);
        let p = resp.interact_pointer_pos().or(resp.hover_pos()).map(to_card);
        match tool {
            Tool::Select | Tool::Lasso => return self.selection_input(ctx, resp, p, tool),
            Tool::Polygon => return self.polygon_input(resp, p),
            Tool::Text => return self.text_input(resp, p),
            _ => {}
        }
        let Some(p) = p else { return };
        let settings = self.world().paint_settings();
        if resp.drag_started_by(PointerButton::Primary) || resp.clicked() && self.stroke.is_none() {
            let start = resp.interact_pointer_pos().map(to_card).unwrap_or(p);
            self.push_undo();
            if tool == Tool::Bucket {
                self.with_picture(|pic| paint::fill(pic, start.0, start.1, settings.ink()));
                return;
            }
            // The pencil draws white over black, like MacPaint.
            let color = if tool == Tool::Pencil {
                let dark = self.with_picture(|pic| paint::is_dark(pic.get(start.0, start.1))).unwrap_or(false);
                if dark { Pixel::White } else { Pixel::Black }
            } else {
                Pixel::Black
            };
            self.stroke = Some(PaintStroke { start, last: start, color });
            if freehand(tool) {
                self.freehand(tool, start, start, color, &settings);
            }
            if resp.clicked() {
                self.stroke = None;
            }
            return;
        }
        let Some(stroke) = &self.stroke else { return };
        let (start, last, color) = (stroke.start, stroke.last, stroke.color);
        if resp.dragged() {
            // The spray can keeps spraying while held still.
            if freehand(tool) && (p != last || tool == Tool::Spray) {
                self.freehand(tool, last, p, color, &settings);
            }
            if let Some(s) = &mut self.stroke {
                s.last = p;
            }
        }
        if resp.drag_stopped() {
            self.stroke = None;
            if !freehand(tool)
                && let Some(t) = tool.drag_tool()
            {
                self.with_picture(|pic| paint::apply_drag(pic, t, start, last, &settings));
            }
        }
    }

    fn freehand(&self, tool: Tool, a: (i32, i32), b: (i32, i32), color: Pixel, settings: &paint::Settings) {
        self.with_picture(|pic| match tool {
            Tool::Pencil => paint::line(pic, a.0, a.1, b.0, b.1, 1, color),
            _ => {
                if let Some(t) = tool.drag_tool() {
                    paint::apply_drag(pic, t, a, b, settings);
                }
            }
        });
    }

    /// The select and lasso tools: drag out a selection, or drag inside one
    /// to move it.
    fn selection_input(&mut self, ctx: &egui::Context, resp: &egui::Response, p: Option<(i32, i32)>, tool: Tool) {
        let Some(p) = p else { return };
        if resp.drag_started_by(PointerButton::Primary) {
            let inside = matches!(&self.marquee, Some(Marquee::Selected { region }) if region.contains(p.0, p.1));
            if inside {
                let Some(Marquee::Selected { region }) = self.marquee.take() else { return };
                self.push_undo();
                let pixels = self.with_picture(|pic| region.lift(pic)).unwrap_or_default();
                let tex = ctx.load_texture("floating", render::picture_image(&pixels), egui::TextureOptions::NEAREST);
                self.marquee = Some(Marquee::Moving { region, pixels, press: p, offset: (0, 0), tex });
            } else {
                self.commit_marquee();
                self.marquee = Some(if tool == Tool::Lasso {
                    Marquee::Lassoing { points: vec![p] }
                } else {
                    Marquee::Selecting { start: p, end: p }
                });
            }
            return;
        }
        if resp.dragged() {
            match &mut self.marquee {
                Some(Marquee::Selecting { end, .. }) => *end = p,
                Some(Marquee::Lassoing { points }) if points.last() != Some(&p) => points.push(p),
                Some(Marquee::Moving { press, offset, .. }) => *offset = (p.0 - press.0, p.1 - press.1),
                _ => {}
            }
        }
        if resp.drag_stopped() {
            match self.marquee.take() {
                Some(Marquee::Selecting { start, end }) => {
                    let rect = (start.0.min(end.0), start.1.min(end.1), start.0.max(end.0), start.1.max(end.1));
                    if rect.2 - rect.0 > 1 && rect.3 - rect.1 > 1 {
                        self.marquee = Some(Marquee::Selected { region: Region { rect, lasso: None } });
                    }
                }
                Some(Marquee::Lassoing { points }) if points.len() >= 3 => {
                    let l = points.iter().map(|p| p.0).min().unwrap_or(0);
                    let t = points.iter().map(|p| p.1).min().unwrap_or(0);
                    let r = points.iter().map(|p| p.0).max().unwrap_or(0) + 1;
                    let b = points.iter().map(|p| p.1).max().unwrap_or(0) + 1;
                    self.marquee = Some(Marquee::Selected { region: Region { rect: (l, t, r, b), lasso: Some(points) } });
                }
                Some(Marquee::Moving { region, pixels, offset, .. }) => {
                    let (l, t, _, _) = region.rect;
                    self.with_picture(|pic| paint::paste(pic, &pixels, l + offset.0, t + offset.1));
                    self.marquee = Some(Marquee::Selected { region: translated(&region, offset.0, offset.1) });
                }
                Some(Marquee::Lassoing { .. }) => {}
                other => self.marquee = other,
            }
        }
        if resp.clicked() {
            self.commit_marquee();
            self.marquee = None;
        }
    }

    /// Drop a floating selection back onto the picture.
    pub(super) fn commit_marquee(&mut self) {
        if !matches!(self.marquee, Some(Marquee::Moving { .. })) {
            return;
        }
        if let Some(Marquee::Moving { region, pixels, offset, .. }) = self.marquee.take() {
            let (l, t, _, _) = region.rect;
            self.with_picture(|pic| paint::paste(pic, &pixels, l + offset.0, t + offset.1));
            self.marquee = Some(Marquee::Selected { region: translated(&region, offset.0, offset.1) });
        }
    }

    /// The polygon tool: click corners; click the first corner to close the
    /// shape, or double-click to finish an open one.
    fn polygon_input(&mut self, resp: &egui::Response, p: Option<(i32, i32)>) {
        let Some(p) = p else { return };
        if resp.double_clicked() {
            self.finish_polygon(false);
            return;
        }
        if !resp.clicked() {
            return;
        }
        match &mut self.polygon {
            None => self.polygon = Some(vec![p]),
            Some(pts) => {
                let first = pts[0];
                if pts.len() >= 3 && (p.0 - first.0).abs() <= 4 && (p.1 - first.1).abs() <= 4 {
                    self.finish_polygon(true);
                } else {
                    pts.push(p);
                }
            }
        }
    }

    fn finish_polygon(&mut self, closed: bool) {
        let Some(mut pts) = self.polygon.take() else { return };
        pts.dedup();
        if pts.len() < 2 {
            return;
        }
        let s = self.world().paint_settings();
        self.push_undo();
        self.with_picture(|pic| {
            if closed || s.filled {
                paint::polygon(pic, &pts, s.line_size, s.filled.then(|| s.ink()));
            } else {
                for seg in pts.windows(2) {
                    paint::line(pic, seg[0].0, seg[0].1, seg[1].0, seg[1].1, s.line_size, Pixel::Black);
                }
            }
        });
    }

    /// The text tool: click to place text, then type. The text is painted
    /// into the picture when you click elsewhere or change tools.
    fn text_input(&mut self, resp: &egui::Response, p: Option<(i32, i32)>) {
        if resp.clicked()
            && let Some(p) = p
        {
            self.commit_text();
            self.text_entry = Some(TextEntry { pos: p, text: String::new(), focus: true });
        }
    }

    pub(super) fn text_tool_editor(&mut self, ui: &mut egui::Ui, origin: Pos2) {
        let (size, font, card_size) = {
            let w = self.world();
            (w.paint_text_size, w.paint_font.to_ascii_lowercase(), Vec2::new(w.stack.width as f32, w.stack.height as f32))
        };
        let Some(entry) = &mut self.text_entry else { return };
        let family = if font.contains("courier") || font.contains("monaco") || font.contains("mono") {
            FontFamily::Monospace
        } else {
            FontFamily::Proportional
        };
        let min = origin + Vec2::new(entry.pos.0 as f32, entry.pos.1 as f32);
        let rect = Rect::from_min_max(min, origin + card_size);
        let resp = ui.put(
            rect,
            egui::TextEdit::multiline(&mut entry.text)
                .id(egui::Id::unique("paint-text"))
                .frame(egui::Frame::NONE)
                .margin(Vec2::ZERO)
                .font(FontId::new(size as f32, family))
                .text_color(Color32::BLACK)
                .desired_width(rect.width()),
        );
        if entry.focus {
            resp.request_focus();
            entry.focus = false;
        }
    }

    /// Paint the text being typed into the picture.
    pub(super) fn commit_text(&mut self) {
        let Some(entry) = self.text_entry.take() else { return };
        if entry.text.is_empty() {
            return;
        }
        let (size, font, style) = {
            let w = self.world();
            (w.paint_text_size, w.paint_font.clone(), w.paint_text_style.clone())
        };
        self.push_undo();
        self.with_picture(|pic| paint::draw_text(pic, entry.pos.0, entry.pos.1, &entry.text, size, &font, &style));
    }

    // ----- Paint menu -----

    pub(super) fn select_all(&mut self) {
        if self.world().tool != Tool::Select {
            self.choose_tool(Tool::Select);
        }
        let (w, h) = {
            let w = self.world();
            (w.stack.width as i32, w.stack.height as i32)
        };
        self.marquee = Some(Marquee::Selected { region: Region { rect: (0, 0, w, h), lasso: None } });
    }

    /// Apply a Paint menu command to the selection, or the whole picture.
    pub(super) fn apply_paint_op(&mut self, op: PaintOp) {
        self.commit_marquee();
        let selection = match &self.marquee {
            Some(Marquee::Selected { region }) => Some(region.clone()),
            _ => None,
        };
        let ink = Ink::Pattern(self.world().paint_settings().pattern);
        self.push_undo();
        let new_rect = self.with_picture(|pic| {
            let region = selection.clone().unwrap_or_else(|| Region::all(pic));
            paint::apply_op(pic, &region, op, ink)
        });
        if let (Some(rect), Some(region)) = (new_rect, selection)
            && rect != region.rect
        {
            // Rotating changes the selection's shape.
            self.marquee = Some(Marquee::Selected { region: Region { rect, lasso: None } });
        }
    }

    // ----- previews -----

    pub(super) fn paint_overlay(&self, painter: &egui::Painter, origin: Pos2, tool: Tool, ctx: &egui::Context) {
        let at = |x: i32, y: i32| origin + Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
        let stroke = Stroke::new(1.0, Color32::BLACK);
        let dashed = |points: &[Pos2]| {
            for s in egui::Shape::dashed_line(points, stroke, 4.0, 3.0) {
                painter.add(s);
            }
        };
        if let Some(s) = &self.stroke {
            let (a, b) = (at(s.start.0, s.start.1), at(s.last.0, s.last.1));
            let r = Rect::from_two_pos(a, b);
            match tool {
                Tool::Line => {
                    painter.line_segment([a, b], stroke);
                }
                Tool::Rect => {
                    painter.rect_stroke(r, 0.0, stroke, StrokeKind::Middle);
                }
                Tool::RoundRect => {
                    painter.rect_stroke(r, 10.0, stroke, StrokeKind::Middle);
                }
                Tool::Oval => {
                    painter.add(egui::epaint::EllipseShape {
                        center: r.center(),
                        radius: r.size() / 2.0,
                        fill: Color32::TRANSPARENT,
                        stroke,
                        angle: 0.0,
                    });
                }
                Tool::RegularPolygon => {
                    let sides = self.world().poly_sides;
                    let mut pts: Vec<Pos2> =
                        paint::regular_polygon_points(s.start, s.last, sides).iter().map(|p| at(p.0, p.1)).collect();
                    if let Some(first) = pts.first().copied() {
                        pts.push(first);
                    }
                    painter.add(egui::Shape::line(pts, stroke));
                }
                _ => {}
            }
        }
        if let Some(pts) = &self.polygon {
            let mut line: Vec<Pos2> = pts.iter().map(|p| at(p.0, p.1)).collect();
            if let Some(mouse) = ctx.input(|i| i.pointer.latest_pos()) {
                line.push(mouse);
            }
            painter.add(egui::Shape::line(line, stroke));
        }
        match &self.marquee {
            Some(Marquee::Selecting { start, end }) => {
                render::dashed_rect(painter, Rect::from_two_pos(at(start.0, start.1), at(end.0, end.1)), Color32::BLACK);
            }
            Some(Marquee::Lassoing { points }) => {
                dashed(&points.iter().map(|p| at(p.0, p.1)).collect::<Vec<_>>());
            }
            Some(Marquee::Selected { region }) => match &region.lasso {
                Some(points) => {
                    let mut outline: Vec<Pos2> = points.iter().map(|p| at(p.0, p.1)).collect();
                    if let Some(first) = outline.first().copied() {
                        outline.push(first);
                    }
                    dashed(&outline);
                }
                None => {
                    let (l, t, r, b) = region.rect;
                    render::dashed_rect(painter, Rect::from_two_pos(at(l, t), at(r, b)), Color32::BLACK);
                }
            },
            Some(Marquee::Moving { region, offset: (dx, dy), tex, .. }) => {
                let (l, t, r, b) = region.rect;
                let rr = Rect::from_two_pos(at(l + dx, t + dy), at(r + dx, b + dy)).translate(Vec2::splat(-0.5));
                painter.image(tex.id(), rr, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                render::dashed_rect(painter, rr, Color32::BLACK);
            }
            None => {}
        }
    }
}
