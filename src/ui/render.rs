//! Drawing cards: picture layers, buttons and fields in a black-and-white,
//! HyperCard-like style, plus visual-effect transitions between cards.

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, TextFormat,
    TextureHandle, TextureOptions, Vec2, text::LayoutJob,
};

use crate::model::styles::{self, StyleRun};
use crate::model::{Icon, Id, Layer, POPUP_TEXT_INSET, Part, PartKind, PartStyle, Picture, Pixel, TextAlign, bitmap_font, metrics};
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
        Self::capture_card(w, w.card_id, w.edit_bg)
    }

    /// Any card of the stack, as browsing shows it (or just its background).
    pub fn capture_card(w: &World, card: Id, edit_bg: bool) -> Scene {
        let s = &w.stack;
        let c = s.card(card);
        let bg = c.map(|c| c.background).unwrap_or(0);
        let mut parts = Vec::new();
        let layers: &[Layer] = if edit_bg { &[Layer::Background] } else { &[Layer::Background, Layer::Card] };
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
        let colors = addcolor::shapes(w, card, edit_bg);
        for pv in &mut parts {
            if let Some(c) = pv.part.back_color {
                pv.fill = rgb(c);
            } else if !colors.is_empty() {
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
            show_card_pict: !edit_bg && c.is_some_and(|c| c.show_pict),
            edit_bg,
            parts,
            found: w.found.clone().filter(|f| f.card == card && card == w.card_id),
            selection: w.current_selection().cloned().filter(|_| card == w.card_id),
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
    /// Pictures with black and white swapped, for the cards on screen that
    /// have a button that can invert them (see `inverts`).
    inverted: HashMap<(Layer, Id, u64), (u64, TextureHandle)>,
}

pub fn picture_image(p: &Picture) -> egui::ColorImage {
    let mut img = egui::ColorImage::filled([p.width as usize, p.height as usize], Color32::TRANSPARENT);
    for (i, px) in p.iter().enumerate() {
        img.pixels[i] = match px {
            Pixel::CLEAR => Color32::TRANSPARENT,
            Pixel::WHITE => WHITE,
            Pixel::BLACK => BLACK,
            Pixel([r, g, b, a]) => Color32::from_rgba_unmultiplied(r, g, b, a),
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
        self.refresh_inverted(ctx, w, scene);
    }

    /// Inverted copies of the scene's pictures, if a button on it can invert
    /// them. AddColor's colors aren't inverted: they show as plain white.
    fn refresh_inverted(&mut self, ctx: &egui::Context, w: &World, scene: &Scene) {
        if !scene.parts.iter().any(|pv| can_invert(&pv.part)) {
            return;
        }
        let rev = w.picture_rev;
        let ck = scene.color_key;
        if self.inverted.len() > 4 {
            self.inverted.retain(|(l, id, _), _| (*l, *id) == (Layer::Background, scene.bg) || (*l, *id) == (Layer::Card, scene.card));
        }
        for (layer, id) in [(Layer::Background, scene.bg), (Layer::Card, scene.card)] {
            if self.inverted.get(&(layer, id, ck)).is_some_and(|(r, _)| *r == rev) {
                continue;
            }
            let pic = match layer {
                Layer::Background => w.stack.background(id).map(|b| &b.picture),
                Layer::Card => w.stack.card(id).map(|c| &c.picture),
            };
            let Some(pic) = pic.filter(|p| !p.is_blank()) else { continue };
            let mut img = picture_image(pic);
            for px in &mut img.pixels {
                *px = invert_color(*px);
            }
            let handle = ctx.load_texture(format!("pict-{layer:?}-{id}-{ck}-inv"), img, TextureOptions::NEAREST);
            self.inverted.insert((layer, id, ck), (rev, handle));
        }
    }

    /// Each texture with an inverted copy, mapped to that copy.
    fn inverse_ids(&self) -> HashMap<egui::TextureId, egui::TextureId> {
        let mut ids = HashMap::new();
        for (key, (rev, inv)) in &self.inverted {
            if let Some((r, Some(h))) = self.map.get(key)
                && r == rev
            {
                ids.insert(h.id(), inv.id());
            }
        }
        for (_, n, i) in self.icons.values() {
            ids.insert(n.id(), i.id());
            ids.insert(i.id(), n.id());
        }
        ids
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
pub(super) fn font_id(font: &str, size: u32) -> FontId {
    // A stack's own bitmap font.
    if let Some(family) = super::bitmap_fonts::family(font, size) {
        return FontId::new(size.clamp(1, 256) as f32, family);
    }
    let font = font.to_ascii_lowercase();
    let family = if font.contains("courier") || font.contains("monaco") || font.contains("mono") {
        FontFamily::Monospace
    } else {
        FontFamily::Proportional
    };
    FontId::new(size.clamp(6, 128) as f32, family)
}

/// How text in a font, size and style looks. Bold is drawn as a second,
/// offset pass, and outline and shadow as edges under white letters (see
/// `field_job`).
fn format_for(font: &str, size: u32, style: &str, color: Color32) -> TextFormat {
    let mut f = TextFormat { font_id: font_id(font, size), color, ..Default::default() };
    f.italics = styles::has_style(style, "italic");
    if styles::has_style(style, "underline") {
        f.underline = Stroke::new(1.0, color);
    }
    f
}

thread_local! {
    /// egui's glyph widths, by family, size and character.
    static GLYPH_WIDTHS: std::cell::RefCell<HashMap<(FontFamily, u32, char), f32>> = Default::default();
}

fn glyph_width(ctx: &egui::Context, font: &FontId, c: char) -> f32 {
    let key = (font.family.clone(), font.size.to_bits(), c);
    if let Some(w) = GLYPH_WIDTHS.with(|g| g.borrow().get(&key).copied()) {
        return w;
    }
    let w = ctx.fonts_mut(|f| f.glyph_width(font, c));
    GLYPH_WIDTHS.with(|g| g.borrow_mut().insert(key, w));
    w
}

thread_local! {
    /// The egui size that best matches a HyperCard font and size.
    static MATCHED_SIZES: std::cell::RefCell<HashMap<(String, u32), f32>> = Default::default();
}

/// The size to draw a HyperCard font at so that its text is about as wide
/// as it was in HyperCard, leaving less for letter spacing to make up.
fn matched_size(ctx: &egui::Context, font: &str, size: u32, id: &FontId) -> f32 {
    if bitmap_font::font(font, size).is_some() {
        return id.size;
    }
    let key = (font.to_ascii_lowercase(), size);
    if let Some(s) = MATCHED_SIZES.with(|m| m.borrow().get(&key).copied()) {
        return s;
    }
    const SAMPLE: &str = "The quick brown fox jumps over the lazy dog, 1234567890.";
    let theirs = metrics::metrics(font, size, "plain").text_width(SAMPLE);
    let ours: f32 = SAMPLE.chars().map(|c| glyph_width(ctx, id, c)).sum();
    let ratio = if ours > 0.0 { (theirs / ours).clamp(0.85, 1.2) } else { 1.0 };
    // Half sizes keep the glyph cache small.
    let matched = (id.size * ratio * 2.0).round() / 2.0;
    MATCHED_SIZES.with(|m| m.borrow_mut().insert(key, matched));
    matched
}

/// Append text spaced by HyperCard's character widths, so lines break
/// where they did. Words keep their own letter spacing, squeezed only when
/// they'd run past where HyperCard put them; the spaces between words
/// widen to bring the next word to its place. (A space never starts a
/// wrapped line, so the extra room can't indent one.)
fn append_spaced(ctx: &egui::Context, job: &mut LayoutJob, text: &str, m: &metrics::Metrics, fmt: TextFormat) {
    super::system_fonts::note_text(text);
    // A stack's own bitmap font is drawn with HyperCard's widths already.
    if m.bitmap.is_some() {
        job.append(text, 0.0, fmt);
        return;
    }
    // Words and runs of spaces, each its own section; a newline ends a word.
    let mut tokens: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut prev_space = None;
    for (i, c) in text.char_indices() {
        let space = c == ' ' || c == '\t';
        if prev_space.is_some_and(|p| p != space) && start < i {
            tokens.push(&text[start..i]);
            start = i;
        }
        prev_space = Some(space);
        if c == '\n' {
            tokens.push(&text[start..i + 1]);
            start = i + 1;
            prev_space = None;
        }
    }
    if start < text.len() {
        tokens.push(&text[start..]);
    }
    let measured: Vec<Token> = tokens
        .iter()
        .map(|t| {
            let glyphs: Vec<char> = t.chars().filter(|c| *c != '\n').collect();
            Token {
                ours: glyphs.iter().map(|c| glyph_width(ctx, &fmt.font_id, *c)).sum(),
                theirs: m.text_width(t),
                word: glyphs.first().is_some_and(|c| *c != ' ' && *c != '\t'),
                glyphs: glyphs.len(),
                line_end: t.ends_with('\n'),
            }
        })
        .collect();
    for (token, (letter_spacing, lead)) in tokens.iter().zip(spacing(&measured)) {
        let mut f = fmt.clone();
        f.extra_letter_spacing = letter_spacing;
        job.append(token, lead, f);
    }
}

/// A word or run of spaces, measured: egui's width and HyperCard's.
struct Token {
    ours: f32,
    theirs: f32,
    word: bool,
    glyphs: usize,
    line_end: bool,
}

/// Letter spacing for each word and padding before each run of spaces, so
/// the text keeps HyperCard's widths: words are drawn a little tighter
/// where egui's font is wider, and spaces take up what's left. `drift` is
/// how far egui's pen is ahead of HyperCard's. A run of spaces pads by at
/// most its own width, so text that has fallen behind (a long run with no
/// spaces, such as JSON) catches up a little at each space rather than all
/// at once in one wide gap.
fn spacing(tokens: &[Token]) -> Vec<(f32, f32)> {
    let mut drift = 0.0;
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        let (mut letter_spacing, mut lead) = (0.0, 0.0);
        if t.word && t.glyphs > 1 {
            // egui adds the spacing between the glyphs of a section.
            let gaps = (t.glyphs - 1) as f32;
            letter_spacing = ((t.theirs - t.ours - drift) / gaps).clamp(-1.5, 0.0);
            drift += t.ours + letter_spacing * gaps - t.theirs;
        } else if t.word {
            drift += t.ours - t.theirs;
        } else {
            lead = (t.theirs - t.ours - drift).clamp(0.0, t.theirs.max(0.0));
            drift += lead + t.ours - t.theirs;
        }
        if t.line_end {
            drift = 0.0;
        }
        out.push((letter_spacing, lead));
    }
    out
}

fn text_format(part: &Part, color: Color32) -> TextFormat {
    format_for(&part.text_font, part.text_size, &part.text_style, color)
}

/// A one-line label in the part's font, spaced as HyperCard spaced it.
fn label_job(ctx: &egui::Context, part: &Part, text: &str, color: Color32) -> LayoutJob {
    let mut fmt = text_format(part, color);
    fmt.font_id.size = matched_size(ctx, &part.text_font, part.text_size, &fmt.font_id);
    let mut job = LayoutJob::default();
    append_spaced(ctx, &mut job, text, &metrics::metrics(&part.text_font, part.text_size, &part.text_style), fmt);
    job
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
    for (i, pv) in scene.parts.iter().enumerate() {
        let is_card = matches!(pv.obj, ObjRef::Part { layer: Layer::Card, .. });
        if is_card && !card_pict_drawn {
            card_pict_drawn = true;
            draw_card_picture(painter, tex, scene, card_rect, uv);
        }
        if pv.part.visible {
            if can_invert(&pv.part) && (pv.hilite || pressed == Some(pv.obj)) {
                invert_under(painter, tex, scene, i, origin, live);
            }
            draw_part(painter, tex, pv, origin, pressed == Some(pv.obj), scene.highlight(pv.obj), live);
        }
    }
    if !card_pict_drawn {
        draw_card_picture(painter, tex, scene, card_rect, uv);
    }
}

/// Draw again what lies above part `i`, clipped to it. A scrolling field is a
/// live widget drawn after the card, so whatever is layered over it (a
/// popup field, a button, the card picture over a background field) has to
/// be put back on top.
pub fn draw_over(painter: &Painter, tex: &Textures, scene: &Scene, i: usize, origin: Pos2, live: bool) {
    let rect = part_rect(&scene.parts[i].part, origin);
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let card_rect = Rect::from_min_size(origin, Vec2::new(scene.width as f32, scene.height as f32));
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    let mut card_pict_drawn = matches!(scene.parts[i].obj, ObjRef::Part { layer: Layer::Card, .. });
    for (j, pv) in scene.parts.iter().enumerate().skip(i + 1) {
        let is_card = matches!(pv.obj, ObjRef::Part { layer: Layer::Card, .. });
        if is_card && !card_pict_drawn {
            card_pict_drawn = true;
            draw_card_picture(&painter, tex, scene, card_rect, uv);
        }
        if pv.part.visible && part_rect(&pv.part, origin).intersects(rect) {
            if can_invert(&pv.part) && pv.hilite {
                invert_under(&painter, tex, scene, j, origin, live);
            }
            draw_part(&painter, tex, pv, origin, false, scene.highlight(pv.obj), live);
        }
    }
    if !card_pict_drawn {
        draw_card_picture(&painter, tex, scene, card_rect, uv);
    }
}

/// Whether a button inverts what's under it when hilited: in HyperCard a
/// hilited transparent button with no icon reverses black and white in its
/// rectangle, so stacks laid one over text to show it white on black.
fn can_invert(p: &Part) -> bool {
    p.visible && p.kind == PartKind::Button && p.style == PartStyle::Transparent && p.icon == 0
}

/// Draw what's under part `i` again, inside its rectangle, with black and
/// white reversed: the pictures and the parts below it, but not those above.
fn invert_under(painter: &Painter, tex: &Textures, scene: &Scene, i: usize, origin: Pos2, live: bool) {
    let rect = part_rect(&scene.parts[i].part, origin);
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let card_rect = Rect::from_min_size(origin, Vec2::new(scene.width as f32, scene.height as f32));
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    let layer = painter.layer_id();
    let start = painter.ctx().graphics_mut(|g| g.entry(layer).next_idx().0);
    painter.rect_filled(card_rect, 0.0, WHITE);
    if scene.show_bg_pict
        && let Some(t) = tex.get(Layer::Background, scene.bg, scene.color_key)
    {
        painter.image(t.id(), card_rect, uv, WHITE);
    }
    let mut card_pict_drawn = false;
    for pv in &scene.parts[..i] {
        let is_card = matches!(pv.obj, ObjRef::Part { layer: Layer::Card, .. });
        if is_card && !card_pict_drawn {
            card_pict_drawn = true;
            draw_card_picture(&painter, tex, scene, card_rect, uv);
        }
        if pv.part.visible {
            draw_part(&painter, tex, pv, origin, false, scene.highlight(pv.obj), live);
        }
    }
    let end = painter.ctx().graphics_mut(|g| g.entry(layer).next_idx().0);
    let swap = tex.inverse_ids();
    painter.ctx().graphics_mut(|g| {
        let list = g.entry(layer);
        for idx in start..end {
            list.mutate_shape(egui::layers::ShapeIdx(idx), |cs| invert_shape(&mut cs.shape, &swap));
        }
    });
}

/// Black for white, keeping transparency.
fn invert_color(c: Color32) -> Color32 {
    let a = c.a();
    Color32::from_rgba_premultiplied(a - c.r().min(a), a - c.g().min(a), a - c.b().min(a), a)
}

/// Reverse a shape's colors; a picture or icon changes to its inverted copy.
fn invert_shape(shape: &mut egui::Shape, swap: &HashMap<egui::TextureId, egui::TextureId>) {
    use egui::Shape;
    use egui::epaint::ColorMode;
    let path_stroke = |s: &mut egui::epaint::PathStroke| {
        if let ColorMode::Solid(c) = &mut s.color {
            *c = invert_color(*c);
        }
    };
    match shape {
        Shape::Vec(v) => v.iter_mut().for_each(|s| invert_shape(s, swap)),
        Shape::Circle(c) => (c.fill, c.stroke.color) = (invert_color(c.fill), invert_color(c.stroke.color)),
        Shape::Ellipse(e) => (e.fill, e.stroke.color) = (invert_color(e.fill), invert_color(e.stroke.color)),
        Shape::LineSegment { stroke, .. } => stroke.color = invert_color(stroke.color),
        Shape::Rect(r) => (r.fill, r.stroke.color) = (invert_color(r.fill), invert_color(r.stroke.color)),
        Shape::Band(b) => (b.fill, b.stroke.color) = (invert_color(b.fill), invert_color(b.stroke.color)),
        Shape::Path(p) => {
            p.fill = invert_color(p.fill);
            path_stroke(&mut p.stroke);
        }
        Shape::QuadraticBezier(q) => {
            q.fill = invert_color(q.fill);
            path_stroke(&mut q.stroke);
        }
        Shape::CubicBezier(c) => {
            c.fill = invert_color(c.fill);
            path_stroke(&mut c.stroke);
        }
        Shape::Text(t) => {
            let color = t.override_text_color.unwrap_or_else(|| {
                t.galley.job.sections.first().map(|s| s.format.color).filter(|c| *c != Color32::PLACEHOLDER).unwrap_or(t.fallback_color)
            });
            t.override_text_color = Some(invert_color(color));
        }
        Shape::Mesh(m) => {
            if let Some(to) = swap.get(&m.texture_id) {
                std::sync::Arc::make_mut(m).texture_id = *to;
            } else if m.texture_id == egui::TextureId::default() {
                for v in &mut std::sync::Arc::make_mut(m).vertices {
                    v.color = invert_color(v.color);
                }
            }
        }
        _ => {}
    }
}

fn draw_card_picture(painter: &Painter, tex: &Textures, scene: &Scene, card_rect: Rect, uv: Rect) {
    if scene.show_card_pict
        && let Some(t) = tex.get(Layer::Card, scene.card, scene.color_key)
    {
        painter.image(t.id(), card_rect, uv, WHITE);
    }
}

fn rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// A part's text color (`the foreColor`): black unless it has one.
pub fn text_color(p: &Part) -> Color32 {
    p.fore_color.map_or(BLACK, rgb)
}

/// A part's frame color (`the borderColor`): black unless it has one.
pub fn frame_color(p: &Part) -> Color32 {
    p.border_color.map_or(BLACK, rgb)
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
    // Hilited, the fill and text colors swap: white on black, in black and white.
    let (fg, bg) = if inverted && !only_icon_inverts {
        (p.back_color.map_or(WHITE, rgb), text_color(p))
    } else {
        (text_color(p), pv.fill)
    };
    let fg = if p.enabled { fg } else { GRAY };
    let stroke = Stroke::new(1.0, frame_color(p));
    match p.style {
        // A hilited one reverses what's under it (see `invert_under`).
        PartStyle::Transparent => {}
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
        PartStyle::Standard => {
            painter.rect_filled(rect, 8.0, bg);
            painter.rect_stroke(rect, 8.0, stroke, StrokeKind::Inside);
        }
        PartStyle::Popup => {
            draw_popup(&painter, p, rect, fg, bg);
            return;
        }
        PartStyle::Slider => {
            draw_slider(&painter, p, rect, pressed);
            return;
        }
        PartStyle::Default => {
            painter.rect_stroke(rect, 10.0, Stroke::new(3.0, frame_color(p)), StrokeKind::Inside);
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
                let mark = Stroke::new(1.0, text_color(p));
                painter.line_segment([boxr.left_top(), boxr.right_bottom()], mark);
                painter.line_segment([boxr.right_top(), boxr.left_bottom()], mark);
            }
        } else {
            painter.circle_filled(boxr.center(), size / 2.0, if pressed { Color32::from_gray(200) } else { pv.fill });
            painter.circle_stroke(boxr.center(), size / 2.0, stroke);
            if pv.hilite {
                painter.circle_filled(boxr.center(), size / 4.0, text_color(p));
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

/// A popup: its name in the title area at the left, then a framed and
/// shadowed menu box holding the chosen item and a downward triangle.
fn draw_popup(painter: &Painter, p: &Part, rect: Rect, fg: Color32, bg: Color32) {
    let at = |x: i32, y: i32| rect.min + Vec2::new((x - p.rect.left) as f32, (y - p.rect.top) as f32);
    let b = p.popup_box();
    let boxr = Rect::from_min_max(at(b.left, b.top), at(b.right, b.bottom));
    painter.rect_filled(boxr.translate(Vec2::splat(1.0)), 0.0, BLACK);
    painter.rect_filled(boxr, 0.0, bg);
    painter.rect_stroke(boxr, 0.0, Stroke::new(1.0, frame_color(p)), StrokeKind::Inside);
    let tri = p.popup_triangle().map(|(x, y)| at(x, y));
    painter.add(egui::Shape::convex_polygon(tri.to_vec(), fg, Stroke::NONE));
    if p.show_name && p.title_width > 0 && !p.name.is_empty() {
        let title = painter.with_clip_rect(Rect::from_min_max(rect.min, Pos2::new(boxr.left(), rect.max.y)));
        let label_fg = if p.enabled { text_color(p) } else { GRAY };
        draw_label(&title, p, Pos2::new(rect.left() + 2.0, boxr.center().y), Align2::LEFT_CENTER, &p.name, label_fg);
    }
    let text_rect = Rect::from_min_max(
        Pos2::new(boxr.left() + POPUP_TEXT_INSET as f32, boxr.top()),
        Pos2::new(tri[0].x - 2.0, boxr.bottom()),
    );
    let inside = painter.with_clip_rect(text_rect.intersect(painter.clip_rect()));
    draw_label(&inside, p, Pos2::new(text_rect.left(), boxr.center().y), Align2::LEFT_CENTER, p.selected_text(), fg);
}

/// A slider: a thin rounded track, filled from the min to the thumb in the
/// button's backColor (black if it has none), and a round white thumb with
/// a soft shadow, darkened while it's held. Its name, if shown, sits in its
/// titleWidth at the left, as a popup's does.
fn draw_slider(painter: &Painter, p: &Part, rect: Rect, pressed: bool) {
    let at = |x: i32, y: i32| rect.min + Vec2::new((x - p.rect.left) as f32, (y - p.rect.top) as f32);
    let (a, b) = p.slider_track();
    let along = a as f32 + (b - a) as f32 * p.slider_fraction() as f32;
    let vertical = p.slider_vertical();
    let fill = if p.enabled { p.back_color.map_or(BLACK, rgb) } else { GRAY };
    let empty = Color32::from_gray(205);
    let thickness = 4.0;
    let (start, end, thumb) = if vertical {
        let x = rect.center().x;
        (Pos2::new(x, at(0, a).y), Pos2::new(x, at(0, b).y), Pos2::new(x, rect.min.y + (along - p.rect.top as f32)))
    } else {
        let y = rect.center().y;
        (Pos2::new(at(a, 0).x, y), Pos2::new(at(b, 0).x, y), Pos2::new(rect.min.x + (along - p.rect.left as f32), y))
    };
    let bar = |from: Pos2, to: Pos2, color: Color32| {
        let r = Rect::from_two_pos(from, to).expand(thickness / 2.0);
        painter.rect_filled(r, thickness / 2.0, color);
    };
    bar(start, end, empty);
    bar(start, thumb, fill);
    let radius = p.slider_thumb_radius() as f32;
    painter.circle_filled(thumb + Vec2::new(0.0, 1.0), radius, Color32::from_black_alpha(60));
    painter.circle_filled(thumb, radius, if pressed { Color32::from_gray(225) } else { WHITE });
    painter.circle_stroke(thumb, radius, Stroke::new(1.0, if p.enabled { frame_color(p) } else { GRAY }));
    if !vertical && p.show_name && p.title_width > 0 && !p.name.is_empty() {
        let title = painter.with_clip_rect(Rect::from_min_max(rect.min, Pos2::new(rect.left() + p.title_width as f32, rect.max.y)));
        let label_fg = if p.enabled { text_color(p) } else { GRAY };
        draw_label(&title, p, Pos2::new(rect.left() + 2.0, rect.center().y), Align2::LEFT_CENTER, &p.name, label_fg);
    }
}

pub const ICON_SIZE: f32 = 32.0;

/// HyperCard's icon button layout: the icon and a 9-point name beneath it,
/// centered together. A missing icon leaves its space empty, as in HyperCard.
fn draw_icon_and_name(painter: &Painter, tex: &Textures, p: &Part, rect: Rect, inverted: bool, fg: Color32, fill: Color32) {
    let label_part = Part { text_size: 9, text_style: "plain".into(), text_font: "Geneva".into(), ..p.clone() };
    let label = (p.show_name && !p.name.is_empty()).then(|| {
        painter.layout_job(label_job(painter.ctx(), &label_part, &p.name, fg))
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
    let job = label_job(painter.ctx(), p, text, color);
    let galley = painter.layout_job(job);
    let r = align.anchor_size(pos, galley.size());
    painter.galley(r.min, galley.clone(), color);
    if is_bold(p) {
        painter.galley(r.min + Vec2::new(0.6, 0.0), galley, color);
    }
}

/// Room kept free at the right of a scrolling field for its scroll bar.
pub const SCROLLBAR: f32 = 16.0;

/// Where a scrolling field's scroll area goes: the text area plus the room
/// for its scroll bar.
pub fn scroll_view_rect(p: &Part, rect: Rect) -> Rect {
    let text = field_text_rect(p, rect);
    Rect::from_min_max(text.min, Pos2::new(rect.right() - 1.0, text.bottom()))
}

/// Where a field's text goes, inset from its frame as in HyperCard: a
/// 1-pixel frame, 3 pixels each side (wideMargins: 8 left, 6 right, and 4
/// at the top), less any shadow or scroll bar.
pub fn field_text_rect(p: &Part, rect: Rect) -> Rect {
    let wide = if p.wide_margins { 1.0 } else { 0.0 };
    let (mut right, mut bottom) = (4.0 + 3.0 * wide, 1.0);
    match p.style {
        PartStyle::Shadow => {
            right += 2.0;
            bottom += 2.0;
        }
        PartStyle::Scrolling => right += SCROLLBAR,
        _ => {}
    }
    // Text stays clear of a combo field's ▼.
    if p.choice_arrow().is_some() {
        right += crate::model::CHOICE_ARROW as f32;
    }
    Rect::from_min_max(rect.min + Vec2::new(4.0 + 5.0 * wide, 1.0 + 4.0 * wide), rect.max - Vec2::new(right, bottom))
}

/// Lay out field text with its style runs, optionally hiliting a range of
/// characters. The built-in fonts have no bold weight, so bold text is drawn
/// twice: `Pass::Bold` lays out the same text with only the bold runs
/// visible, to draw again slightly to the right.
pub fn field_job(
    ctx: &egui::Context,
    p: &Part,
    text: &str,
    runs: &[StyleRun],
    width: f32,
    hl: Option<(usize, usize)>,
    pass: Pass,
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
        let ink = if lit { WHITE } else { run.color.map_or_else(|| text_color(p), rgb) };
        let edged = styles::has_style(style, "outline") || styles::has_style(style, "shadow");
        let color = match pass {
            Pass::Plain => ink,
            // Outline and shadow letters are white, over their black edges.
            Pass::Shown if edged => if lit { BLACK } else { WHITE },
            Pass::Shown => ink,
            Pass::Bold if styles::has_style(style, "bold") => ink,
            Pass::Edge if edged => ink,
            Pass::Bold | Pass::Edge => Color32::TRANSPARENT,
        };
        let (font, size) = (run.font.as_deref().unwrap_or(&p.text_font), run.size.unwrap_or(p.text_size));
        let mut fmt = format_for(font, size, style, color);
        if lit && matches!(pass, Pass::Plain | Pass::Shown) {
            fmt.background = text_color(p);
        }
        let m = metrics::metrics(font, size, style);
        fmt.font_id.size = matched_size(ctx, font, size, &fmt.font_id);
        let line_height = if p.fixed_line_height { p.line_height() } else { m.line_height() };
        fmt.line_height = Some(line_height as f32);
        append_spaced(ctx, &mut job, &text[bytes[a]..bytes[b]], &m, fmt);
    }
    if text.is_empty() {
        let mut fmt = text_format(p, text_color(p));
        let m = metrics::metrics(&p.text_font, p.text_size, &p.text_style);
        fmt.line_height = Some(if p.fixed_line_height { p.line_height() } else { m.line_height() } as f32);
        job.append("", 0.0, fmt);
    }
    job.wrap.max_width = if p.dont_wrap { f32::INFINITY } else { width };
    job.halign = match p.text_align {
        TextAlign::Left => egui::Align::LEFT,
        TextAlign::Center => egui::Align::Center,
        TextAlign::Right => egui::Align::RIGHT,
    };
    job
}

/// Which drawing of a field's text a layout is for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// Every run in its ink: for editing, and for finding characters.
    Plain,
    /// As shown on the card: outline and shadow letters in white.
    Shown,
    /// Only the bold runs, drawn again a little to the right.
    Bold,
    /// Only the outline and shadow runs, drawn around their letters.
    Edge,
}

/// Whether any of a field's text has a style.
fn has_style_in(p: &Part, runs: &[StyleRun], name: &str) -> bool {
    let has = |s: &str| styles::has_style(s, name);
    runs.iter().any(|r| r.style.style.as_deref().map_or(has(&p.text_style), has)) || (has(&p.text_style) && runs.first().is_none_or(|r| r.start > 0))
}

/// Whether any of a field's text is bold.
pub fn has_bold(p: &Part, runs: &[StyleRun]) -> bool {
    has_style_in(p, runs, "bold")
}

/// Draw a field's laid-out text (`Pass::Shown`) at `pos`, thickening its
/// bold runs. Outline and shadow text is drawn as QuickDraw drew it: the
/// letters in black a pixel out each way (and, for shadow, a pixel further
/// down and right), then the letters themselves in white. On a black bar
/// that's white lettering.
pub fn paint_field_text(painter: &Painter, pv: &PartView, galley: std::sync::Arc<egui::Galley>, pos: Pos2, width: f32, hl: Option<(usize, usize)>) {
    let outline = has_style_in(&pv.part, &pv.styles, "outline");
    let shadow = has_style_in(&pv.part, &pv.styles, "shadow");
    if outline || shadow {
        let edge = painter.layout_job(field_job(painter.ctx(), &pv.part, &pv.text, &pv.styles, width, hl, Pass::Edge));
        let mut offsets: Vec<(f32, f32)> = vec![(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];
        if shadow {
            offsets.extend([(2.0, 1.0), (1.0, 2.0), (2.0, 2.0)]);
        }
        for (dx, dy) in offsets {
            painter.galley(pos + Vec2::new(dx, dy), edge.clone(), BLACK);
        }
    }
    painter.galley(pos, galley, BLACK);
    if has_bold(&pv.part, &pv.styles) {
        let bold = painter.layout_job(field_job(painter.ctx(), &pv.part, &pv.text, &pv.styles, width, hl, Pass::Bold));
        painter.galley(pos + Vec2::new(0.6, 0.0), bold, BLACK);
    }
}

/// List fields (autoSelect) hilite whole lines, across the field.
pub fn hilite_lines(painter: &Painter, p: &Part, galley: &egui::Galley, pos: Pos2, text_rect: Rect, hl: Option<(usize, usize)>) {
    let Some((start, end)) = hl.filter(|_| p.auto_select) else { return };
    let top = galley.pos_from_cursor(egui::text::CCursor::new(start)).top();
    let bottom = galley.pos_from_cursor(egui::text::CCursor::new(end)).bottom();
    let rows = Rect::from_x_y_ranges(text_rect.x_range(), pos.y + top..=pos.y + bottom);
    painter.rect_filled(rows, 0.0, text_color(p));
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
    let galley = painter.layout_job(field_job(painter.ctx(), p, &pv.text, &pv.styles, text_rect.width(), None, Pass::Plain));
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
    let galley = painter.layout_job(field_job(painter.ctx(), p, &pv.text, &pv.styles, text_rect.width(), None, Pass::Plain));
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
    draw_field_box(painter, p, rect, fill);
    // A combo field: its ▼, behind a line, inside the right edge.
    if p.choice_arrow().is_some() {
        let w = crate::model::CHOICE_ARROW as f32;
        let x = rect.right() - w;
        painter.line_segment([Pos2::new(x, rect.top() + 1.0), Pos2::new(x, rect.bottom() - 1.0)], Stroke::new(1.0, frame_color(p)));
        let c = Pos2::new(x + w / 2.0, rect.center().y);
        let tri = vec![c + Vec2::new(-4.5, -2.5), c + Vec2::new(4.5, -2.5), c + Vec2::new(0.0, 2.5)];
        painter.add(egui::Shape::convex_polygon(tri, text_color(p), Stroke::NONE));
    }
}

fn draw_field_box(painter: &Painter, p: &Part, rect: Rect, fill: Color32) {
    let stroke = Stroke::new(1.0, frame_color(p));
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
    let job = field_job(painter.ctx(), p, &pv.text, &pv.styles, text_rect.width(), highlight, Pass::Shown);
    let galley = painter.layout_job(job);
    // Clip to the whole field so large text isn't cut by the margins.
    let clip_rect = if p.style == PartStyle::Scrolling { text_rect } else { rect.shrink(1.0) };
    let clip = painter.with_clip_rect(clip_rect.intersect(painter.clip_rect()));
    let scroll = if p.style == PartStyle::Scrolling { p.scroll as f32 } else { 0.0 };
    if p.show_lines {
        draw_show_lines(&clip, p, &galley, text_rect, scroll);
    }
    let pos = field_text_origin(p, text_rect);
    hilite_lines(&clip, p, &galley, pos, text_rect, highlight);
    let content_h = galley.size().y;
    paint_field_text(&clip, pv, galley, pos, text_rect.width(), highlight);
    if p.style == PartStyle::Scrolling {
        draw_scroll_indicator(painter, scroll_view_rect(p, rect), scroll, content_h);
    }
}

/// showLines: a dotted line on each line of text's baseline, then every
/// textHeight to the bottom of the field. Drawn the same while typing.
pub fn draw_show_lines(painter: &Painter, p: &Part, galley: &egui::Galley, text_rect: Rect, scroll: f32) {
    let line_h = p.line_height() as f32;
    let baseline = |top: f32, h: f32| top + h - (h / 4.0).floor();
    let mut ys: Vec<f32> = galley.rows.iter().map(|r| baseline(r.pos.y, r.height())).collect();
    let mut next = galley.rows.last().map_or(0.0, |r| r.pos.y + r.height());
    while next - scroll < text_rect.height() {
        ys.push(baseline(next, line_h));
        next += line_h;
    }
    let dotted = Stroke::new(1.0, GRAY);
    for y in ys.into_iter().map(|y| text_rect.top() + y - scroll + 0.5) {
        if y > text_rect.top() && y < text_rect.bottom() {
            painter.line_segment([Pos2::new(text_rect.left(), y), Pos2::new(text_rect.right(), y)], dotted);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn word(ours: f32, theirs: f32, glyphs: usize) -> Token {
        Token { ours, theirs, word: true, glyphs, line_end: false }
    }
    fn space(ours: f32, theirs: f32) -> Token {
        Token { ours, theirs, word: false, glyphs: 1, line_end: false }
    }

    #[test]
    fn spacing_keeps_hypercards_widths() {
        // A word wider in egui is tightened to HyperCard's width.
        let s = spacing(&[word(110.0, 100.0, 11)]);
        assert_eq!(s[0], (-1.0, 0.0));
        // A space after a word that came out narrow takes up the difference.
        let s = spacing(&[word(95.0, 100.0, 10), space(3.0, 4.0)]);
        assert_eq!(s[1], (0.0, 4.0));
        // A long run with no spaces falls far behind: the next space pads by
        // no more than its own width, not by the whole difference.
        let s = spacing(&[word(300.0, 400.0, 60), space(3.0, 4.0), word(30.0, 30.0, 5)]);
        assert_eq!(s[1].1, 4.0);
    }
}
