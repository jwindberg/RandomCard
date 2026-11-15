//! The paint tools, pattern and color palettes, selections, Paint menu and
//! paint undo.

use eframe::egui::{
    self, Color32, FontFamily, FontId, Key, PointerButton, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle,
    Vec2,
};

use crate::model::{Picture, Pixel};
use crate::paint::{self, PaintOp, Region};
use crate::runtime::world::{PaintDialog, Tool};

use super::{App, render, windows};

/// FatBits magnifies this many times.
pub const FAT_ZOOM: f32 = 8.0;

/// FatBits: the paint shown magnified for pixel-by-pixel work, as in
/// HyperCard. `left` and `top` are the card pixel at the view's top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FatBits {
    pub left: f32,
    pub top: f32,
}

/// The card pixel under a point on the screen, through FatBits if it's on.
pub fn to_card(fat: Option<FatBits>, origin: Pos2, q: Pos2) -> (i32, i32) {
    match fat {
        Some(f) => (
            ((q.x - origin.x) / FAT_ZOOM + f.left.floor()).floor() as i32,
            ((q.y - origin.y) / FAT_ZOOM + f.top.floor()).floor() as i32,
        ),
        None => ((q.x - origin.x).floor() as i32, (q.y - origin.y).floor() as i32),
    }
}

/// Where a card point is on the screen, through FatBits if it's on.
pub fn to_screen(fat: Option<FatBits>, origin: Pos2, x: f32, y: f32) -> Pos2 {
    match fat {
        Some(f) => origin + Vec2::new((x - f.left.floor()) * FAT_ZOOM, (y - f.top.floor()) * FAT_ZOOM),
        None => origin + Vec2::new(x, y),
    }
}

pub struct PaintStroke {
    start: (i32, i32),
    last: (i32, i32),
    /// The pencil's color, fixed for the whole stroke.
    color: Pixel,
    /// Shift with a freehand tool: the way it goes, chosen by the first
    /// move after Shift is pressed, and the point it goes straight from.
    axis: Option<(paint::Axis, (i32, i32))>,
    /// Draw Multiple: where the drag last left a copy of the shape.
    multi_last: (i32, i32),
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

/// Which paint color a swatch or the eyedropper sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSlot {
    Line,
    Fill,
}

/// The palette's swatches, each showing what's chosen and popping up the
/// choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swatch {
    Pattern,
    Color(ColorSlot),
    Setting(PaintDialog),
}

impl Swatch {
    /// In the palette's order: pattern and colors, then line, brush, sides.
    pub const ALL: [Swatch; 6] = [
        Swatch::Pattern,
        Swatch::Color(ColorSlot::Line),
        Swatch::Color(ColorSlot::Fill),
        Swatch::Setting(PaintDialog::LineSize),
        Swatch::Setting(PaintDialog::BrushShape),
        Swatch::Setting(PaintDialog::PolygonSides),
    ];

    fn tip(self) -> &'static str {
        match self {
            Swatch::Pattern => "Pattern: for the brush, spray, bucket and filled shapes",
            Swatch::Color(ColorSlot::Line) => "Line color: lines, borders, the pencil and text",
            Swatch::Color(ColorSlot::Fill) => "Fill color: the pattern's color",
            Swatch::Setting(PaintDialog::LineSize) => "Line size",
            Swatch::Setting(PaintDialog::BrushShape) => "Brush shape",
            Swatch::Setting(PaintDialog::PolygonSides) => "Polygon sides",
        }
    }
}

/// The Options menu's settings that go on and off: item, icon, tip.
const OPTION_TOGGLES: [(&str, &str, &str); 5] = [
    ("Draw Filled", "⬤", "Draw Filled: shapes filled with the pattern"),
    ("Draw Centered", "⊙", "Draw Centered: lines and shapes grow from where you start"),
    ("Draw Multiple", "❐", "Draw Multiple: leave the shape behind as you drag (Option-1 to 9 spaces them)"),
    ("Grid", "#", "Grid: lines, shapes and selections snap to 8 pixels"),
    ("FatBits", "🔍", "FatBits: magnify to paint pixel by pixel"),
];

/// The Paint menu's commands in HyperCard's groups: item (also the tip),
/// icon, and whether RandomCard has it yet.
const PAINT_GROUPS: [&[(&str, &str, bool)]; 4] = [
    &[("Select", "⬚", false), ("Select All", "▣", true)],
    &[
        ("Fill", "▦", true),
        ("Invert", "◑", true),
        ("Pickup", "⊡", false),
        ("Darken", "▼", true),
        ("Lighten", "△", true),
        ("Trace Edges", "◌", true),
        ("Rotate Left", "↺", true),
        ("Rotate Right", "↻", true),
        ("Flip Vertical", "⇅", true),
        ("Flip Horizontal", "⇆", true),
    ],
    &[("Opaque", "■", true), ("Transparent", "□", true)],
    &[("Keep", "⤓", false), ("Revert", "⤒", false)],
];

