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

/// A pattern being edited: which one, its bits so far, whether the next
/// click on the card takes the pattern there, and, during a drag, whether
/// squares are being turned on or off.
pub struct PatternEdit {
    index: usize,
    bits: [u8; 8],
    pub picking: bool,
    painting: Option<bool>,
    /// The preview strip's picture.
    preview: Option<TextureHandle>,
}

/// The palette's swatches, each showing what's chosen and popping up the
/// choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swatch {
    Pattern,
    Color(ColorSlot),
}

impl Swatch {
    fn tip(self) -> &'static str {
        match self {
            Swatch::Pattern => "Pattern: for the brush, spray, bucket and filled shapes. Double-click to edit it.",
            Swatch::Color(ColorSlot::Line) => "Line color: lines, borders, the pencil and text",
            Swatch::Color(ColorSlot::Fill) => "Fill color: the pattern's color",
        }
    }
}

/// The width of the palette's rows, inside its margins.
pub(super) const ROW: f32 = 7.0 * TOOL_SIZE + 5.0 * TOOL_GAP + GROUP_GAP;
/// The room between the palette's buttons.
const GAP: f32 = 2.0;

/// The tools in their groups, a row at a time.
const TOOL_ROWS: [&[(&str, &[Tool])]; 3] = [
    &[("Card", &[Tool::Browse, Tool::Button, Tool::Field]), ("Select", &[Tool::Select, Tool::Lasso])],
    &[("Paint", &[Tool::Pencil, Tool::Brush, Tool::Spray, Tool::Eraser, Tool::Bucket, Tool::Text])],
    &[
        ("Lines", &[Tool::Line, Tool::Curve]),
        ("Shapes", &[Tool::Rect, Tool::RoundRect, Tool::Oval, Tool::RegularPolygon, Tool::Polygon]),
    ],
];
/// A tool button's size, and the room between tools and between groups:
/// seven tools and two groups fill a row.
const TOOL_SIZE: f32 = 38.0;
const TOOL_GAP: f32 = 4.0;
const GROUP_GAP: f32 = 12.0;

/// A paint option a tool can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolOption {
    LineSize,
    Filled,
    Centered,
    Multiple,
    Sides,
    BrushShape,
    SpraySize,
    SprayDensity,
}

/// The options a tool understands, in the order they show.
fn tool_options(t: Tool) -> &'static [ToolOption] {
    use ToolOption::*;
    match t {
        Tool::Line => &[LineSize, Centered, Multiple],
        Tool::Curve => &[LineSize],
        Tool::Rect | Tool::RoundRect | Tool::Oval => &[LineSize, Filled, Centered, Multiple],
        Tool::RegularPolygon => &[LineSize, Filled, Sides, Multiple],
        Tool::Polygon => &[LineSize, Filled],
        Tool::Brush => &[BrushShape],
        Tool::Spray => &[SpraySize, SprayDensity],
        _ => &[],
    }
}

/// The tool whose options hold one of the Options menu's settings: the
/// chosen tool when it has it, else the tool it's best known for.
pub(super) fn options_tool(chosen: Tool, d: PaintDialog) -> Tool {
    let (option, usual) = match d {
        PaintDialog::LineSize => (ToolOption::LineSize, Tool::Line),
        PaintDialog::BrushShape => (ToolOption::BrushShape, Tool::Brush),
        PaintDialog::PolygonSides => (ToolOption::Sides, Tool::RegularPolygon),
    };
    if tool_options(chosen).contains(&option) { chosen } else { usual }
}

/// A pull-down's button: what's chosen, drawn by `draw`, and a ▾.
fn pull_down(ui: &mut egui::Ui, size: Vec2, draw: impl FnOnce(&egui::Painter, Rect)) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let edge = if resp.hovered() { Color32::from_gray(150) } else { Color32::from_gray(200) };
    ui.painter().rect(rect, 5.0, Color32::WHITE, Stroke::new(1.0, edge), StrokeKind::Inside);
    draw(ui.painter(), rect.with_max_x(rect.max.x - 16.0));
    let arrow = rect.right_center() - Vec2::new(9.0, 0.0);
    let down = vec![arrow + Vec2::new(-4.0, -2.0), arrow + Vec2::new(4.0, -2.0), arrow + Vec2::new(0.0, 3.0)];
    ui.painter().add(egui::Shape::convex_polygon(down, Color32::from_gray(80), Stroke::NONE));
    resp
}

/// Which paint a tool uses: (the line color, the pattern in the fill color).
/// The shapes use the pattern only with Draw Filled; Select and Lasso use
/// it for Fill.
fn uses_paint(t: Tool, filled: bool) -> (bool, bool) {
    match t {
        Tool::Pencil | Tool::Text | Tool::Line | Tool::Curve => (true, false),
        Tool::Brush | Tool::Spray | Tool::Bucket | Tool::Select | Tool::Lasso => (false, true),
        Tool::Rect | Tool::RoundRect | Tool::Oval | Tool::RegularPolygon | Tool::Polygon => (true, filled),
        Tool::Eraser | Tool::Browse | Tool::Button | Tool::Field => (false, false),
    }
}

/// The size of each of `n` buttons sharing a palette row.
fn cell_size(n: usize, height: f32) -> Vec2 {
    Vec2::new(((ROW - GAP * (n as f32 - 1.0)) / n as f32).floor(), height)
}

