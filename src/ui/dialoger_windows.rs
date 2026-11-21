//! Dialoger's dialogs, drawn from the stack's DLOG and DITL: a classic
//! dialog box the size of its DLOG, each item at its place. A modal one
//! sits over everything until a button closes it; a floating one is a
//! windoid. Clicks choose in lists and pop-ups here, then go to the script
//! thread as hits, which turns check boxes and radio buttons.

use eframe::egui::{self, Color32, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};

use super::App;
use crate::runtime::dialoger::Look;
use crate::runtime::world::{ScriptWindow, lock};

/// A windoid's title bar, and a document window's.
const WINDOID_BAR: f32 = 11.0;
const TITLE_BAR: f32 = 19.0;

impl App {
    /// A modal dialog's box and items; the item clicked, if any.
    pub(super) fn dialoger_modal(&mut self, ctx: &egui::Context, name: &str) -> Option<usize> {
        let t = lock(&self.rt.world).script_windows.iter().find(|t| t.name == name).cloned()?;
        let mut hit = None;
        egui::Modal::new(egui::Id::unique(("dialoger", name))).frame(egui::Frame::NONE).show(ctx, |ui| {
            hit = self.dialoger_box(ui, &t, false);
        });
        hit
    }

    /// A floating dialog, where its DLOG puts it on the card window.
    pub(super) fn dialoger_floating(&mut self, ctx: &egui::Context, t: &ScriptWindow, at: egui::Pos2) -> Option<usize> {
        if !t.dialoger.as_ref().is_some_and(|d| d.floating) {
            return None;
        }
        let mut hit = None;
        let id = egui::Id::unique(("dialoger-float", t.name.as_str()));
        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(at - vec2(1.0, WINDOID_BAR + 1.0)).show(ctx, |ui| {
            hit = self.dialoger_box(ui, t, true);
        });
        hit
    }