/// Whether a stack has left one of RandomCard's menu items enabled.
fn menu_item_enabled(w: &crate::runtime::world::World, item: &str) -> bool {
    crate::runtime::menus::find_item(&w.menus, item).is_none_or(|i| i.enabled)
}

/// The recent colors before any are chosen: black, white and a rainbow.
pub const STARTING_COLORS: [Pixel; 10] = [
    Pixel::BLACK,
    Pixel::WHITE,
    Pixel([128, 128, 128, 255]),
    Pixel([220, 40, 40, 255]),
    Pixel([245, 140, 20, 255]),
    Pixel([245, 210, 30, 255]),
    Pixel([40, 160, 60, 255]),
    Pixel([30, 160, 220, 255]),
    Pixel([40, 70, 200, 255]),
    Pixel([140, 60, 180, 255]),
];

const RECENT_COLORS: usize = 10;

pub fn color32(p: Pixel) -> Color32 {
    let [r, g, b, a] = p.0;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn pixel(c: Color32) -> Pixel {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Pixel::rgba(r, g, b, a.max(1))
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

    /// The palette: the tools, then swatches showing the pattern, the line
    /// and fill colors, the line size, the brush and the polygon sides.
    /// Clicking a swatch unfolds its choices below.
    /// Returns the height its contents take.
    pub(super) fn palette(&mut self, ui: &mut egui::Ui) -> f32 {
        let tool = self.world().tool;
        let mut height = 0.0;
        // A scroll bar with its own room, so it doesn't cover the swatches.
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(2.0);
            egui::Grid::new("tools").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                for (i, t) in Tool::ALL.iter().enumerate() {
                    let icon = RichText::new(tool_icon(*t)).size(16.0);
                    let resp = ui
                        .add_sized([36.0, 26.0], egui::Button::selectable(tool == *t, icon))
                        .on_hover_text(windows::capitalize(t.name()));
                    if resp.clicked() {
                        self.choose_tool(*t);
                    }
                    // As in HyperCard: double-click a tool for its setting.
                    if resp.double_clicked() {
                        match t {
                            Tool::Line => self.open_swatch = Some(Swatch::Setting(PaintDialog::LineSize)),
                            Tool::Brush => self.open_swatch = Some(Swatch::Setting(PaintDialog::BrushShape)),
                            Tool::RegularPolygon => self.open_swatch = Some(Swatch::Setting(PaintDialog::PolygonSides)),
                            Tool::Pencil => self.do_menu("FatBits"),
                            Tool::Rect | Tool::RoundRect | Tool::Oval | Tool::Polygon => self.do_menu("Draw Filled"),
                            _ => {}
                        }
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            ui.separator();
            egui::Grid::new("swatches").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                for (i, which) in Swatch::ALL.into_iter().enumerate() {
                    self.swatch(ui, which);
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            // The open swatch's choices unfold below, in the palette (a
            // popup couldn't reach past the palette's own small window).
            if let Some(which) = self.open_swatch {
                ui.separator();
                if self.swatch_choices(ui, which) {
                    self.open_swatch = None;
                }
            }
            ui.separator();
            self.option_toggles(ui);
            ui.separator();
            egui::CollapsingHeader::new("Paint").id_salt("paint-ops").default_open(false).show_unindented(ui, |ui| {
                self.paint_buttons(ui);
            });
            height = ui.min_rect().height();
        });
        height
    }

    /// The Options menu's settings that go on and off, as buttons that show
    /// whether they're on. Each chooses its menu item, so scripts' doMenu
    /// handlers see it, and a menu item a stack disables disables its button.
    fn option_toggles(&mut self, ui: &mut egui::Ui) {
        let (filled, centered, multiple, grid, enabled) = {
            let w = self.world();
            let enabled: Vec<bool> = OPTION_TOGGLES.iter().map(|(item, _, _)| menu_item_enabled(&w, item)).collect();
            (w.fill_shapes, w.centered, w.multiple, w.grid, enabled)
        };
        let fat = self.fat.is_some();
        let on = [filled, centered, multiple, grid, fat];
        egui::Grid::new("option-toggles").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
            for (i, (item, icon, tip)) in OPTION_TOGGLES.iter().enumerate() {
                let button = egui::Button::selectable(on[i], RichText::new(*icon).size(15.0));
                let resp = ui.add_enabled_ui(enabled[i], |ui| ui.add_sized([36.0, 24.0], button)).inner;
                if resp.on_hover_text(*tip).clicked() {
                    self.do_menu(item);
                }
                if i % 3 == 2 {
                    ui.end_row();
                }
            }
        });
    }

    /// The Paint menu's commands, as buttons in HyperCard's groups: on the
    /// selection, or the whole picture when nothing is selected. The ones
    /// RandomCard doesn't have yet are in their places, greyed out.
    fn paint_buttons(&mut self, ui: &mut egui::Ui) {
        let w = self.world();
        let enabled: Vec<Vec<bool>> =
            PAINT_GROUPS.iter().map(|g| g.iter().map(|(item, _, has)| *has && menu_item_enabled(&w, item)).collect()).collect();
        drop(w);
        for (g, group) in PAINT_GROUPS.iter().enumerate() {
            if g > 0 {
                ui.add_space(4.0);
            }
            egui::Grid::new(("paint-buttons", g)).spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                for (i, (item, icon, has)) in group.iter().enumerate() {
                    let button = egui::Button::new(RichText::new(*icon).size(15.0));
                    let resp = ui.add_enabled_ui(enabled[g][i], |ui| ui.add_sized([36.0, 24.0], button)).inner;
                    let tip = if *has { item.to_string() } else { format!("{item}: not yet in RandomCard") };
                    if resp.on_hover_text(tip.clone()).on_disabled_hover_text(tip).clicked() {
                        self.do_menu(item);
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        }
    }

    /// One of the palette's swatches: what's chosen now, drawn. Clicking it
    /// opens or closes its choices.
    fn swatch(&mut self, ui: &mut egui::Ui, which: Swatch) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(36.0, 26.0), Sense::click());
        self.draw_swatch(ui, rect, which);
        let hover = if resp.hovered() { Stroke::new(2.0, Color32::from_rgb(0, 90, 200)) } else { Stroke::new(1.0, Color32::GRAY) };
        ui.painter().rect_stroke(rect, 2.0, hover, StrokeKind::Inside);
        let open = self.open_swatch == Some(which);
        if open {
            ui.painter().rect_stroke(rect, 2.0, Stroke::new(2.0, Color32::from_rgb(0, 90, 200)), StrokeKind::Inside);
        }
        if resp.on_hover_text(which.tip()).clicked() {
            self.open_swatch = if open { None } else { Some(which) };
        }
    }

    fn draw_swatch(&mut self, ui: &mut egui::Ui, rect: Rect, which: Swatch) {
        let (pattern, patterns, line_color, fill_color, line_size, brush, sides) = {
            let w = self.world();
            (w.pattern, w.stack.patterns.clone(), w.line_color, w.fill_color, w.line_size, w.brush, w.poly_sides)
        };
        let painter = ui.painter().with_clip_rect(rect.shrink(1.0));
        painter.rect_filled(rect, 2.0, Color32::WHITE);
        let c = rect.center();
        match which {
            Swatch::Pattern => {
                let textures = self.pattern_textures(ui.ctx(), &patterns, fill_color);
                if let Some(tex) = textures.get(pattern.wrapping_sub(1)) {
                    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(rect.width() / 8.0, rect.height() / 8.0));
                    painter.image(*tex, rect, uv, Color32::WHITE);
                }
            }
            // The line color as a frame, the fill color filling the swatch.
            Swatch::Color(ColorSlot::Line) => {
                painter.rect_stroke(rect.shrink(5.0), 0.0, Stroke::new(4.0, color32(line_color)), StrokeKind::Middle);
            }
            Swatch::Color(ColorSlot::Fill) => {
                painter.rect_filled(rect.shrink(4.0), 0.0, color32(fill_color));
            }
            Swatch::Setting(PaintDialog::LineSize) => {
                painter.rect_filled(Rect::from_center_size(c, Vec2::new(24.0, line_size as f32)), 0.0, Color32::BLACK);
            }
            Swatch::Setting(PaintDialog::BrushShape) => {
                for (dx, dy) in paint::brush_shape(brush) {
                    let at = c + Vec2::new(dx as f32, dy as f32) * 1.5;
                    painter.rect_filled(Rect::from_min_size(at, Vec2::splat(1.5)), 0.0, Color32::BLACK);
                }
            }
            Swatch::Setting(PaintDialog::PolygonSides) => {
                let mut pts: Vec<Pos2> = paint::regular_polygon_points((0, 0), (0, -10), sides)
                    .iter()
                    .map(|p| c + Vec2::new(p.0 as f32, p.1 as f32 + 1.0))
                    .collect();
                if let Some(first) = pts.first().copied() {
                    pts.push(first);
                }
                painter.add(egui::Shape::line(pts, Stroke::new(1.0, Color32::BLACK)));
            }
        }
    }

    /// A swatch's choices, in its popup; true when one was taken.
    fn swatch_choices(&mut self, ui: &mut egui::Ui, which: Swatch) -> bool {
        match which {
            Swatch::Pattern => self.pattern_choices(ui),
            Swatch::Color(slot) => self.color_choices(ui, slot),
            Swatch::Setting(setting) => self.setting_choices(ui, setting),
        }
    }

    fn pattern_choices(&mut self, ui: &mut egui::Ui) -> bool {
        let (pattern, patterns, fill_color) = {
            let w = self.world();
            (w.pattern, w.stack.patterns.clone(), w.fill_color)
        };
        let textures = self.pattern_textures(ui.ctx(), &patterns, fill_color);
        let mut chosen = false;
        egui::Grid::new("pattern-choices").spacing([3.0, 3.0]).min_col_width(0.0).show(ui, |ui| {
            for (i, tex) in textures.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 16.0), Sense::click());
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(26.0 / 8.0, 16.0 / 8.0));
                ui.painter().image(*tex, rect, uv, Color32::WHITE);
                let (width, color) = if pattern == i + 1 || resp.hovered() {
                    (2.0, Color32::from_rgb(0, 90, 200))
                } else {
                    (1.0, Color32::GRAY)
                };
                ui.painter().rect_stroke(rect, 0.0, Stroke::new(width, color), StrokeKind::Outside);
                if resp.on_hover_text(format!("Pattern {}", i + 1)).clicked() {
                    self.world().pattern = i + 1;
                    chosen = true;
                }
                if i % 4 == 3 {
                    ui.end_row();
                }
            }
        });
        chosen
    }

    /// The line or fill color: a picker, the eyedropper, and the recent
    /// colors. The picker stays open while it's dragged.
    fn color_choices(&mut self, ui: &mut egui::Ui, slot: ColorSlot) -> bool {
        let current = {
            let w = self.world();
            if slot == ColorSlot::Line { w.line_color } else { w.fill_color }
        };
        let what = match slot {
            ColorSlot::Line => "Line color: lines, borders, the pencil and text",
            ColorSlot::Fill => "Fill color: the pattern, for the brush, spray, bucket and filled shapes",
        };
        ui.label(RichText::new(what).small());
        ui.spacing_mut().slider_width = 104.0;
        let mut c = color32(current);
        if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
            self.set_paint_color(slot, pixel(c), false);
        }
        let mut chosen = false;
        let recent = self.recent_colors.clone();
        egui::Grid::new("recent-colors").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
            for (i, color) in recent.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(20.0, 14.0), Sense::click());
                ui.painter().rect_filled(rect, 0.0, color32(*color));
                ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
                if resp.clicked() {
                    self.set_paint_color(slot, *color, false);
                    chosen = true;
                }
                if i % 5 == 4 {
                    ui.end_row();
                }
            }
        });
        if ui.button("💧 From the card").on_hover_text("Then click the card to take its color there").clicked() {
            self.eyedropper = Some(slot);
            chosen = true;
        }
        chosen
    }

    /// Set a paint color, remembering it among the recent colors.
    pub(super) fn set_paint_color(&mut self, slot: ColorSlot, color: Pixel, remember: bool) {
        {
            let mut w = self.world();
            match slot {
                ColorSlot::Line => w.line_color = color,
                ColorSlot::Fill => w.fill_color = color,
            }
        }
        if remember {
            self.remember_color(color);
        }
    }

    /// The colors a tool is about to paint with become recent colors.
    fn remember_used(&mut self, tool: Tool, s: &paint::Settings) {
        let fills = matches!(tool, Tool::Brush | Tool::Spray | Tool::Bucket)
            || s.filled && matches!(tool, Tool::Rect | Tool::RoundRect | Tool::Oval | Tool::RegularPolygon | Tool::Polygon);
        if fills {
            self.remember_color(s.fill_color);
        }
        if !matches!(tool, Tool::Brush | Tool::Spray | Tool::Bucket | Tool::Eraser) {
            self.remember_color(s.line_color);
        }
    }

    fn remember_color(&mut self, color: Pixel) {
        if self.recent_colors.first() == Some(&color) {
            return;
        }
        self.recent_colors.retain(|c| *c != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(RECENT_COLORS);
    }

    /// With the eyedropper: a click on the card takes the color shown there
    /// (the card's paint, else the background's, else white).
    pub(super) fn eyedropper_input(&mut self, resp: &egui::Response, origin: Pos2) {
        if !resp.clicked() {
            return;
        }
        let (Some(slot), Some(q)) = (self.eyedropper, resp.interact_pointer_pos()) else { return };
        let (x, y) = to_card(self.fat, origin, q);
        let color = {
            let w = self.world();
            let card = w.stack.card(w.card_id).map(|c| c.picture.get(x, y)).unwrap_or(Pixel::CLEAR);
            let bg = w.stack.background(w.bg_id()).map(|b| b.picture.get(x, y)).unwrap_or(Pixel::CLEAR);
            paint::paper(if card.is_clear() { bg } else { card })
        };
        self.set_paint_color(slot, color, true);
        self.eyedropper = None;
    }

    /// Repeating 8×8 textures for pattern swatches, in the fill color,
    /// rebuilt when the stack's patterns or the color change.
    pub(super) fn pattern_textures(&mut self, ctx: &egui::Context, patterns: &[[u8; 8]], color: Pixel) -> Vec<egui::TextureId> {
        if self.pattern_textures.as_ref().is_none_or(|(cached, c, _)| cached != patterns || *c != color) {
            let textures = patterns
                .iter()
                .enumerate()
                .map(|(i, rows)| {
                    let mut img = egui::ColorImage::filled([8, 8], Color32::WHITE);
                    for (y, row) in rows.iter().enumerate() {
                        for x in 0..8 {
                            if row & (0x80 >> x) != 0 {
                                img.pixels[y * 8 + x] = color32(color);
                            }
                        }
                    }
                    ctx.load_texture(format!("pattern-{i}"), img, egui::TextureOptions::NEAREST_REPEAT)
                })
                .collect();
            self.pattern_textures = Some((patterns.to_vec(), color, textures));
        }
        self.pattern_textures.as_ref().map(|(_, _, t)| t.iter().map(|h| h.id()).collect()).unwrap_or_default()
    }

    /// Power Keys: with a paint tool, single keys choose paint commands,
    /// as in HyperCard, when the message box is hidden. Option and a digit
    /// sets the space Draw Multiple leaves, power keys or not.
    pub(super) fn power_keys(&mut self, ctx: &egui::Context) {
        let digits = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        for (n, k) in digits.into_iter().enumerate() {
            if ctx.input(|i| i.modifiers.alt && !i.modifiers.command && i.key_pressed(k)) {
                self.world().multi_space = n as u32 + 1;
            }
        }
        let (on, msg) = {
            let w = self.world();
            (w.power_keys, w.msg_visible)
        };
        if !on || msg || ctx.input(|i| i.modifiers.command || i.modifiers.alt) {
            return;
        }
        let typed: Vec<char> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .flat_map(|t| t.chars().collect::<Vec<_>>())
                .collect()
        });
        for c in typed {
            let item = match c.to_ascii_lowercase() {
                'a' => "Select All",
                'c' => "Draw Centered",
                'd' => "Darken",
                'e' => "Trace Edges",
                'f' => "Fill",
                'g' => "Grid",
                'h' => "Flip Horizontal",
                'i' => "Invert",
                'l' => "Lighten",
                'm' => "Draw Multiple",
                'o' => "Opaque",
                't' => "Transparent",
                'v' => "Flip Vertical",
                '[' => "Rotate Left",
                ']' => "Rotate Right",
                // Black and white: the patterns with every bit set, or none.
                'b' | 'w' => {
                    let bits = if c.eq_ignore_ascii_case(&'b') { 0xff } else { 0x00 };
                    let mut w = self.world();
                    if let Some(n) = w.stack.patterns.iter().position(|p| p.iter().all(|r| *r == bits)) {
                        w.pattern = n + 1;
                    }
                    continue;
                }
                _ => continue,
            };
            self.do_menu(item);
        }
    }

    /// The Options menu's Line Size, Brush Shape and Polygon Sides, as
    /// dialogs when the palette isn't showing (when it is, its swatches
    /// pop up the same choices): click a choice to take it.
    pub(super) fn show_paint_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.paint_dialog else { return };
        let title = match dialog {
            PaintDialog::LineSize => "Line Size",
            PaintDialog::BrushShape => "Brush Shape",
            PaintDialog::PolygonSides => "Polygon Sides",
        };
        let mut open = true;
        let mut chosen = false;
        egui::Window::new(title)
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| chosen = self.setting_choices(ui, dialog));
        if !open || chosen {
            self.paint_dialog = None;
        }
    }

    /// The choices for the line size, brush or polygon sides; true when
    /// one was taken.
    fn setting_choices(&mut self, ui: &mut egui::Ui, dialog: PaintDialog) -> bool {
        let (line_size, brush, sides) = {
            let w = self.world();
            (w.line_size, w.brush, w.poly_sides)
        };
        let mut chosen = false;
        let selected = Color32::from_rgb(0, 90, 200);
        // Laid out to fit the palette's width: lines down a column, brushes
        // and polygons in grids.
        let mut choice = |ui: &mut egui::Ui, size: Vec2, current: bool, tip: String, draw: &dyn Fn(&egui::Painter, Rect)| {
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
            draw(ui.painter(), rect);
            let hot = current || resp.hovered();
            let stroke = if hot { Stroke::new(2.0, selected) } else { Stroke::new(1.0, Color32::GRAY) };
            ui.painter().rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
            let clicked = resp.on_hover_text(tip).clicked();
            chosen |= clicked;
            clicked
        };
        match dialog {
            PaintDialog::LineSize => {
                for size in [1u32, 2, 3, 4, 6, 8] {
                    let draw = |p: &egui::Painter, r: Rect| {
                        p.rect_filled(Rect::from_center_size(r.center(), Vec2::new(r.width() - 16.0, size as f32)), 0.0, Color32::BLACK);
                    };
                    if choice(ui, Vec2::new(110.0, 16.0), size == line_size, format!("{size}"), &draw) {
                        self.world().line_size = size;
                    }
                }
            }
            PaintDialog::BrushShape => {
                egui::Grid::new("brush-shapes").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                    for (i, name) in paint::BRUSH_NAMES.iter().enumerate() {
                        let n = i as u32 + 1;
                        let draw = |p: &egui::Painter, r: Rect| {
                            for (dx, dy) in paint::brush_shape(n) {
                                let at = r.center() + Vec2::new(dx as f32, dy as f32) * 1.5;
                                p.rect_filled(Rect::from_min_size(at, Vec2::splat(1.5)), 0.0, Color32::BLACK);
                            }
                        };
                        let current = (brush.max(1) - 1) % paint::BRUSH_NAMES.len() as u32 + 1 == n;
                        if choice(ui, Vec2::splat(26.0), current, name.to_string(), &draw) {
                            self.world().brush = n;
                        }
                        if i % 4 == 3 {
                            ui.end_row();
                        }
                    }
                });
            }
            PaintDialog::PolygonSides => {
                egui::Grid::new("polygon-sides").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                    for (i, n) in [3u32, 4, 5, 6, 8, 12].into_iter().enumerate() {
                        let draw = |p: &egui::Painter, r: Rect| {
                            let c = r.center();
                            let mut pts: Vec<Pos2> = paint::regular_polygon_points((0, 0), (0, -13), n)
                                .iter()
                                .map(|q| c + Vec2::new(q.0 as f32, q.1 as f32 + 1.0))
                                .collect();
                            if let Some(first) = pts.first().copied() {
                                pts.push(first);
                            }
                            p.add(egui::Shape::line(pts, Stroke::new(1.5, Color32::BLACK)));
                        };
                        if choice(ui, Vec2::splat(35.0), n == sides, format!("{n} sides"), &draw) {
                            self.world().poly_sides = n;
                        }
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
            }
        }
        chosen
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
        let fat = self.fat;
        // In FatBits, Option-drag moves the view instead of painting.
        if fat.is_some() && ctx.input(|i| i.modifiers.alt) {
            return;
        }
        let to_card = |q: Pos2| to_card(fat, origin, q);
        let p = resp.interact_pointer_pos().or(resp.hover_pos()).map(to_card);
        // Command-click with the pencil magnifies there, as in HyperCard.
        if tool == Tool::Pencil
            && fat.is_none()
            && resp.clicked()
            && ctx.input(|i| i.modifiers.command)
            && let Some((x, y)) = p
        {
            self.fat_bits_at(x, y);
            return;
        }
        match tool {
            Tool::Select | Tool::Lasso => return self.selection_input(ctx, resp, p, tool),
            Tool::Polygon => return self.polygon_input(ctx, resp, p),
            Tool::Text => return self.text_input(resp, p),
            _ => {}
        }
        let Some(p) = p else { return };
        let (settings, grid, multiple, space) = {
            let w = self.world();
            (w.paint_settings(), w.grid, w.multiple, w.multi_space)
        };
        // Grid: lines and shapes start and end 8 pixels apart.
        let shape = tool.drag_tool().is_some_and(paint::shape_tool);
        let snap = |q: (i32, i32)| if grid && shape { paint::snap_grid(q) } else { q };
        let p = snap(p);
        if resp.drag_started_by(PointerButton::Primary) || resp.clicked() && self.stroke.is_none() {
            let start = snap(resp.interact_pointer_pos().map(to_card).unwrap_or(p));
            self.push_undo();
            self.remember_used(tool, &settings);
            if tool == Tool::Bucket {
                self.with_picture(|pic| paint::fill(pic, start.0, start.1, settings.ink()));
                return;
            }
            // The pencil draws white over its own color, like MacPaint.
            let color = if tool == Tool::Pencil {
                self.with_picture(|pic| paint::pencil_color(pic, start.0, start.1, settings.line_color))
                    .unwrap_or(settings.line_color)
            } else {
                settings.line_color
            };
            self.stroke = Some(PaintStroke { start, last: start, color, axis: None, multi_last: start });
            if freehand(tool) {
                self.freehand(tool, start, start, color, &settings);
            }
            if resp.clicked() {
                self.stroke = None;
            }
            return;
        }
        let Some(stroke) = &mut self.stroke else { return };
        let (start, last, color) = (stroke.start, stroke.last, stroke.color);
        // Shift goes straight: across or down for the freehand tools, at a
        // multiple of 15° for lines, and square for shapes.
        let shift = ctx.input(|i| i.modifiers.shift);
        let p = match tool.drag_tool() {
            Some(_) if freehand(tool) => {
                if !shift {
                    stroke.axis = None;
                } else if stroke.axis.is_none() {
                    stroke.axis = paint::Axis::of(last, p).map(|axis| (axis, last));
                }
                stroke.axis.map_or(p, |(axis, from)| axis.keep(from, p))
            }
            Some(t) if shift => snap(paint::constrain(t, start, p)),
            _ => p,
        };
        // Draw Multiple: the shape is left behind as the drag goes.
        if resp.dragged()
            && multiple
            && shape
            && paint::multiple_due(stroke.multi_last, p, space)
            && let Some(t) = tool.drag_tool()
        {
            stroke.multi_last = p;
            self.with_picture(|pic| paint::apply_drag(pic, t, start, p, &settings));
        }
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
        // Grid: selection rectangles and moves go by 8 pixels.
        let grid = self.world().grid;
        let p = if grid && tool == Tool::Select { paint::snap_grid(p) } else { p };
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
                Some(Marquee::Moving { press, offset, .. }) => {
                    *offset = (p.0 - press.0, p.1 - press.1);
                    // Shift moves it straight across or straight up and down.
                    if ctx.input(|i| i.modifiers.shift)
                        && let Some(axis) = paint::Axis::of((0, 0), *offset)
                    {
                        *offset = axis.keep((0, 0), *offset);
                    }
                    if grid {
                        *offset = paint::snap_grid(*offset);
                    }
                }
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
    fn polygon_input(&mut self, ctx: &egui::Context, resp: &egui::Response, p: Option<(i32, i32)>) {
        let Some(p) = p else { return };
        let p = self.polygon_point(ctx, p);
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

    /// The polygon's next corner: with Shift, its side goes at a multiple
    /// of 15°.
    fn polygon_point(&self, ctx: &egui::Context, p: (i32, i32)) -> (i32, i32) {
        match self.polygon.as_ref().and_then(|pts| pts.last()) {
            Some(&last) if ctx.input(|i| i.modifiers.shift) => paint::snap_15(last, p),
            _ => p,
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
        self.remember_used(Tool::Polygon, &s);
        self.with_picture(|pic| {
            if closed || s.filled {
                paint::polygon(pic, &pts, s.outline(), s.filled.then(|| s.ink()));
            } else {
                for seg in pts.windows(2) {
                    paint::line(pic, seg[0].0, seg[0].1, seg[1].0, seg[1].1, s.line_size, s.line_color);
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
        let (size, font, card_size, color) = {
            let w = self.world();
            (
                w.paint_text_size,
                w.paint_font.to_ascii_lowercase(),
                Vec2::new(w.stack.width as f32, w.stack.height as f32),
                color32(w.line_color),
            )
        };
        let Some(entry) = &mut self.text_entry else { return };
        let family = if font.contains("courier") || font.contains("monaco") || font.contains("mono") {
            FontFamily::Monospace
        } else {
            FontFamily::Proportional
        };
        let min = to_screen(self.fat, origin, entry.pos.0 as f32, entry.pos.1 as f32);
        let rect = Rect::from_min_max(min, (origin + card_size).max(min + Vec2::splat(40.0)));
        let resp = ui.put(
            rect,
            egui::TextEdit::multiline(&mut entry.text)
                .id(egui::Id::unique("paint-text"))
                .frame(egui::Frame::NONE)
                .margin(Vec2::ZERO)
                .font(FontId::new(size as f32, family))
                .text_color(color)
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
        let (size, font, style, color) = {
            let w = self.world();
            (w.paint_text_size, w.paint_font.clone(), w.paint_text_style.clone(), w.line_color)
        };
        self.push_undo();
        self.remember_color(color);
        let how = paint::PaintText { size, font: &font, style: &style, color };
        self.with_picture(|pic| paint::draw_text(pic, entry.pos.0, entry.pos.1, &entry.text, &how));
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

    /// Select a rectangle of the picture, as Import Paint leaves it.
    pub(super) fn select_paint(&mut self, rect: (i32, i32, i32, i32)) {
        if !matches!(self.world().tool, Tool::Select | Tool::Lasso) {
            self.choose_tool(Tool::Select);
        }
        self.commit_marquee();
        self.marquee = Some(Marquee::Selected { region: Region { rect, lasso: None } });
    }

    /// Export Paint, once the card has been drawn clean: ask for a
    /// screenshot, and when it comes, save the card's part of it.
    pub(super) fn take_export(&mut self, ctx: &egui::Context) {
        let size = {
            let w = self.world();
            (w.stack.width, w.stack.height)
        };
        let Some(export) = &mut self.export else { return };
        if export.done.load(std::sync::atomic::Ordering::Acquire) {
            self.export = None;
            return;
        }
        if !export.asked {
            export.asked = true;
            let (origin, ppp) = (self.card_origin, ctx.pixels_per_point());
            let (path, done) = (export.path.clone(), export.done.clone());
            ctx.request_screenshot(move |shot| {
                if let Err(e) = save_card_shot(&shot, origin, ppp, size, &path) {
                    eprintln!("Export Paint: {e}");
                }
                done.store(true, std::sync::atomic::Ordering::Release);
            });
        }
        ctx.request_repaint();
    }

    /// Apply a Paint menu command to the selection, or the whole picture.
    pub(super) fn apply_paint_op(&mut self, op: PaintOp) {
        self.commit_marquee();
        let selection = match &self.marquee {
            Some(Marquee::Selected { region }) => Some(region.clone()),
            _ => None,
        };
        let ink = self.world().paint_settings().ink();
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

    /// Turn FatBits on, centered on a card pixel.
    pub(super) fn fat_bits_at(&mut self, x: i32, y: i32) {
        let (w, h) = {
            let w = self.world();
            (w.stack.width as f32, w.stack.height as f32)
        };
        let (vw, vh) = (w / FAT_ZOOM, h / FAT_ZOOM);
        self.fat = Some(FatBits {
            left: (x as f32 - vw / 2.0).clamp(0.0, (w - vw).max(0.0)),
            top: (y as f32 - vh / 2.0).clamp(0.0, (h - vh).max(0.0)),
        });
    }

    /// Moving around in FatBits: Option-drag (Alt) or the scroll wheel.
    pub(super) fn fat_bits_pan(&mut self, ctx: &egui::Context, resp: &egui::Response, size: Vec2) {
        let Some(f) = &mut self.fat else { return };
        let mut delta = Vec2::ZERO;
        if ctx.input(|i| i.modifiers.alt) && resp.dragged() {
            delta -= resp.drag_delta() / FAT_ZOOM;
        }
        if resp.hovered() {
            delta -= ctx.input(|i| i.smooth_scroll_delta) / FAT_ZOOM;
        }
        let (vw, vh) = (size.x / FAT_ZOOM, size.y / FAT_ZOOM);
        f.left = (f.left + delta.x).clamp(0.0, (size.x - vw).max(0.0));
        f.top = (f.top + delta.y).clamp(0.0, (size.y - vh).max(0.0));
    }

    /// FatBits: the paint magnified, with a line between pixels, and the
    /// same part at actual size in the corner. Buttons and fields aren't
    /// shown, as in HyperCard.
    pub(super) fn draw_fat_bits(&self, painter: &egui::Painter, scene: &render::Scene, origin: Pos2, size: Vec2) {
        let Some(f) = self.fat else { return };
        let (left, top) = (f.left.floor(), f.top.floor());
        let view = Vec2::new(size.x / FAT_ZOOM, size.y / FAT_ZOOM);
        let uv = Rect::from_min_size(Pos2::new(left / size.x, top / size.y), Vec2::new(view.x / size.x, view.y / size.y));
        let whole = Rect::from_min_size(origin, size);
        let draw = |rect: Rect| {
            painter.rect_filled(rect, 0.0, Color32::WHITE);
            let layers = [
                (crate::model::Layer::Background, scene.bg, scene.show_bg_pict),
                (crate::model::Layer::Card, scene.card, scene.show_card_pict),
            ];
            for (layer, id, shown) in layers {
                if let Some(t) = self.tex.picture(layer, id, scene.color_key).filter(|_| shown) {
                    painter.image(t.id(), rect, uv, Color32::WHITE);
                }
            }
        };
        draw(whole);
        // The lines between pixels.
        let grid = Stroke::new(1.0, Color32::from_gray(225));
        for i in 1..view.x.ceil() as i32 {
            let x = origin.x + i as f32 * FAT_ZOOM;
            painter.line_segment([Pos2::new(x, whole.top()), Pos2::new(x, whole.bottom())], grid);
        }
        for i in 1..view.y.ceil() as i32 {
            let y = origin.y + i as f32 * FAT_ZOOM;
            painter.line_segment([Pos2::new(whole.left(), y), Pos2::new(whole.right(), y)], grid);
        }
        // Actual size, in the corner.
        let inset = Rect::from_min_size(Pos2::new(whole.left() + 8.0, whole.bottom() - view.y - 8.0), view).round();
        painter.rect_filled(inset.expand(2.0), 0.0, Color32::BLACK);
        draw(inset);
    }

    pub(super) fn paint_overlay(&self, painter: &egui::Painter, origin: Pos2, tool: Tool, ctx: &egui::Context) {
        let fat = self.fat;
        let at = |x: i32, y: i32| to_screen(fat, origin, x as f32 + 0.5, y as f32 + 0.5);
        let stroke = Stroke::new(1.0, Color32::BLACK);
        let dashed = |points: &[Pos2]| {
            for s in egui::Shape::dashed_line(points, stroke, 4.0, 3.0) {
                painter.add(s);
            }
        };
        if let Some(s) = &self.stroke {
            let centered = self.world().centered;
            let start = match tool.drag_tool() {
                Some(t) if centered => paint::centered_start(t, s.start, s.last),
                _ => s.start,
            };
            let (a, b) = (at(start.0, start.1), at(s.last.0, s.last.1));
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
                let card = to_card(fat, origin, mouse);
                let corner = self.polygon_point(ctx, card);
                line.push(at(corner.0, corner.1));
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

/// Export Paint waiting for its screenshot.
pub struct Export {
    path: std::path::PathBuf,
    asked: bool,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Export {
    pub fn new(path: std::path::PathBuf) -> Export {
        Export { path, asked: false, done: Default::default() }
    }
}

/// Save the card's part of a window screenshot as a PNG, at one pixel a
/// card pixel whatever the screen's scale.
fn save_card_shot(shot: &egui::ColorImage, origin: Pos2, ppp: f32, (width, height): (u32, u32), path: &std::path::Path) -> Result<(), String> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let sx = ((origin.x + x as f32 + 0.5) * ppp) as usize;
            let sy = ((origin.y + y as f32 + 0.5) * ppp) as usize;
            let c = if sx < shot.width() && sy < shot.height() { shot[(sx, sy)] } else { egui::Color32::WHITE };
            rgba.extend([c.r(), c.g(), c.b(), 255]);
        }
    }
    let png = crate::paint::image_file::encode_png(width, height, &rgba)?;
    std::fs::write(path, png).map_err(|e| e.to_string())
}