/// A color picker the palette's width: saturation across and brightness
/// down for the hue below it, then the red, green and blue to type or drag.
/// Returns the new color when it changes.
fn color_picker(ui: &mut egui::Ui, current: Color32) -> Option<Color32> {
    use egui::ecolor::Hsva;
    let width = ui.available_width().floor();
    // The hue is kept while the color is gray or black, which have none.
    let id = egui::Id::unique("palette-hsva");
    let mut hsva = match ui.data(|d| d.get_temp::<(Color32, Hsva)>(id)) {
        Some((c, h)) if c == current => h,
        _ => Hsva::from(current),
    };
    let before = hsva;
    let mesh_rect = |painter: &egui::Painter, rect: Rect, (cols, rows): (usize, usize), color_at: &dyn Fn(f32, f32) -> Color32| {
        let mut mesh = egui::Mesh::default();
        for j in 0..=rows {
            for i in 0..=cols {
                let (u, v) = (i as f32 / cols as f32, j as f32 / rows as f32);
                mesh.colored_vertex(rect.lerp_inside(Vec2::new(u, v)), color_at(u, v));
            }
        }
        let row = cols as u32 + 1;
        for j in 0..rows as u32 {
            for i in 0..cols as u32 {
                let a = j * row + i;
                mesh.add_triangle(a, a + 1, a + row);
                mesh.add_triangle(a + 1, a + row + 1, a + row);
            }
        }
        painter.add(mesh);
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
    };
    let marker = |painter: &egui::Painter, at: Pos2| {
        painter.circle_stroke(at, 4.0, Stroke::new(2.5, Color32::BLACK));
        painter.circle_stroke(at, 4.0, Stroke::new(1.0, Color32::WHITE));
    };
    // Saturation and brightness.
    let (sv, resp) = ui.allocate_exact_size(Vec2::new(width, 90.0), Sense::click_and_drag());
    if let Some(p) = resp.interact_pointer_pos() {
        hsva.s = ((p.x - sv.left()) / sv.width()).clamp(0.0, 1.0);
        hsva.v = 1.0 - ((p.y - sv.top()) / sv.height()).clamp(0.0, 1.0);
    }
    let h = hsva.h;
    mesh_rect(ui.painter(), sv, (12, 6), &|u, v| Hsva::new(h, u, 1.0 - v, 1.0).into());
    marker(ui.painter(), Pos2::new(sv.left() + hsva.s * sv.width(), sv.top() + (1.0 - hsva.v) * sv.height()));
    // Hue.
    let (bar, resp) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::click_and_drag());
    if let Some(p) = resp.interact_pointer_pos() {
        hsva.h = ((p.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
    }
    mesh_rect(ui.painter(), bar, (36, 1), &|u, _| Hsva::new(u, 1.0, 1.0, 1.0).into());
    marker(ui.painter(), Pos2::new(bar.left() + hsva.h * bar.width(), bar.center().y));
    let mut color = if hsva == before { current } else { Color32::from(hsva) };
    // Red, green and blue, and the color as hex, as on the web.
    let [mut r, mut g, mut b, _] = color.to_array();
    let hex_id = egui::Id::unique("palette-hex");
    let (typed, hex) = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            ui.spacing_mut().interact_size.x = 0.0;
            let field = ((width - 3.0 * GAP) / 4.0).floor();
            let mut changed = false;
            for (n, label) in [(&mut r, "R "), (&mut g, "G "), (&mut b, "B ")] {
                changed |= ui.add_sized([field, 18.0], egui::DragValue::new(n).speed(0.5).prefix(label)).changed();
            }
            // While it's being typed in, the field keeps what's typed;
            // otherwise it shows the color.
            let shown = format!("#{r:02X}{g:02X}{b:02X}");
            let mut text = ui.data(|d| d.get_temp::<String>(hex_id)).unwrap_or(shown);
            let edit = egui::TextEdit::singleline(&mut text).char_limit(7).font(egui::TextStyle::Monospace);
            let resp = ui.add_sized([field, 18.0], edit).on_hover_text("The color as hex, as on the web: #2C78DC or #F80");
            let hex = if resp.has_focus() {
                ui.data_mut(|d| d.insert_temp(hex_id, text));
                None
            } else {
                ui.data_mut(|d| d.remove::<String>(hex_id));
                // Return or clicking away takes it, with or without the #,
                // or a name such as "orange".
                let parse = crate::model::color::parse_color;
                let t = text.trim();
                resp.lost_focus().then(|| parse(t).or_else(|| parse(&format!("#{t}")))).flatten()
            };
            (changed, hex)
        })
        .inner;
    if typed {
        color = Color32::from_rgb(r, g, b);
        hsva = Hsva::from(color);
    }
    if let Some([r, g, b]) = hex {
        color = Color32::from_rgb(r, g, b);
        hsva = Hsva::from(color);
    }
    ui.data_mut(|d| d.insert_temp(id, (color, hsva)));
    (color != current).then_some(color)
}

