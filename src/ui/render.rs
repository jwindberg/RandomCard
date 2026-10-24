//! Drawing cards: picture layers, buttons and fields in a black-and-white,
//! HyperCard-like style, plus visual-effect transitions between cards.

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, TextFormat,
    TextureHandle, TextureOptions, Vec2, text::LayoutJob,
};

use crate::model::styles::{self, StyleRun};
use crate::model::{Icon, Id, Layer, Part, PartKind, PartStyle, Picture, Pixel, TextAlign};
use crate::runtime::world::{FieldRange, ObjRef, World};

use super::addcolor::{self, ColorShape};

pub const BLACK: Color32 = Color32::BLACK;
pub const WHITE: Color32 = Color32::WHITE;
pub const GRAY: Color32 = Color32::from_gray(150);

#[derive(Clone)]
pub struct PartView {
    pub obj: ObjRef,
    pub part: Part,
    pub text: String,
    /// The style runs of a field's text.
    pub styles: Vec<StyleRun>,
    pub hilite: bool,
    /// What its white is filled with: white, or its AddColor color.
    pub fill: Color32,
}

/// Everything needed to draw one card, copied out of the world.
#[derive(Clone)]
pub struct Scene {
    pub card: Id,
    pub bg: Id,
    pub width: u32,
    pub height: u32,
    pub show_bg_pict: bool,
    pub show_card_pict: bool,
    pub edit_bg: bool,
    /// Background parts first, then card parts, each in z-order.
    pub parts: Vec<PartView>,
    pub found: Option<FieldRange>,
    pub selection: Option<FieldRange>,
    /// The icons this card's buttons show.
    pub icons: HashMap<i32, Icon>,
    /// AddColor shapes, and a key for them (0 when there are none).
    pub colors: Vec<ColorShape>,
    pub color_key: u64,
}

impl Scene {
    pub fn capture(w: &World) -> Scene {
        let s = &w.stack;
        let card = w.card_id;
        let c = s.card(card);
        let bg = c.map(|c| c.background).unwrap_or(0);
        let mut parts = Vec::new();
        let layers: &[Layer] = if w.edit_bg { &[Layer::Background] } else { &[Layer::Background, Layer::Card] };
        for &layer in layers {
            for p in s.parts(card, layer) {
                let mut part = p.clone();
                // Scripts aren't needed for drawing; don't copy them every frame.
                part.script = String::new();
                parts.push(PartView {
                    obj: ObjRef::Part { layer, id: p.id, card },
                    text: if p.kind == PartKind::Field || p.contents.is_empty() {
                        s.field_text(card, layer, p.id)
                    } else {
                        p.contents.clone()
                    },
                    styles: if p.kind == PartKind::Field { s.field_styles(card, layer, p.id) } else { Vec::new() },
                    hilite: s.hilite(card, layer, p.id),
                    fill: WHITE,
                    part,
                });
            }
        }
        let colors = addcolor::shapes(s, card, w.edit_bg);
        if !colors.is_empty() {
            for pv in &mut parts {
                let (x, y) = pv.part.rect.center();
                pv.fill = addcolor::color_at(&colors, x, y);
            }
        }
        let icons = parts
            .iter()
            .filter(|pv| pv.part.kind == PartKind::Button && pv.part.icon != 0)
            .filter_map(|pv| Some((pv.part.icon, s.icons.get(&pv.part.icon)?.clone())))
            .collect();
        Scene {
            card,
            bg,
            width: s.width,
            height: s.height,
            show_bg_pict: s.background(bg).is_some_and(|b| b.show_pict),
            show_card_pict: !w.edit_bg && c.is_some_and(|c| c.show_pict),
            edit_bg: w.edit_bg,
            parts,
            found: w.found.clone().filter(|f| f.card == card),
            selection: w.current_selection().cloned(),
            icons,
            color_key: addcolor::key(&colors),
            colors,
        }
    }

    /// The characters hilited in a field: its selection, or found text.
    pub fn highlight(&self, obj: ObjRef) -> Option<(usize, usize)> {
        let in_field = |r: &&FieldRange| obj == (ObjRef::Part { layer: r.layer, id: r.field, card: r.card });
        let selected = self.selection.as_ref().filter(in_field).filter(|r| r.start < r.end);
        selected.or(self.found.as_ref().filter(in_field)).map(|r| (r.start, r.end))
    }

