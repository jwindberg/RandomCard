//! Drawing cards: picture layers, buttons and fields in a black-and-white,
//! HyperCard-like style, plus visual-effect transitions between cards.

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, TextFormat,
    TextureHandle, TextureOptions, Vec2, text::LayoutJob,
};

use crate::model::{Icon, Id, Layer, Part, PartKind, PartStyle, Picture, Pixel, TextAlign};
use crate::runtime::world::{Found, ObjRef, World};

pub const BLACK: Color32 = Color32::BLACK;
pub const WHITE: Color32 = Color32::WHITE;
pub const GRAY: Color32 = Color32::from_gray(150);

#[derive(Clone)]
pub struct PartView {
    pub obj: ObjRef,
    pub part: Part,
    pub text: String,
    pub hilite: bool,
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
    pub found: Option<Found>,
    /// The icons this card's buttons show.
    pub icons: HashMap<i32, Icon>,
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
                    hilite: s.hilite(card, layer, p.id),
                    part,
                });
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
            icons,
        }
    }

    pub fn part_at(&self, x: i32, y: i32) -> Option<&PartView> {
        self.parts.iter().rev().find(|p| p.part.visible && p.part.rect.contains(x, y))
    }
}

/// GPU textures for picture layers, refreshed when pictures change.
#[derive(Default)]
pub struct Textures {
    map: HashMap<(Layer, Id), (u64, Option<TextureHandle>)>,
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
        for (layer, id) in [(Layer::Background, scene.bg), (Layer::Card, scene.card)] {
            if self.map.get(&(layer, id)).is_some_and(|(r, _)| *r == rev) {
                continue;
            }
            let pic = match layer {
                Layer::Background => w.stack.background(id).map(|b| &b.picture),
                Layer::Card => w.stack.card(id).map(|c| &c.picture),
            };
            let Some(pic) = pic else { continue };
            let handle = if pic.is_blank() {
                None
            } else {
                let img = picture_image(pic);
                let name = format!("pict-{layer:?}-{id}");
                Some(match self.map.remove(&(layer, id)) {
                    Some((_, Some(mut h))) => {
                        h.set(img, TextureOptions::NEAREST);
                        h
                    }
                    _ => ctx.load_texture(name, img, TextureOptions::NEAREST),
                })
            };
            self.map.insert((layer, id), (rev, handle));
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

    fn get(&self, layer: Layer, id: Id) -> Option<&TextureHandle> {
        self.map.get(&(layer, id)).and_then(|(_, h)| h.as_ref())
    }
}

pub fn font_for(part: &Part) -> FontId {
    let font = part.text_font.to_ascii_lowercase();
    let family = if font.contains("courier") || font.contains("monaco") || font.contains("mono") {
        FontFamily::Monospace
    } else {
        FontFamily::Proportional
    };
    FontId::new(part.text_size.clamp(6, 128) as f32, family)
}

fn text_format(part: &Part, color: Color32) -> TextFormat {
    let style = part.text_style.to_ascii_lowercase();
    let mut f = TextFormat { font_id: font_for(part), color, ..Default::default() };
    f.italics = style.contains("italic");
    if style.contains("underline") {
        f.underline = Stroke::new(1.0, color);
    }
    if style.contains("strike") {
        f.strikethrough = Stroke::new(1.0, color);
    }
    f
}

fn is_bold(part: &Part) -> bool {
    part.text_style.to_ascii_lowercase().contains("bold")
}

/// Draw a scene with its top-left corner at `origin`.
pub fn draw_scene(painter: &Painter, tex: &Textures, scene: &Scene, origin: Pos2, pressed: Option<ObjRef>) {
    let card_rect = Rect::from_min_size(origin, Vec2::new(scene.width as f32, scene.height as f32));
    painter.rect_filled(card_rect, 0.0, WHITE);
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    if scene.show_bg_pict
        && let Some(t) = tex.get(Layer::Background, scene.bg)
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
            draw_part(painter, tex, pv, origin, pressed == Some(pv.obj), scene.found.as_ref());
        }
    }
    if !card_pict_drawn {
        draw_card_picture(painter, tex, scene, card_rect, uv);
    }
}