/// The Options menu's settings that go on and off: item, icon, tip.
const OPTION_TOGGLES: [(&str, &str, &str); 2] = [
    ("Grid", "#", "Grid: lines, shapes and selections snap to 8 pixels"),
    ("FatBits", "🔍", "FatBits: magnify to paint pixel by pixel"),
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

/// Tools that paint continuously as the mouse moves.
fn freehand(t: Tool) -> bool {
    matches!(t, Tool::Pencil | Tool::Brush | Tool::Eraser | Tool::Spray | Tool::Curve)
}

impl App {
    // ----- palette -----

    /// The palette, in rows the same width: the tools, the pattern with its
    /// Edit button, the colors and line settings, the options, and the
    /// Paint commands. Clicking a swatch unfolds its choices just below its
    /// row. Returns the height its contents take.
    pub(super) fn palette(&mut self, ui: &mut egui::Ui) -> f32 {
        let (tool, look) = {
            let w = self.world();
            let filled = w.fill_shapes.then(|| (w.stack.pattern(w.pattern), w.fill_color.0));
            (w.tool, super::tool_icons::Look { line_size: w.line_size, sides: w.poly_sides, filled })
        };
        let mut height = 0.0;
        // A scroll bar with its own room, so it doesn't cover the swatches.
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(2.0);
            for (row, groups) in TOOL_ROWS.iter().enumerate() {
                if row > 0 {
                    ui.add_space(6.0);
                }
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = GROUP_GAP;
                    for (label, tools) in groups.iter() {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.label(RichText::new(label.to_uppercase()).size(9.5).strong().color(Color32::from_gray(90)));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = TOOL_GAP;
                                for t in tools.iter() {
                                    self.tool_button(ui, *t, tool, look);
                                }
                            });
                        });
                    }
                });
                // A tool's options unfold under its own row.
                if let Some(t) = self.tool_options
                    && groups.iter().any(|(_, tools)| tools.contains(&t))
                {
                    self.tool_options_row(ui, t);
                }
            }
            // Browse, Button and Field don't paint: nothing below applies.
            if tool.is_paint() {
                ui.separator();
                self.patterns_section(ui, tool);
                ui.separator();
                self.option_toggles(ui);
            }
            height = ui.min_rect().height();
        });
        height
    }

    /// Patterns: the line and fill colors as two overlapping squares, the
    /// pattern, and Edit… beside it. Clicking one unfolds its choices
    /// below. What the chosen tool doesn't use is greyed out.
    fn patterns_section(&mut self, ui: &mut egui::Ui, tool: Tool) {
        let (filled, line_color, fill_color) = {
            let w = self.world();
            (w.fill_shapes, w.line_color, w.fill_color)
        };
        let (uses_line, uses_pattern) = uses_paint(tool, filled);
        // Choices for something the tool doesn't use fold away.
        let open_unused = match self.open_swatch {
            Some(Swatch::Pattern) | Some(Swatch::Color(ColorSlot::Fill)) => !uses_pattern,
            Some(Swatch::Color(ColorSlot::Line)) => !uses_line,
            _ => false,
        };
        if open_unused {
            self.open_swatch = None;
            self.pattern_edit = None;
        }
        ui.label(RichText::new("PATTERNS").size(9.5).strong().color(Color32::from_gray(90)));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            // The colors: the line color in front, the fill color behind.
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let (area, _) = ui.allocate_exact_size(Vec2::splat(50.0), Sense::hover());
                let back = Rect::from_min_size(area.min + Vec2::splat(18.0), Vec2::splat(32.0));
                let front = Rect::from_min_size(area.min, Vec2::splat(32.0));
                let sense = |on: bool| if on { Sense::click() } else { Sense::hover() };
                let fill_resp = ui.interact(back, egui::Id::unique("palette-fill-color"), sense(uses_pattern));
                let line_resp = ui.interact(front, egui::Id::unique("palette-line-color"), sense(uses_line));
                let blue = Color32::from_rgb(44, 120, 220);
                let fade = |c: Color32, on: bool| if on { c } else { c.gamma_multiply(0.25) };
                let painter = ui.painter();
                for (rect, resp, slot, on) in [(back, &fill_resp, ColorSlot::Fill, uses_pattern), (front, &line_resp, ColorSlot::Line, uses_line)] {
                    let open = self.open_swatch == Some(Swatch::Color(slot));
                    let color = color32(if slot == ColorSlot::Line { line_color } else { fill_color });
                    if slot == ColorSlot::Line {
                        painter.rect_filled(rect, 6.0, fade(Color32::WHITE, on));
                        painter.rect_stroke(rect.shrink(3.0), 3.0, Stroke::new(6.0, fade(color, on)), StrokeKind::Middle);
                    } else {
                        painter.rect_filled(rect, 6.0, fade(color, on));
                    }
                    let edge = if open || (on && resp.hovered()) { Stroke::new(2.0, blue) } else { Stroke::new(1.0, fade(Color32::from_gray(110), on)) };
                    painter.rect_stroke(rect, 6.0, edge, StrokeKind::Outside);
                }
                ui.label(RichText::new("Line · Fill").size(10.0).color(Color32::from_gray(80)));
                let tips = [
                    (&line_resp, ColorSlot::Line, uses_line, "Line color: the pencil, lines, outlines and text"),
                    (&fill_resp, ColorSlot::Fill, uses_pattern, "Fill color: the color the pattern paints in"),
                ];
                for (resp, slot, on, tip) in tips {
                    let tip = if on { tip.to_string() } else { format!("{tip}. The {} doesn't use it.", tool.name()) };
                    if resp.clone().on_hover_text(tip).clicked() {
                        let which = Swatch::Color(slot);
                        self.open_swatch = if self.open_swatch == Some(which) { None } else { Some(which) };
                        self.pattern_edit = None;
                    }
                }
            });
            // The pattern, and Edit… for changing it.
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add_enabled_ui(uses_pattern, |ui| self.swatch(ui, Swatch::Pattern, Vec2::splat(48.0)));
                ui.label(RichText::new("Pattern").size(10.0).color(Color32::from_gray(80)));
            });
            let editing = self.pattern_edit.is_some();
            let resp = ui
                .add_enabled(uses_pattern, egui::Button::new("Edit…").selected(editing).min_size(Vec2::new(64.0, 30.0)))
                .on_hover_text("Edit Pattern: change the chosen pattern; it's kept with the stack");
            if resp.clicked() {
                if editing {
                    self.pattern_edit = None;
                    self.open_swatch = None;
                } else {
                    let n = self.world().pattern;
                    self.edit_pattern(n);
                }
            }
        });
        self.unfold(ui, &[Swatch::Pattern, Swatch::Color(ColorSlot::Line), Swatch::Color(ColorSlot::Fill)]);
    }

    /// One tool's button: its icon, showing its setting, and a corner mark
    /// when double-clicking it does more. Double-clicking a tool with
    /// choices opens them under its row, and again closes them.
    fn tool_button(&mut self, ui: &mut egui::Ui, t: Tool, chosen: Tool, look: super::tool_icons::Look) {
        let icon = TOOL_SIZE - 12.0;
        let name = windows::capitalize(t.name());
        let has_options = !tool_options(t).is_empty();
        let tip = match t {
            _ if has_options => format!("{name}. Double-click for its options."),
            Tool::Pencil => format!("{name}. Double-click for FatBits."),
            _ => name.clone(),
        };
        // A tile: blue when chosen, a little darker under the mouse.
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(TOOL_SIZE), Sense::click());
        let (fill, stroke) = if chosen == t {
            (Color32::from_rgb(228, 240, 255), Stroke::new(2.0, Color32::from_rgb(44, 120, 220)))
        } else if resp.hovered() {
            (Color32::from_rgb(238, 242, 246), Stroke::new(1.0, Color32::from_gray(170)))
        } else {
            (Color32::from_gray(252), Stroke::new(1.0, Color32::from_gray(212)))
        };
        ui.painter().rect(rect, 7.0, fill, stroke, StrokeKind::Inside);
        match self.tool_icons.get(ui.ctx(), t, look, icon) {
            Some(tex) => {
                let at = Rect::from_center_size(rect.center(), Vec2::splat(icon));
                ui.painter().image(tex, at, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, &name, FontId::proportional(9.0), Color32::BLACK);
            }
        }
        let resp = resp.on_hover_text(tip);
        if has_options {
            let rb = resp.rect.right_bottom() + Vec2::new(-3.0, -3.0);
            let mark = vec![rb, rb + Vec2::new(-6.0, 0.0), rb + Vec2::new(0.0, -6.0)];
            ui.painter().add(egui::Shape::convex_polygon(mark, Color32::from_gray(100), Stroke::NONE));
        }
        if resp.clicked() {
            // Another tool's options fold away.
            if self.tool_options.is_some_and(|open| open != t) {
                self.tool_options = None;
            }
            self.choose_tool(t);
        }
        // Double-clicking a tool opens its options, and again closes them.
        // The pencil's turns on FatBits, as in HyperCard.
        if resp.double_clicked() {
            if has_options {
                self.tool_options = if self.tool_options == Some(t) { None } else { Some(t) };
            } else if t == Tool::Pencil {
                self.do_menu("FatBits");
            }
        }
    }

    /// A tool's options, under its row: only the ones it understands.
    /// They're shared: the oval's line size is the rectangle's too.
    fn tool_options_row(&mut self, ui: &mut egui::Ui, t: Tool) {
        let options = tool_options(t);
        ui.add_space(4.0);
        egui::Frame::group(ui.style()).inner_margin(6.0).show(ui, |ui| {
            ui.set_width(ROW - 14.0);
            if options == [ToolOption::BrushShape] {
                self.setting_choices(ui, PaintDialog::BrushShape);
                return;
            }
            let (filled, centered, multiple, enabled) = {
                let w = self.world();
                let enabled = |item| menu_item_enabled(&w, item);
                (w.fill_shapes, w.centered, w.multiple, [enabled("Draw Filled"), enabled("Draw Centered"), enabled("Draw Multiple")])
            };
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for option in options {
                    let toggle = match option {
                        ToolOption::LineSize => {
                            self.line_size_menu(ui);
                            None
                        }
                        ToolOption::Sides => {
                            self.sides_menu(ui);
                            None
                        }
                        ToolOption::SpraySize => {
                            self.spray_size_menu(ui);
                            None
                        }
                        ToolOption::SprayDensity => {
                            self.spray_density_buttons(ui);
                            None
                        }
                        ToolOption::Filled => Some(("Filled", "Draw Filled", filled, enabled[0], "Draw Filled: fill shapes with the pattern")),
                        ToolOption::Centered => {
                            Some(("Centered", "Draw Centered", centered, enabled[1], "Draw Centered: grow from where you start"))
                        }
                        ToolOption::Multiple => Some((
                            "Multiple",
                            "Draw Multiple",
                            multiple,
                            enabled[2],
                            "Draw Multiple: leave the shape behind as you drag (Option-1 to 9 spaces them)",
                        )),
                        ToolOption::BrushShape => None,
                    };
                    if let Some((label, item, on, enabled, tip)) = toggle {
                        let button = egui::Button::new(label).selected(on).min_size(Vec2::new(0.0, 24.0));
                        if ui.add_enabled(enabled, button).on_hover_text(tip).clicked() {
                            self.do_menu(item);
                        }
                    }
                }
            });
        });
    }

    /// The line size as a pull-down showing the line itself.
    fn line_size_menu(&mut self, ui: &mut egui::Ui) {
        let current = self.world().line_size;
        let sample = |painter: &egui::Painter, r: Rect, size: u32| {
            let line = Rect::from_center_size(r.center(), Vec2::new(r.width() - 12.0, size as f32));
            painter.rect_filled(line, 0.0, Color32::BLACK);
        };
        let resp = pull_down(ui, Vec2::new(84.0, 24.0), |painter, r| sample(painter, r, current))
            .on_hover_text("Line size: lines, curves and shape outlines");
        egui::Popup::menu(&resp).show(|ui| {
            for size in [1u32, 2, 3, 4, 6, 8] {
                let (r, item) = ui.allocate_exact_size(Vec2::new(120.0, 22.0), Sense::click());
                if size == current || item.hovered() {
                    ui.painter().rect_filled(r, 3.0, Color32::from_rgb(228, 240, 255));
                }
                sample(ui.painter(), r.with_max_x(r.max.x - 28.0), size);
                let at = r.right_center() - Vec2::new(8.0, 0.0);
                ui.painter().text(at, egui::Align2::RIGHT_CENTER, size.to_string(), FontId::proportional(11.0), Color32::from_gray(70));
                if item.clicked() {
                    self.world().line_size = size;
                }
            }
        });
    }

    /// The spray's size as a pull-down showing its circle.
    fn spray_size_menu(&mut self, ui: &mut egui::Ui) {
        let current = self.world().spray_size;
        let circle = |painter: &egui::Painter, c: Pos2, size: u32| {
            painter.circle_stroke(c, size as f32 / 2.0, Stroke::new(1.0, Color32::from_gray(60)));
            painter.circle_filled(c, 1.0, Color32::from_gray(60));
        };
        let resp = pull_down(ui, Vec2::new(72.0, 24.0), |painter, r| {
            circle(painter, r.left_center() + Vec2::new(14.0, 0.0), current.min(20));
            let at = r.left_center() + Vec2::new(28.0, 0.0);
            painter.text(at, egui::Align2::LEFT_CENTER, current.to_string(), FontId::proportional(12.0), Color32::BLACK);
        })
        .on_hover_text("Spray size: the circle it sprays in, in pixels across");
        egui::Popup::menu(&resp).show(|ui| {
            for size in paint::SPRAY_SIZES {
                let (r, item) = ui.allocate_exact_size(Vec2::new(100.0, size as f32 + 8.0), Sense::click());
                if size == current || item.hovered() {
                    ui.painter().rect_filled(r, 3.0, Color32::from_rgb(228, 240, 255));
                }
                circle(ui.painter(), r.left_center() + Vec2::new(22.0, 0.0), size);
                let at = r.left_center() + Vec2::new(46.0, 0.0);
                ui.painter().text(at, egui::Align2::LEFT_CENTER, format!("{size} pixels"), FontId::proportional(12.0), Color32::from_gray(40));
                if item.clicked() {
                    self.world().spray_size = size;
                }
            }
        });
    }

    /// The spray's density: how many dots at a time.
    fn spray_density_buttons(&mut self, ui: &mut egui::Ui) {
        let current = self.world().spray_density;
        for (density, name) in paint::SPRAY_DENSITIES {
            let button = egui::Button::new(name).selected(current == density).min_size(Vec2::new(0.0, 24.0));
            if ui.add(button).on_hover_text(format!("{name}: {density} dots at a time")).clicked() {
                self.world().spray_density = density;
            }
        }
    }

    /// Regular Polygon's sides as a pull-down showing the polygon.
    fn sides_menu(&mut self, ui: &mut egui::Ui) {
        let current = self.world().poly_sides;
        let shape = |painter: &egui::Painter, c: Pos2, n: u32, r: f32| {
            // Standing on a side, like the tool's icon.
            let tau = std::f32::consts::TAU;
            let start = -tau / 4.0 + if n.is_multiple_of(4) { tau / (2.0 * n as f32) } else { 0.0 };
            let mut pts: Vec<Pos2> = (0..n).map(|i| c + Vec2::angled(start + i as f32 * tau / n as f32) * r).collect();
            pts.push(pts[0]);
            painter.add(egui::Shape::line(pts, Stroke::new(1.5, Color32::BLACK)));
        };
        let resp = pull_down(ui, Vec2::new(64.0, 24.0), |painter, r| {
            shape(painter, r.left_center() + Vec2::new(16.0, 0.0), current, 8.0);
            let at = r.left_center() + Vec2::new(30.0, 0.0);
            painter.text(at, egui::Align2::LEFT_CENTER, current.to_string(), FontId::proportional(12.0), Color32::BLACK);
        })
        .on_hover_text("Sides of the regular polygon");
        egui::Popup::menu(&resp).show(|ui| {
            for n in [3u32, 4, 5, 6, 8, 12] {
                let (r, item) = ui.allocate_exact_size(Vec2::new(90.0, 26.0), Sense::click());
                if n == current || item.hovered() {
                    ui.painter().rect_filled(r, 3.0, Color32::from_rgb(228, 240, 255));
                }
                shape(ui.painter(), r.left_center() + Vec2::new(16.0, 0.0), n, 9.0);
                let at = r.left_center() + Vec2::new(34.0, 0.0);
                ui.painter().text(at, egui::Align2::LEFT_CENTER, format!("{n} sides"), FontId::proportional(12.0), Color32::from_gray(40));
                if item.clicked() {
                    self.world().poly_sides = n;
                }
            }
        });
    }

    /// The open swatch's choices, unfolded under its row when it's one of
    /// `row` (a popup couldn't reach past the palette's own small window).
    fn unfold(&mut self, ui: &mut egui::Ui, row: &[Swatch]) {
        let Some(which) = self.open_swatch.filter(|w| row.contains(w)) else { return };
        ui.add_space(GAP);
        egui::Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(ROW - 10.0);
            if self.swatch_choices(ui, which) {
                self.open_swatch = None;
            }
        });
    }

    /// The Options menu's settings that go on and off, as buttons that show
    /// whether they're on. Each chooses its menu item, so scripts' doMenu
    /// handlers see it, and a menu item a stack disables disables its button.
    fn option_toggles(&mut self, ui: &mut egui::Ui) {
        let (grid, enabled) = {
            let w = self.world();
            let enabled: Vec<bool> = OPTION_TOGGLES.iter().map(|(item, _, _)| menu_item_enabled(&w, item)).collect();
            (w.grid, enabled)
        };
        let on = [grid, self.fat.is_some()];
        let size = cell_size(OPTION_TOGGLES.len(), 26.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for (i, (item, icon, tip)) in OPTION_TOGGLES.iter().enumerate() {
                let button = egui::Button::selectable(on[i], RichText::new(*icon).size(15.0));
                let resp = ui.add_enabled_ui(enabled[i], |ui| ui.add_sized(size, button)).inner;
                if resp.on_hover_text(*tip).clicked() {
                    self.do_menu(item);
                }
            }
        });
    }

    /// One of the palette's swatches: what's chosen now, drawn. Clicking it
    /// opens or closes its choices.
    fn swatch(&mut self, ui: &mut egui::Ui, which: Swatch, size: Vec2) {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        self.draw_swatch(ui, rect, which);
        let hover = if resp.hovered() { Stroke::new(2.0, Color32::from_rgb(0, 90, 200)) } else { Stroke::new(1.0, Color32::GRAY) };
        ui.painter().rect_stroke(rect, 2.0, hover, StrokeKind::Inside);
        let open = self.open_swatch == Some(which);
        if open {
            ui.painter().rect_stroke(rect, 2.0, Stroke::new(2.0, Color32::from_rgb(0, 90, 200)), StrokeKind::Inside);
        }
        let resp = resp.on_hover_text(which.tip());
        if resp.clicked() {
            self.open_swatch = if open { None } else { Some(which) };
            // Closing or leaving the pattern swatch drops an edit.
            self.pattern_edit = None;
        }
        // As HyperCard's double-click on a pattern: edit it.
        if which == Swatch::Pattern && resp.double_clicked() {
            let n = self.world().pattern;
            self.edit_pattern(n);
        }
    }

    fn draw_swatch(&mut self, ui: &mut egui::Ui, rect: Rect, which: Swatch) {
        let (pattern, patterns, line_color, fill_color) = {
            let w = self.world();
            (w.pattern, w.stack.patterns.clone(), w.line_color, w.fill_color)
        };
        let painter = ui.painter().with_clip_rect(rect.shrink(1.0));
        painter.rect_filled(rect, 2.0, Color32::WHITE);
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
        }
    }

    /// A swatch's choices, in its popup; true when one was taken.
    fn swatch_choices(&mut self, ui: &mut egui::Ui, which: Swatch) -> bool {
        match which {
            Swatch::Pattern if self.pattern_edit.is_some() => self.pattern_editor(ui),
            Swatch::Pattern => self.pattern_choices(ui),
            Swatch::Color(slot) => self.color_choices(ui, slot),
        }
    }

    fn pattern_choices(&mut self, ui: &mut egui::Ui) -> bool {
        let (pattern, patterns, fill_color) = {
            let w = self.world();
            (w.pattern, w.stack.patterns.clone(), w.fill_color)
        };
        let textures = self.pattern_textures(ui.ctx(), &patterns, fill_color);
        let mut chosen = false;
        // Eight across, five down.
        let size = Vec2::new(((ROW - 10.0 - 7.0 * 3.0) / 8.0).floor(), 18.0);
        egui::Grid::new("pattern-choices").spacing([3.0, 3.0]).min_col_width(0.0).show(ui, |ui| {
            for (i, tex) in textures.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(size.x / 8.0, size.y / 8.0));
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
                if i % 8 == 7 {
                    ui.end_row();
                }
            }
        });
        chosen
    }

    /// Start editing pattern `n` (from 1), on the pattern swatch.
    pub(super) fn edit_pattern(&mut self, n: usize) {
        let bits = self.world().stack.pattern(n);
        self.pattern_edit = Some(PatternEdit { index: n.clamp(1, 40) - 1, bits, picking: false, painting: None, preview: None });
        self.open_swatch = Some(Swatch::Pattern);
    }

    /// HyperCard's pattern editor: click a square to turn it on or off
    /// (dragging sets the rest the same way), or click the card to take the
    /// pattern there. OK keeps it with the stack; Cancel leaves it as it was.
    fn pattern_editor(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(edit) = &mut self.pattern_edit else { return true };
        ui.label(RichText::new(format!("Pattern {}", edit.index + 1)).small());
        const CELL: f32 = 14.0;
        let mut done = None;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(CELL * 8.0), Sense::click_and_drag());
            let cell_at = |p: Pos2| {
                let (x, y) = (((p.x - rect.left()) / CELL).floor() as i32, ((p.y - rect.top()) / CELL).floor() as i32);
                ((0..8).contains(&x) && (0..8).contains(&y)).then_some((x as usize, y as usize))
            };
            // The first square pressed decides whether a drag turns squares
            // on or off; a click just turns one over.
            if (resp.clicked() || resp.drag_started() || resp.dragged())
                && let Some((x, y)) = resp.interact_pointer_pos().and_then(cell_at)
            {
                let bit = 0x80u8 >> x;
                let on = *edit.painting.get_or_insert(edit.bits[y] & bit == 0);
                if on { edit.bits[y] |= bit } else { edit.bits[y] &= !bit }
            }
            if resp.clicked() || resp.drag_stopped() {
                edit.painting = None;
            }
            let painter = ui.painter();
            for (y, row) in edit.bits.iter().enumerate() {
                for x in 0..8 {
                    let r = Rect::from_min_size(rect.min + Vec2::new(x as f32 * CELL, y as f32 * CELL), Vec2::splat(CELL));
                    let on = row & (0x80 >> x) != 0;
                    painter.rect_filled(r.shrink(0.5), 0.0, if on { Color32::BLACK } else { Color32::WHITE });
                }
            }
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                // What's left of the unfolded swatch's width.
                let width = ROW - 10.0 - CELL * 8.0 - 6.0;
                // How it paints: at actual size above, doubled below.
                let (w, h) = (width as usize, 24usize);
                let mut img = egui::ColorImage::filled([w, h], Color32::WHITE);
                for py in 0..h {
                    for px in 0..w {
                        let scale = if py < h / 3 { 1 } else { 2 };
                        if edit.bits[(py / scale) % 8] & (0x80 >> ((px / scale) % 8)) != 0 {
                            img.pixels[py * w + px] = Color32::BLACK;
                        }
                    }
                }
                let tex = match &mut edit.preview {
                    Some(t) => {
                        t.set(img, egui::TextureOptions::NEAREST);
                        t.id()
                    }
                    None => edit.preview.insert(ui.ctx().load_texture("pattern-edit", img, egui::TextureOptions::NEAREST)).id(),
                };
                let (strip, _) = ui.allocate_exact_size(Vec2::new(w as f32, h as f32), Sense::hover());
                ui.painter().image(tex, strip, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                ui.painter().rect_stroke(strip, 0.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
                ui.spacing_mut().item_spacing.y = GAP;
                ui.add_space(2.0);
                let full = Vec2::new(width, 20.0);
                if ui.add_sized(full, egui::Button::new("Invert")).clicked() {
                    edit.bits = edit.bits.map(|row| !row);
                }
                let picking = edit.picking;
                if ui
                    .add_sized(full, egui::Button::new("Card…").selected(picking))
                    .on_hover_text("Then click the card: the pattern there, around the click")
                    .clicked()
                {
                    edit.picking = !picking;
                }
                if ui.add_sized(full, egui::Button::new("OK")).clicked() {
                    done = Some(true);
                }
                if ui.add_sized(full, egui::Button::new("Cancel")).clicked() {
                    done = Some(false);
                }
            });
        });
        match done {
            Some(true) => {
                let (index, bits) = (edit.index, edit.bits);
                self.pattern_edit = None;
                let mut w = self.world();
                if w.stack.patterns.len() < 40 {
                    let missing: Vec<[u8; 8]> = paint::DEFAULT_PATTERNS[w.stack.patterns.len()..].to_vec();
                    w.stack.patterns.extend(missing);
                }
                w.stack.patterns[index] = bits;
                w.pattern = index + 1;
                w.dirty = true;
                true
            }
            Some(false) => {
                self.pattern_edit = None;
                true
            }
            None => false,
        }
    }

    /// The pattern editor's "From card": a click on the card takes the 8×8
    /// pattern around it, lined up as patterns paint, so it paints just as
    /// it looks there. Anything darker than mid-gray is a set bit.
    pub(super) fn pattern_pick_input(&mut self, resp: &egui::Response, origin: Pos2) {
        if !resp.clicked() {
            return;
        }
        let Some(q) = resp.interact_pointer_pos() else { return };
        let (cx, cy) = to_card(self.fat, origin, q);
        let mut bits = [0u8; 8];
        {
            let w = self.world();
            let card = w.stack.card(w.card_id).map(|c| &c.picture);
            let bg = w.stack.background(w.bg_id()).map(|b| &b.picture);
            for y in cy - 4..cy + 4 {
                for x in cx - 4..cx + 4 {
                    let at = |p: Option<&Picture>| p.map(|p| p.get(x, y)).unwrap_or(Pixel::CLEAR);
                    let c = at(card);
                    let p = paint::paper(if c.is_clear() { at(bg) } else { c });
                    let [r, g, b, _] = p.0;
                    if (r as u32 + g as u32 + b as u32) < 3 * 128 {
                        bits[y.rem_euclid(8) as usize] |= 0x80 >> x.rem_euclid(8);
                    }
                }
            }
        }
        if let Some(edit) = &mut self.pattern_edit {
            edit.bits = bits;
            edit.picking = false;
        }
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
        if let Some(c) = color_picker(ui, color32(current)) {
            self.set_paint_color(slot, pixel(c), false);
        }
        let mut chosen = false;
        let recent = self.recent_colors.clone();
        egui::Grid::new("recent-colors").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
            for color in recent.iter() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(((ROW - 10.0 - 9.0 * 2.0) / 10.0).floor(), 16.0), Sense::click());
                ui.painter().rect_filled(rect, 0.0, color32(*color));
                ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
                if resp.clicked() {
                    self.set_paint_color(slot, *color, false);
                    chosen = true;
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
        // Laid out to fit the palette's width: lines in two columns, the
        // brushes eight across, the polygons in a row.
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
                egui::Grid::new("line-sizes").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                    for (i, size) in [1u32, 2, 3, 4, 6, 8].into_iter().enumerate() {
                        let draw = |p: &egui::Painter, r: Rect| {
                            p.rect_filled(Rect::from_center_size(r.center(), Vec2::new(r.width() - 16.0, size as f32)), 0.0, Color32::BLACK);
                        };
                        if choice(ui, Vec2::new(((ROW - 12.0) / 2.0).floor(), 18.0), size == line_size, format!("{size}"), &draw) {
                            self.world().line_size = size;
                        }
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
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
                        if choice(ui, Vec2::splat(((ROW - 10.0 - 7.0 * 2.0) / 8.0).floor()), current, name.to_string(), &draw) {
                            self.world().brush = n;
                        }
                        if i % 8 == 7 {
                            ui.end_row();
                        }
                    }
                });
            }
            PaintDialog::PolygonSides => {
                egui::Grid::new("polygon-sides").spacing([2.0, 2.0]).min_col_width(0.0).show(ui, |ui| {
                    for n in [3u32, 4, 5, 6, 8, 12] {
                        let draw = |p: &egui::Painter, r: Rect| {
                            let c = r.center();
                            // Standing on a side, like the tool's icon.
                            let turn = if n.is_multiple_of(4) { std::f32::consts::PI / n as f32 } else { 0.0 };
                            let corner = ((13.0 * turn.sin()).round() as i32, (-13.0 * turn.cos()).round() as i32);
                            let mut pts: Vec<Pos2> = paint::regular_polygon_points((0, 0), corner, n)
                                .iter()
                                .map(|q| c + Vec2::new(q.0 as f32, q.1 as f32 + 1.0))
                                .collect();
                            if let Some(first) = pts.first().copied() {
                                pts.push(first);
                            }
                            p.add(egui::Shape::line(pts, Stroke::new(1.5, Color32::BLACK)));
                        };
                        if choice(ui, Vec2::splat(((ROW - 10.0 - 5.0 * 2.0) / 6.0).floor()), n == sides, format!("{n} sides"), &draw) {
                            self.world().poly_sides = n;
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

    /// The pointer over the card is the tool, as in HyperCard: the brush
    /// as its shape at its real size, the eraser's square and the spray's
    /// circle, and the pencil and bucket with their tips where paint
    /// lands. Lines, shapes and selecting get a crosshair, and text an
    /// I-beam.
    pub(super) fn tool_cursor(&mut self, ctx: &egui::Context, painter: &egui::Painter, resp: &egui::Response, origin: Pos2, tool: Tool) {
        let over = resp.contains_pointer() || resp.dragged();
        let Some(at) = ctx.input(|i| i.pointer.latest_pos()).filter(|_| over) else { return };
        let fat = self.fat;
        let zoom = if fat.is_some() { FAT_ZOOM } else { 1.0 };
        let (cx, cy) = to_card(fat, origin, at);
        // A card pixel's square on the screen.
        let cell = |x: i32, y: i32| Rect::from_min_size(to_screen(fat, origin, x as f32, y as f32), Vec2::splat(zoom));
        let (brush, fill, spray) = {
            let w = self.world();
            (w.brush, w.fill_color, w.spray_size)
        };
        let halo = Color32::from_white_alpha(220);
        match tool {
            Tool::Brush => {
                ctx.set_cursor_icon(egui::CursorIcon::None);
                let cells: Vec<Rect> = paint::brush_shape(brush).into_iter().map(|(dx, dy)| cell(cx + dx, cy + dy)).collect();
                // As a mesh: egui softens the edges of shapes, and a
                // softened rectangle a pixel wide all but disappears.
                let mut mesh = egui::Mesh::default();
                for r in &cells {
                    mesh.add_colored_rect(r.expand(1.0), halo);
                }
                for r in &cells {
                    mesh.add_colored_rect(*r, color32(fill));
                }
                painter.add(mesh);
            }
            Tool::Eraser => {
                // The eraser's 16 pixels square, as it erases.
                ctx.set_cursor_icon(egui::CursorIcon::None);
                let r = Rect::from_min_max(cell(cx - 7, cy - 7).min, cell(cx + 8, cy + 8).max);
                painter.rect(r, 0.0, Color32::WHITE, Stroke::new(1.0, Color32::BLACK), StrokeKind::Outside);
            }
            Tool::Spray => {
                ctx.set_cursor_icon(egui::CursorIcon::None);
                let c = cell(cx, cy).center();
                let radius = spray as f32 / 2.0 * zoom;
                painter.circle_stroke(c, radius, Stroke::new(3.0, halo));
                painter.circle_stroke(c, radius, Stroke::new(1.0, Color32::from_gray(40)));
                painter.circle_filled(c, 1.5, Color32::from_gray(40));
            }
            Tool::Pencil | Tool::Bucket => {
                // The tool's icon, its tip (on the drawing's 32 grid) at the
                // pointer.
                ctx.set_cursor_icon(egui::CursorIcon::None);
                const SIZE: f32 = 24.0;
                let tip = if tool == Tool::Pencil { Vec2::new(7.5, 24.5) } else { Vec2::new(5.0, 27.5) };
                let look = super::tool_icons::Look::default();
                if let Some(tex) = self.tool_icons.get(ctx, tool, look, SIZE) {
                    let min = at - tip * (SIZE / 32.0);
                    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                    painter.image(tex, Rect::from_min_size(min, Vec2::splat(SIZE)), uv, Color32::WHITE);
                }
            }
            Tool::Text => ctx.set_cursor_icon(egui::CursorIcon::Text),
            _ => ctx.set_cursor_icon(egui::CursorIcon::Crosshair),
        }
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