    pub fn part_at(&self, x: i32, y: i32) -> Option<&PartView> {
        self.parts.iter().rev().find(|p| p.part.visible && p.part.rect.contains(x, y))
    }
}

/// GPU textures for picture layers, refreshed when pictures change.
#[derive(Default)]
pub struct Textures {
    /// By layer, id and AddColor key, with the picture revision drawn.
    map: HashMap<(Layer, Id, u64), (u64, Option<TextureHandle>)>,
    /// AddColor maps, by key, for the card's own white.
    color_maps: HashMap<u64, TextureHandle>,
    /// Icon textures, normal and inverted (for hilited buttons).
    icons: HashMap<i32, (Icon, TextureHandle, TextureHandle)>,
}

pub fn picture_image(p: &Picture) -> egui::ColorImage {
    let mut img = egui::ColorImage::filled([p.width as usize, p.height as usize], Color32::TRANSPARENT);
    for (i, px) in p.pixels.iter().enumerate() {
        img.pixels[i] = match px {
            Pixel::Clear => Color32::TRANSPARENT,
            Pixel::White => WHITE,
            Pixel::Black => BLACK,
        };
    }
    img
}

impl Textures {
    /// Make sure the textures for `scene` reflect the current pictures.
    pub fn refresh(&mut self, ctx: &egui::Context, w: &World, scene: &Scene) {
        self.refresh_icons(ctx, scene);
        let rev = w.picture_rev;
        let ck = scene.color_key;
        // Each card can have its own colors; keep only a few cards' worth.
        if self.map.len() > 8 {
            self.map.retain(|(_, _, k), _| *k == 0 || *k == ck);
        }
        if self.color_maps.len() > 4 {
            self.color_maps.retain(|k, _| *k == ck);
        }
        let stale: Vec<(Layer, Id)> = [(Layer::Background, scene.bg), (Layer::Card, scene.card)]
            .into_iter()
            .filter(|&(layer, id)| !self.map.get(&(layer, id, ck)).is_some_and(|(r, _)| *r == rev))
            .collect();
        // White in the pictures takes the AddColor color under it.
        let colors = (ck != 0 && (!stale.is_empty() || !self.color_maps.contains_key(&ck)))
            .then(|| addcolor::color_map(&scene.colors, scene.width, scene.height));
        if let Some(map) = &colors
            && !self.color_maps.contains_key(&ck)
        {
            let img = egui::ColorImage::new([scene.width as usize, scene.height as usize], map.clone());
            self.color_maps.insert(ck, ctx.load_texture(format!("addcolor-{ck}"), img, TextureOptions::NEAREST));
        }
        for (layer, id) in stale {
            let pic = match layer {
                Layer::Background => w.stack.background(id).map(|b| &b.picture),
                Layer::Card => w.stack.card(id).map(|c| &c.picture),
            };
            let Some(pic) = pic else { continue };
            let handle = if pic.is_blank() {
                None
            } else {
                let mut img = picture_image(pic);
                if let Some(map) = &colors
                    && map.len() == img.pixels.len()
                {
                    for (px, c) in img.pixels.iter_mut().zip(map) {
                        if *px == WHITE {
                            *px = *c;
                        }
                    }
                }
                let name = format!("pict-{layer:?}-{id}-{ck}");
                Some(match self.map.remove(&(layer, id, ck)) {
                    Some((_, Some(mut h))) => {
                        h.set(img, TextureOptions::NEAREST);
                        h
                    }
                    _ => ctx.load_texture(name, img, TextureOptions::NEAREST),
                })
            };
            self.map.insert((layer, id, ck), (rev, handle));
        }
    }

    fn icon(&self, id: i32, inverted: bool) -> Option<&TextureHandle> {
        self.icons.get(&id).map(|(_, normal, inv)| if inverted { inv } else { normal })
    }

    fn refresh_icons(&mut self, ctx: &egui::Context, scene: &Scene) {
        for (id, icon) in &scene.icons {
            if self.icons.get(id).is_some_and(|(cached, _, _)| cached == icon) {
                continue;
            }
            let normal = picture_image(&icon.picture);
            let mut inverted = normal.clone();
            for px in &mut inverted.pixels {
                if px.a() > 0 {
                    *px = if *px == BLACK { WHITE } else { BLACK };
                }
            }
            let opts = TextureOptions::NEAREST;
            let n = ctx.load_texture(format!("icon-{id}"), normal, opts);
            let i = ctx.load_texture(format!("icon-{id}-inv"), inverted, opts);
            self.icons.insert(*id, (icon.clone(), n, i));
        }
    }