    /// The dialog itself: its frame, then its items.
    fn dialoger_box(&mut self, ui: &mut egui::Ui, t: &ScriptWindow, floating: bool) -> Option<usize> {
        let mut d = t.dialoger.as_deref()?.clone();
        let before = format!("{d:?}");
        let size = vec2(t.rect.width().max(20) as f32, t.rect.height().max(20) as f32);
        // A modal dialog's frame is double, or a document window's title
        // bar; a windoid has a thin title bar.
        let titled = !floating && (0..16).contains(&d.proc_id) && d.proc_id % 4 == 0;
        let (border, bar) = if floating {
            (1.0, WINDOID_BAR)
        } else if titled {
            (1.0, TITLE_BAR)
        } else {
            (4.0, 0.0)
        };
        let (outer, _) = ui.allocate_exact_size(size + vec2(2.0 * border, 2.0 * border + bar), Sense::hover());
        let painter = ui.painter().clone();
        let line = Stroke::new(1.0, Color32::BLACK);
        painter.rect_filled(outer, 0.0, Color32::WHITE);
        painter.rect_stroke(outer, 0.0, line, StrokeKind::Inside);
        let mut hit = None;
        if bar == 0.0 {
            painter.rect_stroke(outer.shrink(2.0), 0.0, Stroke::new(2.0, Color32::BLACK), StrokeKind::Inside);
        } else {
            let bar_rect = Rect::from_min_size(outer.min + vec2(1.0, 1.0), vec2(outer.width() - 2.0, bar));
            for y in (3..bar as i32 - 2).step_by(2) {
                painter.hline(bar_rect.left() + 1.0..=bar_rect.right() - 1.0, bar_rect.top() + y as f32 + 0.5, line);
            }
            painter.hline(bar_rect.x_range(), bar_rect.bottom() - 0.5, line);
            if titled {
                let galley = painter.layout_no_wrap(d.title.clone(), egui::FontId::proportional(12.0), Color32::BLACK);
                let strip = Rect::from_center_size(bar_rect.center(), vec2(galley.size().x + 12.0, bar - 2.0));
                painter.rect_filled(strip, 0.0, Color32::WHITE);
                painter.galley(strip.center() - galley.size() / 2.0, galley, Color32::BLACK);
            }
            // A floating one moves by its bar.
            if floating {
                let drag = ui.interact(bar_rect, ui.scope_id().with("dialoger-bar"), Sense::drag());
                if drag.dragged() {
                    let delta = drag.drag_delta();
                    let mut w = lock(&self.rt.world);
                    if let Some(win) = w.script_windows.iter_mut().find(|x| x.name == t.name) {
                        win.rect = win.rect.translate(delta.x.round() as i32, delta.y.round() as i32);
                    }
                }
            }
            // A floating one's close box closes it (item 0).
            if floating && d.close_box {
                let close = Rect::from_min_size(bar_rect.min + vec2(6.0, 1.0), vec2(9.0, 9.0).min(vec2(bar - 2.0, bar - 2.0)));
                painter.rect_filled(close.expand(1.0), 0.0, Color32::WHITE);
                painter.rect_stroke(close, 0.0, line, StrokeKind::Inside);
                if ui.interact(close.expand(2.0), ui.scope_id().with("dialoger-close"), Sense::click()).clicked() {
                    hit = Some(0);
                }
            }
        }
        let origin = outer.min + vec2(border, border + bar);
        let content = Rect::from_min_size(origin, size);
        let n_items = d.items.len();
        for i in 0..n_items {
            let n = i + 1;
            let item = d.items[i].clone();
            if !item.visible {
                continue;
            }
            let r = Rect::from_min_max(
                origin + vec2(item.rect.left as f32, item.rect.top as f32),
                origin + vec2(item.rect.right as f32, item.rect.bottom as f32),
            );
            // Items parked outside the dialog are its hidden ones.
            if !content.intersects(r) {
                continue;
            }
            let font = item.font.clone().unwrap_or_else(|| ("Chicago".into(), 12, String::new()));
            let font_id = super::render::font_id(&font.0, font.1);
            let rich = |s: &str| {
                let mut t = RichText::new(s).font(font_id.clone()).color(Color32::BLACK);
                if font.2.contains("bold") {
                    t = t.strong();
                }
                if font.2.contains("italic") {
                    t = t.italics();
                }
                if font.2.contains("underline") {
                    t = t.underline();
                }
                t
            };
            match item.look {
                Look::Button => {
                    let button = egui::Button::new(rich(&item.text))
                        .min_size(r.size())
                        .corner_radius(6.0)
                        .wrap_mode(egui::TextWrapMode::Truncate);
                    let resp = ui.put(r, |ui: &mut egui::Ui| ui.add_enabled(item.active, button));
                    if d.default_item == n {
                        painter.rect_stroke(r.expand(4.0), 8.0, Stroke::new(3.0, Color32::BLACK), StrokeKind::Inside);
                    }
                    if resp.clicked() {
                        hit = Some(n);
                    }
                }
                Look::CheckBox => {
                    let mut on = item.on;
                    let resp = ui.put(r, |ui: &mut egui::Ui| ui.add_enabled(item.active, egui::Checkbox::new(&mut on, rich(&item.text))));
                    if resp.clicked() {
                        hit = Some(n);
                    }
                }
                Look::Radio => {
                    let resp = ui.put(r, |ui: &mut egui::Ui| ui.add_enabled(item.active, egui::RadioButton::new(item.on, rich(&item.text))));
                    if resp.clicked() {
                        hit = Some(n);
                    }
                }
                Look::Static => {
                    // Text past its item is cut off, as on the Mac.
                    let text = d.shown_text(&item.text);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
                        ui.set_clip_rect(r.intersect(ui.clip_rect()));
                        ui.add(egui::Label::new(rich(&text)).wrap().selectable(false));
                    });
                }
                Look::Edit => {
                    let mut text = item.text.clone();
                    let multiline = r.height() > 24.0;
                    let edit = if multiline { egui::TextEdit::multiline(&mut text) } else { egui::TextEdit::singleline(&mut text) };
                    let edit = edit.font(font_id.clone()).desired_width(r.width()).interactive(item.active);
                    // The Mac drew a box just outside editable text.
                    painter.rect_stroke(r.expand(3.0), 0.0, line, StrokeKind::Inside);
                    let resp = ui.put(r, edit.frame(egui::Frame::NONE));
                    if item.focus {
                        resp.request_focus();
                        d.items[i].focus = false;
                    }
                    if resp.changed() {
                        d.items[i].text = text;
                    }
                    if resp.gained_focus() && item.enabled {
                        hit = Some(n);
                    }
                }
                Look::Icon | Look::Picture => {
                    if let Some(tex) = self.dialoger_texture(ui.ctx(), item.look == Look::Icon, item.id) {
                        let s = tex.size_vec2();
                        let shown = if item.look == Look::Icon { Rect::from_min_size(r.min, s) } else { r };
                        painter.image(tex.id(), shown, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    // A chosen picture among choices is framed.
                    if item.radio && item.on {
                        painter.rect_stroke(r.expand(2.0), 0.0, Stroke::new(2.0, Color32::BLACK), StrokeKind::Outside);
                    }
                    if item.enabled || item.pict_button {
                        let resp = ui.interact(r, ui.scope_id().with(("dialoger-item", n)), Sense::click());
                        if resp.clicked() && item.active {
                            hit = Some(n);
                        }
                    }
                }
                Look::List => {
                    painter.rect_stroke(r, 0.0, line, StrokeKind::Inside);
                    let (mut chosen, mut double) = (None, false);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(2.0)), |ui| {
                        egui::ScrollArea::vertical().id_salt(("dialoger-list", t.name.as_str(), n)).auto_shrink(false).show(ui, |ui| {
                            // A row each; a chosen one in reverse, as the Mac's
                            // lists showed it.
                            let row_height = font_id.size + 4.0;
                            for (l, text) in item.lines.iter().enumerate() {
                                let sel = item.selected.contains(&l);
                                let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());
                                let ink = if sel { Color32::WHITE } else { Color32::BLACK };
                                if sel {
                                    ui.painter().rect_filled(row, 0.0, Color32::BLACK);
                                }
                                ui.painter().text(row.left_center() + vec2(3.0, 0.0), egui::Align2::LEFT_CENTER, text, font_id.clone(), ink);
                                if !item.active {
                                    continue;
                                }
                                if resp.clicked() {
                                    chosen = Some(l);
                                }
                                if resp.double_clicked() {
                                    chosen = Some(l);
                                    double = true;
                                }
                            }
                        });
                    });
                    if let Some(l) = chosen {
                        let it = &mut d.items[i];
                        match it.choose {
                            0 => {}
                            1 => it.selected = vec![l],
                            _ if double => {}
                            _ => {
                                if let Some(p) = it.selected.iter().position(|s| *s == l) {
                                    it.selected.remove(p);
                                } else {
                                    it.selected.push(l);
                                    it.selected.sort();
                                }
                            }
                        }
                        // A double-click can count as another item (SetDblClick).
                        hit = Some(if double && it.double_click > 0 { it.double_click } else { n });
                    }
                }
                Look::Popup => {
                    let mut chosen = None;
                    let shown = item.selected.first().and_then(|s| item.lines.get(*s)).cloned().unwrap_or_default();
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
                        ui.add_enabled_ui(item.active, |ui| {
                            egui::ComboBox::from_id_salt(("dialoger-pop", t.name.as_str(), n))
                                .width(r.width() - 8.0)
                                .selected_text(rich(&shown))
                                .show_ui(ui, |ui| {
                                    for (l, text) in item.lines.iter().enumerate() {
                                        if ui.selectable_label(item.selected.first() == Some(&l), rich(text)).clicked() {
                                            chosen = Some(l);
                                        }
                                    }
                                });
                        });
                    });
                    if let Some(l) = chosen {
                        d.items[i].selected = vec![l];
                        hit = Some(n);
                    }
                }
                Look::Box => {
                    let color = match item.pattern.to_ascii_lowercase().as_str() {
                        "none" | "white" => None,
                        "gray" | "ltgray" => Some(Color32::GRAY),
                        _ => Some(Color32::BLACK),
                    };
                    if let Some(c) = color {
                        painter.rect_stroke(r, 0.0, Stroke::new(1.0, c), StrokeKind::Inside);
                    }
                }
                Look::User => {}
            }
        }
        // Return chooses the default button and Escape the Cancel one, as
        // on the Mac, unless a field is being typed in.
        if hit.is_none() && !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && d.default_item > 0 {
                hit = Some(d.default_item);
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) && d.cancel_item > 0 {
                hit = Some(d.cancel_item);
            }
        }
        if format!("{d:?}") != before {
            let mut w = lock(&self.rt.world);
            if let Some(win) = w.script_windows.iter_mut().find(|x| x.name == t.name) {
                win.dialoger = Some(Box::new(d));
            }
        }
        hit
    }

    /// An icon's or picture's texture, from the stack.
    fn dialoger_texture(&mut self, ctx: &egui::Context, icon: bool, id: i16) -> Option<egui::TextureHandle> {
        let (stack, picture) = {
            let w = lock(&self.rt.world);
            let picture = if icon {
                w.stack.icons.get(&(id as i32)).map(|i| i.picture.clone())
            } else {
                w.stack.pictures.get(&(id as i32)).map(|p| p.picture.clone())
            };
            (w.stack.name.clone(), picture)
        };
        let key = format!("{stack}:{}:{id}", if icon { "icon" } else { "pict" });
        if let Some(t) = self.dialoger_tex.get(&key) {
            return Some(t.clone());
        }
        let tex = ctx.load_texture(key.clone(), super::render::picture_image(&picture?), egui::TextureOptions::NEAREST);
        self.dialoger_tex.insert(key, tex.clone());
        Some(tex)
    }
}