fn draw_card_picture(painter: &Painter, tex: &Textures, scene: &Scene, card_rect: Rect, uv: Rect) {
    if scene.show_card_pict
        && let Some(t) = tex.get(Layer::Card, scene.card)
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

pub fn draw_part(painter: &Painter, tex: &Textures, pv: &PartView, origin: Pos2, pressed: bool, found: Option<&Found>) {
    let rect = part_rect(&pv.part, origin);
    match pv.part.kind {
        PartKind::Button => draw_button(painter, tex, pv, rect, pressed),
        PartKind::Field => draw_field(painter, pv, rect, found),
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
    let (fg, bg) = if inverted && !only_icon_inverts { (WHITE, BLACK) } else { (BLACK, WHITE) };
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
            painter.rect_filled(boxr, 0.0, if pressed { Color32::from_gray(200) } else { WHITE });
            painter.rect_stroke(boxr, 0.0, stroke, StrokeKind::Inside);
            if pv.hilite {
                painter.line_segment([boxr.left_top(), boxr.right_bottom()], stroke);
                painter.line_segment([boxr.right_top(), boxr.left_bottom()], stroke);
            }
        } else {
            painter.circle_filled(boxr.center(), size / 2.0, if pressed { Color32::from_gray(200) } else { WHITE });
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
        draw_icon_and_name(&painter, tex, p, rect, inverted, fg);
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
fn draw_icon_and_name(painter: &Painter, tex: &Textures, p: &Part, rect: Rect, inverted: bool, fg: Color32) {
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
            painter.rect_filled(Rect::from_min_size(pos - Vec2::new(2.0, 0.0), g.size() + Vec2::new(4.0, 0.0)), 0.0, WHITE);
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
pub const SCROLLBAR: f32 = 16.0;

pub fn field_text_rect(p: &Part, rect: Rect) -> Rect {
    let margin = if p.wide_margins { FIELD_MARGIN + 6.0 } else { FIELD_MARGIN };
    let mut r = rect.shrink2(Vec2::new(margin, 2.0));
    if p.style == PartStyle::Scrolling {
        r.max.x -= SCROLLBAR;
    }
    r
}

/// Lay out field text, optionally highlighting a found range of characters.
pub fn field_job(p: &Part, text: &str, width: f32, found: Option<(usize, usize)>) -> LayoutJob {
    let mut job = LayoutJob::default();
    let fmt = text_format(p, BLACK);
    match found {
        Some((s, e)) => {
            let bs = text.char_indices().nth(s).map(|(i, _)| i).unwrap_or(text.len());
            let be = text.char_indices().nth(e).map(|(i, _)| i).unwrap_or(text.len());
            job.append(&text[..bs], 0.0, fmt.clone());
            let mut hl = fmt.clone();
            hl.background = BLACK;
            hl.color = WHITE;
            job.append(&text[bs..be], 0.0, hl);
            job.append(&text[be..], 0.0, fmt);
        }
        None => job.append(text, 0.0, fmt),
    }
    job.wrap.max_width = if p.dont_wrap { f32::INFINITY } else { width };
    job.halign = match p.text_align {
        TextAlign::Left => egui::Align::LEFT,
        TextAlign::Center => egui::Align::Center,
        TextAlign::Right => egui::Align::RIGHT,
    };
    job
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

pub fn draw_field_frame(painter: &Painter, p: &Part, rect: Rect) {
    let stroke = Stroke::new(1.0, BLACK);
    match p.style {
        PartStyle::Transparent => {}
        PartStyle::Opaque => {
            painter.rect_filled(rect, 0.0, WHITE);
        }
        PartStyle::Shadow => {
            shadowed(painter, rect, 0.0, 2.0);
            painter.rect_filled(rect, 0.0, WHITE);
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
        _ => {
            painter.rect_filled(rect, 0.0, WHITE);
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
    }
}

fn draw_field(painter: &Painter, pv: &PartView, rect: Rect, found: Option<&Found>) {
    let p = &pv.part;
    draw_field_frame(painter, p, rect);
    let text_rect = field_text_rect(p, rect);
    let hl = found.filter(|f| pv.obj == (ObjRef::Part { layer: f.layer, id: f.field, card: f.card }));
    let job = field_job(p, &pv.text, text_rect.width(), hl.map(|f| (f.start, f.end)));
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
    let pos = Pos2::new(aligned_x(p, text_rect), text_rect.top() - scroll);
    clip.galley(pos, galley.clone(), BLACK);
    if is_bold(p) {
        clip.galley(pos + Vec2::new(0.6, 0.0), galley.clone(), BLACK);
    }
    if p.style == PartStyle::Scrolling {
        draw_scrollbar(painter, rect, scroll, galley.size().y, text_rect.height());
    }
}

/// Where a scrolling field's scroll bar and thumb are.
pub struct Scrollbar {
    pub bar: Rect,
    /// None when all the text fits.
    pub thumb: Option<Rect>,
    pub max_scroll: f32,
    /// How far the thumb's top can move.
    pub travel: f32,
}

pub fn scrollbar(rect: Rect, scroll: f32, content_h: f32, view_h: f32) -> Scrollbar {
    let bar = Rect::from_min_max(Pos2::new(rect.right() - SCROLLBAR, rect.top()), rect.max);
    let track = bar.height() - 2.0;
    if content_h <= view_h || track <= 0.0 {
        return Scrollbar { bar, thumb: None, max_scroll: 0.0, travel: 0.0 };
    }
    let thumb_h = (track * view_h / content_h).clamp(12.0_f32.min(track), track);
    let max_scroll = content_h - view_h;
    let travel = track - thumb_h;
    let y = bar.top() + 1.0 + travel * (scroll / max_scroll).clamp(0.0, 1.0);
    let thumb = Rect::from_min_size(Pos2::new(bar.left() + 2.0, y), Vec2::new(SCROLLBAR - 4.0, thumb_h));
    Scrollbar { bar, thumb: Some(thumb), max_scroll, travel }
}

pub fn draw_scrollbar(painter: &Painter, rect: Rect, scroll: f32, content_h: f32, view_h: f32) {
    let sb = scrollbar(rect, scroll, content_h, view_h);
    painter.rect_filled(sb.bar, 0.0, Color32::from_gray(230));
    painter.rect_stroke(sb.bar, 0.0, Stroke::new(1.0, BLACK), StrokeKind::Inside);
    if let Some(thumb) = sb.thumb {
        painter.rect_filled(thumb, 2.0, WHITE);
        painter.rect_stroke(thumb, 2.0, Stroke::new(1.0, BLACK), StrokeKind::Inside);
    }
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
    let draw_new_in = |r: Rect| draw_scene(&clipped(r), tex, new, origin, None);
    match name {
        "dissolve" => {
            draw_scene(&clipped(full), tex, old, origin, None);
            let mut p = clipped(full);
            p.set_opacity(t);
            draw_scene(&p, tex, new, origin, None);
        }
        "wipe" => {
            draw_scene(&clipped(full), tex, old, origin, None);
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
            draw_scene(&clipped(full), tex, old, origin + off, None);
            draw_scene(&clipped(full), tex, new, origin + off + incoming, None);
        }
        "barn" => {
            draw_scene(&clipped(full), tex, old, origin, None);
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
                draw_scene(&clipped(full), tex, new, origin, None);
                let r = Rect::from_center_size(full.center(), size * (1.0 - t));
                draw_scene(&clipped(r), tex, old, origin, None);
            } else {
                draw_scene(&clipped(full), tex, old, origin, None);
                draw_new_in(Rect::from_center_size(full.center(), size * t));
            }
        }
        "venetian" => {
            draw_scene(&clipped(full), tex, old, origin, None);
            let strips = 12;
            let sh = h / strips as f32;
            for i in 0..strips {
                let top = full.top() + i as f32 * sh;
                draw_new_in(Rect::from_min_max(Pos2::new(full.left(), top), Pos2::new(full.right(), top + sh * t)));
            }
        }
        "checkerboard" => {
            draw_scene(&clipped(full), tex, old, origin, None);
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
        _ => draw_scene(&clipped(full), tex, new, origin, None),
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