    fn get(&self, layer: Layer, id: Id, color_key: u64) -> Option<&TextureHandle> {
        self.map.get(&(layer, id, color_key)).and_then(|(_, h)| h.as_ref())
    }
}

/// The nearest built-in font: monospaced for Courier and Monaco.
fn font_id(font: &str, size: u32) -> FontId {
    let font = font.to_ascii_lowercase();
    let family = if font.contains("courier") || font.contains("monaco") || font.contains("mono") {
        FontFamily::Monospace
    } else {
        FontFamily::Proportional
    };
    FontId::new(size.clamp(6, 128) as f32, family)
}

pub fn font_for(part: &Part) -> FontId {
    font_id(&part.text_font, part.text_size)
}

/// How text in a font, size and style looks. Bold is drawn as a second,
/// offset pass (see `field_job`); outline and shadow aren't drawn.
fn format_for(font: &str, size: u32, style: &str, color: Color32) -> TextFormat {
    let mut f = TextFormat { font_id: font_id(font, size), color, ..Default::default() };
    f.italics = styles::has_style(style, "italic");
    if styles::has_style(style, "underline") {
        f.underline = Stroke::new(1.0, color);
    }
    if styles::has_style(style, "condense") {
        f.extra_letter_spacing = -0.6;
    } else if styles::has_style(style, "extend") {
        f.extra_letter_spacing = 1.0;
    }
    f
}

fn text_format(part: &Part, color: Color32) -> TextFormat {
    format_for(&part.text_font, part.text_size, &part.text_style, color)
}

fn is_bold(part: &Part) -> bool {
    part.text_style.to_ascii_lowercase().contains("bold")
}

/// Draw a scene with its top-left corner at `origin`.
/// Draw a scene. `live` leaves scrolling fields' text to scroll area widgets.
pub fn draw_scene(painter: &Painter, tex: &Textures, scene: &Scene, origin: Pos2, pressed: Option<ObjRef>, live: bool) {
    let card_rect = Rect::from_min_size(origin, Vec2::new(scene.width as f32, scene.height as f32));
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    match tex.color_maps.get(&scene.color_key) {
        Some(t) => painter.image(t.id(), card_rect, uv, WHITE),
        None => painter.rect_filled(card_rect, 0.0, WHITE),
    };
    if scene.show_bg_pict
        && let Some(t) = tex.get(Layer::Background, scene.bg, scene.color_key)
    {
        painter.image(t.id(), card_rect, uv, WHITE);
    }
    let mut card_pict_drawn = false;
    for pv in &scene.parts {
        let is_card = matches!(pv.obj, ObjRef::Part { layer: Layer::Card, .. });
        if is_card && !card_pict_drawn {
            card_pict_drawn = true;
            draw_card_picture(painter, tex, scene, card_rect, uv);
        }
        if pv.part.visible {
            draw_part(painter, tex, pv, origin, pressed == Some(pv.obj), scene.highlight(pv.obj), live);
        }
    }
    if !card_pict_drawn {
        draw_card_picture(painter, tex, scene, card_rect, uv);
    }
}

fn draw_card_picture(painter: &Painter, tex: &Textures, scene: &Scene, card_rect: Rect, uv: Rect) {
    if scene.show_card_pict
        && let Some(t) = tex.get(Layer::Card, scene.card, scene.color_key)
    {
        painter.image(t.id(), card_rect, uv, WHITE);
    }
}

pub fn part_rect(part: &Part, origin: Pos2) -> Rect {
    let r = part.rect;
    Rect::from_min_max(
        origin + Vec2::new(r.left as f32, r.top as f32),
        origin + Vec2::new(r.right as f32, r.bottom as f32),
    )
}

pub fn draw_part(
    painter: &Painter,
    tex: &Textures,
    pv: &PartView,
    origin: Pos2,
    pressed: bool,
    highlight: Option<(usize, usize)>,
    live: bool,
) {
    let rect = part_rect(&pv.part, origin);
    match pv.part.kind {
        PartKind::Button => draw_button(painter, tex, pv, rect, pressed),
        PartKind::Field => draw_field(painter, pv, rect, highlight, live),
    }
}

fn shadowed(painter: &Painter, rect: Rect, radius: f32, shadow: f32) {
    if shadow > 0.0 {
        painter.rect_filled(rect.translate(Vec2::splat(shadow)), radius, BLACK);
    }
}

fn draw_button(painter: &Painter, tex: &Textures, pv: &PartView, rect: Rect, pressed: bool) {
    let p = &pv.part;
    let painter = painter.with_clip_rect(rect.expand(4.0).intersect(painter.clip_rect()));
    let checkable = matches!(p.style, PartStyle::CheckBox | PartStyle::Radio);
    let inverted = (pv.hilite || pressed) && !checkable;
    // A hilited transparent icon button inverts only its icon.
    let has_icon = p.icon != 0 && !checkable;
    let only_icon_inverts = has_icon && matches!(p.style, PartStyle::Transparent | PartStyle::Oval);
    let (fg, bg) = if inverted && !only_icon_inverts { (WHITE, BLACK) } else { (BLACK, pv.fill) };
    let fg = if p.enabled { fg } else { GRAY };
    let stroke = Stroke::new(1.0, BLACK);
    match p.style {
        PartStyle::Transparent => {
            if inverted && !has_icon {
                painter.rect_filled(rect, 0.0, BLACK);
            }
        }
        PartStyle::Opaque => {
            painter.rect_filled(rect, 0.0, bg);
        }
        PartStyle::Rectangle => {
            painter.rect_filled(rect, 0.0, bg);
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
        PartStyle::Shadow => {
            shadowed(&painter, rect, 0.0, 2.0);
            painter.rect_filled(rect, 0.0, bg);
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
        PartStyle::RoundRect => {
            shadowed(&painter, rect, 8.0, 1.0);
            painter.rect_filled(rect, 8.0, bg);
            painter.rect_stroke(rect, 8.0, stroke, StrokeKind::Inside);
        }
        PartStyle::Default => {
            painter.rect_stroke(rect, 10.0, Stroke::new(3.0, BLACK), StrokeKind::Inside);
            let inner = rect.shrink(4.0);
            painter.rect_filled(inner, 6.0, bg);
            painter.rect_stroke(inner, 6.0, stroke, StrokeKind::Inside);
        }
        PartStyle::Oval => {
            let c = rect.center();
            let radius = rect.size() / 2.0;
            painter.add(egui::epaint::EllipseShape { center: c, radius, fill: bg, stroke, angle: 0.0 });
        }
        PartStyle::CheckBox | PartStyle::Radio | PartStyle::Scrolling => {}
    }
    if checkable {
        let size = 12.0;
        let boxr = Rect::from_min_size(Pos2::new(rect.left() + 4.0, rect.center().y - size / 2.0), Vec2::splat(size));
        if p.style == PartStyle::CheckBox {
            painter.rect_filled(boxr, 0.0, if pressed { Color32::from_gray(200) } else { pv.fill });
            painter.rect_stroke(boxr, 0.0, stroke, StrokeKind::Inside);
            if pv.hilite {
                painter.line_segment([boxr.left_top(), boxr.right_bottom()], stroke);
                painter.line_segment([boxr.right_top(), boxr.left_bottom()], stroke);
            }
        } else {
            painter.circle_filled(boxr.center(), size / 2.0, if pressed { Color32::from_gray(200) } else { pv.fill });
            painter.circle_stroke(boxr.center(), size / 2.0, stroke);
            if pv.hilite {
                painter.circle_filled(boxr.center(), size / 4.0, BLACK);
            }
        }
        if p.show_name {
            let pos = Pos2::new(boxr.right() + 5.0, rect.center().y);
            draw_label(&painter, p, pos, Align2::LEFT_CENTER, &p.name, fg);
        }
        return;
    }
    if has_icon {
        draw_icon_and_name(&painter, tex, p, rect, inverted, fg, pv.fill);
        return;
    }
    if p.show_name && !p.name.is_empty() {
        let (pos, align) = match p.text_align {
            TextAlign::Left => (Pos2::new(rect.left() + 6.0, rect.center().y), Align2::LEFT_CENTER),
            TextAlign::Right => (Pos2::new(rect.right() - 6.0, rect.center().y), Align2::RIGHT_CENTER),
            TextAlign::Center => (rect.center(), Align2::CENTER_CENTER),
        };
        draw_label(&painter, p, pos, align, &p.name, fg);
    }
}

pub const ICON_SIZE: f32 = 32.0;

/// HyperCard's icon button layout: the icon and a 9-point name beneath it,
/// centred together. A missing icon leaves its space empty, as in HyperCard.
fn draw_icon_and_name(painter: &Painter, tex: &Textures, p: &Part, rect: Rect, inverted: bool, fg: Color32, fill: Color32) {
    let label_part = Part { text_size: 9, text_style: "plain".into(), text_font: "Geneva".into(), ..p.clone() };
    let label = (p.show_name && !p.name.is_empty()).then(|| {
        painter.layout_job(LayoutJob::single_section(p.name.clone(), text_format(&label_part, fg)))
    });
    let block_h = match &label {
        Some(g) => ICON_SIZE + 1.0 + g.size().y + 1.0,
        None => ICON_SIZE + 2.0,
    };
    let top = (rect.top() + rect.height() / 2.0 - block_h / 2.0 + 1.0).floor();
    let icon_rect = Rect::from_min_size(Pos2::new((rect.center().x - ICON_SIZE / 2.0).floor(), top), Vec2::splat(ICON_SIZE));
    if let Some(t) = tex.icon(p.icon, inverted) {
        let tint = if p.enabled { WHITE } else { Color32::from_white_alpha(110) };
        painter.image(t.id(), icon_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
    }
    if let Some(g) = label {
        let x = match p.text_align {
            TextAlign::Left => rect.left() + 4.0,
            TextAlign::Right => rect.right() - 4.0 - g.size().x,
            TextAlign::Center => (rect.center().x - g.size().x / 2.0).floor(),
        };
        let pos = Pos2::new(x, icon_rect.bottom() + 1.0);
        if p.style == PartStyle::Transparent {
            // The name gets a white backdrop so it reads over the picture.
            painter.rect_filled(Rect::from_min_size(pos - Vec2::new(2.0, 0.0), g.size() + Vec2::new(4.0, 0.0)), 0.0, fill);
        }
        painter.galley(pos, g, fg);
    }
}

fn draw_label(painter: &Painter, p: &Part, pos: Pos2, align: Align2, text: &str, color: Color32) {
    let job = LayoutJob::single_section(text.to_string(), text_format(p, color));
    let galley = painter.layout_job(job);
    let r = align.anchor_size(pos, galley.size());
    painter.galley(r.min, galley.clone(), color);
    if is_bold(p) {
        painter.galley(r.min + Vec2::new(0.6, 0.0), galley, color);
    }
}

pub const FIELD_MARGIN: f32 = 4.0;
/// Room kept free at the right of a scrolling field for its scroll bar.
pub const SCROLLBAR: f32 = 14.0;

/// Where a scrolling field's scroll area goes: the text area plus the room
/// for its scroll bar.
pub fn scroll_view_rect(p: &Part, rect: Rect) -> Rect {
    let text = field_text_rect(p, rect);
    Rect::from_min_max(text.min, Pos2::new(rect.right() - 1.0, text.bottom()))
}

pub fn field_text_rect(p: &Part, rect: Rect) -> Rect {
    let margin = if p.wide_margins { FIELD_MARGIN + 6.0 } else { FIELD_MARGIN };
    let mut r = rect.shrink2(Vec2::new(margin, 2.0));
    if p.style == PartStyle::Scrolling {
        r.max.x -= SCROLLBAR;
    }
    r
}

/// Lay out field text with its style runs, optionally hiliting a range of
/// characters. The built-in fonts have no bold weight, so bold text is drawn
/// twice: `bold_pass` lays out the same text with only the bold runs
/// visible, to draw again slightly to the right.
pub fn field_job(
    p: &Part,
    text: &str,
    runs: &[StyleRun],
    width: f32,
    hl: Option<(usize, usize)>,
    bold_pass: bool,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let bytes: Vec<usize> = text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())).collect();
    let len = bytes.len() - 1;
    let mut cuts: Vec<usize> = vec![0, len];
    cuts.extend(runs.iter().map(|r| r.start.min(len)));
    if let Some((s, e)) = hl {
        cuts.extend([s.min(len), e.min(len)]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let run = styles::style_at(runs, a);
        let style = run.style.as_deref().unwrap_or(&p.text_style);
        let lit = hl.is_some_and(|(s, e)| a >= s && a < e);
        let ink = if lit { WHITE } else { BLACK };
        let color = match bold_pass {
            false => ink,
            true if styles::has_style(style, "bold") => ink,
            true => Color32::TRANSPARENT,
        };
        let mut fmt = format_for(run.font.as_deref().unwrap_or(&p.text_font), run.size.unwrap_or(p.text_size), style, color);
        if lit && !bold_pass {
            fmt.background = BLACK;
        }
        job.append(&text[bytes[a]..bytes[b]], 0.0, fmt);
    }
    if text.is_empty() {
        job.append("", 0.0, text_format(p, BLACK));
    }
    job.wrap.max_width = if p.dont_wrap { f32::INFINITY } else { width };
    job.halign = match p.text_align {
        TextAlign::Left => egui::Align::LEFT,
        TextAlign::Center => egui::Align::Center,
        TextAlign::Right => egui::Align::RIGHT,
    };
    job
}

/// Whether any of a field's text is bold.
pub fn has_bold(p: &Part, runs: &[StyleRun]) -> bool {
    let bold = |s: &str| styles::has_style(s, "bold");
    runs.iter().any(|r| r.style.style.as_deref().map_or(bold(&p.text_style), bold)) || (bold(&p.text_style) && runs.first().is_none_or(|r| r.start > 0))
}

/// Draw a field's laid-out text at `pos`, thickening its bold runs.
pub fn paint_field_text(painter: &Painter, pv: &PartView, galley: std::sync::Arc<egui::Galley>, pos: Pos2, width: f32, hl: Option<(usize, usize)>) {
    painter.galley(pos, galley, BLACK);
    if has_bold(&pv.part, &pv.styles) {
        let bold = painter.layout_job(field_job(&pv.part, &pv.text, &pv.styles, width, hl, true));
        painter.galley(pos + Vec2::new(0.6, 0.0), bold, BLACK);
    }
}

/// List fields (autoSelect) hilite whole lines, across the field.
pub fn hilite_lines(painter: &Painter, p: &Part, galley: &egui::Galley, pos: Pos2, text_rect: Rect, hl: Option<(usize, usize)>) {
    let Some((start, end)) = hl.filter(|_| p.auto_select) else { return };
    let top = galley.pos_from_cursor(egui::text::CCursor::new(start)).top();
    let bottom = galley.pos_from_cursor(egui::text::CCursor::new(end)).bottom();
    let rows = Rect::from_x_y_ranges(text_rect.x_range(), pos.y + top..=pos.y + bottom);
    painter.rect_filled(rows, 0.0, BLACK);
}

/// Where a field's text starts on screen, allowing for its scroll.
fn field_text_origin(p: &Part, text_rect: Rect) -> Pos2 {
    let scroll = if p.style == PartStyle::Scrolling { p.scroll as f32 } else { 0.0 };
    Pos2::new(aligned_x(p, text_rect), text_rect.top() - scroll)
}

/// The character under a point in a field, as `draw_field` lays it out, or
/// None below the last line.
pub fn field_char_at(painter: &Painter, pv: &PartView, origin: Pos2, pos: Pos2) -> Option<usize> {
    let p = &pv.part;
    let text_rect = field_text_rect(p, part_rect(p, origin));
    let galley = painter.layout_job(field_job(p, &pv.text, &pv.styles, text_rect.width(), None, false));
    let rel = pos - field_text_origin(p, text_rect);
    if rel.y > galley.size().y {
        return None;
    }
    // The nearest cursor may sit after the character the pointer is over.
    let cursor = galley.cursor_from_pos(rel);
    let at = galley.pos_from_cursor(cursor).left();
    let i = cursor.index.0;
    Some(if rel.x < at && i > 0 { i - 1 } else { i })
}

/// The bottom left of a character in a field, on the card.
pub fn field_char_point(painter: &Painter, pv: &PartView, at: usize) -> Pos2 {
    let p = &pv.part;
    let text_rect = field_text_rect(p, part_rect(p, Pos2::ZERO));
    let galley = painter.layout_job(field_job(p, &pv.text, &pv.styles, text_rect.width(), None, false));
    field_text_origin(p, text_rect) + galley.pos_from_cursor(egui::text::CCursor::new(at)).left_bottom().to_vec2()
}

/// Where to place a galley laid out with the part's alignment: egui lays out
/// centered and right-aligned text around x = 0.
pub fn aligned_x(p: &Part, r: Rect) -> f32 {
    match p.text_align {
        TextAlign::Left => r.left(),
        TextAlign::Center => r.center().x,
        TextAlign::Right => r.right(),
    }
}

/// A field's frame and background; `fill` is its white (or AddColor color).
pub fn draw_field_frame(painter: &Painter, p: &Part, rect: Rect, fill: Color32) {
    let stroke = Stroke::new(1.0, BLACK);
    match p.style {
        PartStyle::Transparent => {}
        PartStyle::Opaque => {
            painter.rect_filled(rect, 0.0, fill);
        }
        PartStyle::Shadow => {
            shadowed(painter, rect, 0.0, 2.0);
            painter.rect_filled(rect, 0.0, fill);
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
        _ => {
            painter.rect_filled(rect, 0.0, fill);
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
    }
}

/// Draw a field. With `live` set, a scrolling field gets only its frame: its
/// text is shown in a scroll area widget by the UI.
fn draw_field(painter: &Painter, pv: &PartView, rect: Rect, highlight: Option<(usize, usize)>, live: bool) {
    let p = &pv.part;
    draw_field_frame(painter, p, rect, pv.fill);
    if live && p.style == PartStyle::Scrolling {
        return;
    }
    let text_rect = field_text_rect(p, rect);
    let job = field_job(p, &pv.text, &pv.styles, text_rect.width(), highlight, false);
    let galley = painter.layout_job(job);
    // Clip to the whole field so large text isn't cut by the margins.
    let clip_rect = if p.style == PartStyle::Scrolling { text_rect } else { rect.shrink(1.0) };
    let clip = painter.with_clip_rect(clip_rect.intersect(painter.clip_rect()));
    let scroll = if p.style == PartStyle::Scrolling { p.scroll as f32 } else { 0.0 };
    let line_h = font_for(p).size * 1.25;
    if p.show_lines {
        let mut y = text_rect.top() + line_h - scroll;
        let dotted = Stroke::new(1.0, GRAY);
        while y < text_rect.bottom() {
            if y > text_rect.top() {
                clip.line_segment([Pos2::new(text_rect.left(), y), Pos2::new(text_rect.right(), y)], dotted);
            }
            y += line_h;
        }
    }
    let pos = field_text_origin(p, text_rect);
    hilite_lines(&clip, p, &galley, pos, text_rect, highlight);
    let content_h = galley.size().y;
    paint_field_text(&clip, pv, galley, pos, text_rect.width(), highlight);
    if p.style == PartStyle::Scrolling {
        draw_scroll_indicator(painter, scroll_view_rect(p, rect), scroll, content_h);
    }
}

/// A still picture of a scroll bar, for when the field isn't live (during
/// visual effects and with the authoring tools): a slim thumb like egui's.
fn draw_scroll_indicator(painter: &Painter, area: Rect, scroll: f32, content_h: f32) {
    let view_h = area.height();
    if content_h <= view_h || view_h <= 0.0 {
        return;
    }
    let thumb_h = (view_h * view_h / content_h).max(12.0_f32.min(view_h));
    let y = area.top() + (view_h - thumb_h) * (scroll / (content_h - view_h)).clamp(0.0, 1.0);
    let thumb = Rect::from_min_size(Pos2::new(area.right() - 8.0, y), Vec2::new(6.0, thumb_h));
    painter.rect_filled(thumb, 3.0, Color32::from_gray(150));
}

/// Draw `old` transitioning to `new` with a HyperCard visual effect, `t` in 0..1.
pub fn draw_transition(
    painter: &Painter,
    tex: &Textures,
    old: &Scene,
    new: &Scene,
    origin: Pos2,
    effect: &str,
    t: f32,
) {
    let size = Vec2::new(new.width as f32, new.height as f32);
    let full = Rect::from_min_size(origin, size);
    let clipped = |r: Rect| painter.with_clip_rect(r.intersect(full));
    let (w, h) = (size.x, size.y);
    let words: Vec<&str> = effect.split_whitespace().collect();
    let has = |s: &str| words.contains(&s);
    let dir = if has("left") {
        "left"
    } else if has("right") {
        "right"
    } else if has("up") {
        "up"
    } else if has("down") {
        "down"
    } else if has("close") || has("out") || effect.contains("shrink") {
        "close"
    } else {
        "open"
    };
    let name = words.first().copied().unwrap_or("plain");
    let draw_new_in = |r: Rect| draw_scene(&clipped(r), tex, new, origin, None, false);
    match name {
        "dissolve" => {
            draw_scene(&clipped(full), tex, old, origin, None, false);
            let mut p = clipped(full);
            p.set_opacity(t);
            draw_scene(&p, tex, new, origin, None, false);
        }
        "wipe" => {
            draw_scene(&clipped(full), tex, old, origin, None, false);
            let r = match dir {
                "left" => Rect::from_min_max(Pos2::new(full.right() - w * t, full.top()), full.max),
                "right" => Rect::from_min_max(full.min, Pos2::new(full.left() + w * t, full.bottom())),
                "up" => Rect::from_min_max(Pos2::new(full.left(), full.bottom() - h * t), full.max),
                _ => Rect::from_min_max(full.min, Pos2::new(full.right(), full.top() + h * t)),
            };
            draw_new_in(r);
        }
        "scroll" | "push" => {
            let off = match dir {
                "left" => Vec2::new(-w * t, 0.0),
                "right" => Vec2::new(w * t, 0.0),
                "up" => Vec2::new(0.0, -h * t),
                _ => Vec2::new(0.0, h * t),
            };
            let incoming = match dir {
                "left" => Vec2::new(w, 0.0),
                "right" => Vec2::new(-w, 0.0),
                "up" => Vec2::new(0.0, h),
                _ => Vec2::new(0.0, -h),
            };
            draw_scene(&clipped(full), tex, old, origin + off, None, false);
            draw_scene(&clipped(full), tex, new, origin + off + incoming, None, false);
        }
        "barn" => {
            draw_scene(&clipped(full), tex, old, origin, None, false);
            if dir == "close" {
                draw_new_in(Rect::from_min_max(full.min, Pos2::new(full.left() + w * t / 2.0, full.bottom())));
                draw_new_in(Rect::from_min_max(Pos2::new(full.right() - w * t / 2.0, full.top()), full.max));
            } else {
                let c = full.center().x;
                draw_new_in(Rect::from_min_max(
                    Pos2::new(c - w * t / 2.0, full.top()),
                    Pos2::new(c + w * t / 2.0, full.bottom()),
                ));
            }
        }
        "iris" | "zoom" | "stretch" | "shrink" => {
            if dir == "close" {
                draw_scene(&clipped(full), tex, new, origin, None, false);
                let r = Rect::from_center_size(full.center(), size * (1.0 - t));
                draw_scene(&clipped(r), tex, old, origin, None, false);
            } else {
                draw_scene(&clipped(full), tex, old, origin, None, false);
                draw_new_in(Rect::from_center_size(full.center(), size * t));
            }
        }
        "venetian" => {
            draw_scene(&clipped(full), tex, old, origin, None, false);
            let strips = 12;
            let sh = h / strips as f32;
            for i in 0..strips {
                let top = full.top() + i as f32 * sh;
                draw_new_in(Rect::from_min_max(Pos2::new(full.left(), top), Pos2::new(full.right(), top + sh * t)));
            }
        }
        "checkerboard" => {
            draw_scene(&clipped(full), tex, old, origin, None, false);
            let (cols, rows) = (8, 6);
            let (cw, ch) = (w / cols as f32, h / rows as f32);
            for row in 0..rows {
                for col in 0..cols {
                    let first = (row + col) % 2 == 0;
                    let p = if first { (t * 2.0).min(1.0) } else { (t * 2.0 - 1.0).max(0.0) };
                    if p <= 0.0 {
                        continue;
                    }
                    let min = Pos2::new(full.left() + col as f32 * cw, full.top() + row as f32 * ch);
                    draw_new_in(Rect::from_min_size(min, Vec2::new(cw, ch * p)));
                }
            }
        }
        _ => draw_scene(&clipped(full), tex, new, origin, None, false),
    }
}

/// Dashed outline used by the button and field tools.
pub fn dashed_rect(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.0, color);
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    for pair in corners.windows(2) {
        let shapes = egui::Shape::dashed_line(&[pair[0], pair[1]], stroke, 4.0, 3.0);
        for s in shapes {
            painter.add(s);
        }
    }
}

pub fn selection_handles(painter: &Painter, rect: Rect) {
    for c in [rect.left_top(), rect.right_top(), rect.left_bottom(), rect.right_bottom()] {
        let r = Rect::from_center_size(c, Vec2::splat(6.0));
        painter.rect_filled(r, CornerRadius::ZERO, BLACK);
    }
}
