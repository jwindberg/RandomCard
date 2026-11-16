//! Movie windows (see `runtime::movies`), drawn over the card where scripts
//! put them: a title bar in the window's style, the movie, and QuickTime's
//! controller under it, or its badge when the controller is hidden.
//! Clicking a playing movie pauses it and double-clicking plays it, as in
//! QuickTime.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::runtime::movies::{CONTROLLER, Style, Tick};
use crate::runtime::world::{Event, ObjRef, lock};
use crate::video::Player;

use super::App;

/// A movie window as the UI draws it this frame.
struct View {
    id: u32,
    name: String,
    style: Style,
    rect: crate::model::Rect,
    movie_rect: crate::model::Rect,
    floating: bool,
    controller: bool,
    badge: bool,
    pass_click: bool,
    playing: bool,
    muted: bool,
    time: f64,
    duration: f64,
    player: Arc<Player>,
}

/// A movie's picture on the GPU, and which frame it is.
pub struct MovieTexture {
    serial: u64,
    tex: egui::TextureHandle,
}

/// The controller's slider being dragged: the movie, and whether it was
/// playing, to play on when let go.
#[derive(Clone, Copy)]
pub struct Scrub {
    id: u32,
    was_playing: bool,
}

/// What the user did to a movie window this frame.
enum Action {
    Move(Vec2),
    Close,
    Toggle,
    Play,
    Pause,
    Mute,
    Step(f64),
    Scrub(f64),
    ShowController,
    Click(Pos2),
}

const GRAY: Color32 = Color32::from_gray(214);
const DARK: Color32 = Color32::from_gray(90);

impl App {
    /// Keep the movies going and draw their windows.
    pub fn movie_windows(&mut self, ctx: &egui::Context) {
        let (views, card, closing, stopped) = {
            let mut w = lock(&self.rt.world);
            let mut closing = Vec::new();
            let mut stopped = Vec::new();
            for m in &mut w.movies {
                match m.tick() {
                    Tick::Close => closing.push(m.id),
                    Tick::Stopped => stopped.push((m.name.clone(), m.id)),
                    Tick::Playing => {}
                }
            }
            let views: Vec<View> = w
                .movies
                .iter()
                .filter(|m| m.visible)
                .map(|m| View {
                    id: m.id,
                    name: m.name.clone(),
                    style: m.style,
                    rect: m.rect,
                    movie_rect: m.movie_rect,
                    floating: m.floating,
                    controller: m.controller,
                    badge: m.badge,
                    pass_click: m.pass_click,
                    playing: m.rate != 0.0,
                    muted: m.volume <= 0,
                    time: m.time(),
                    duration: m.duration(),
                    player: m.player.clone(),
                })
                .collect();
            let open: Vec<u32> = w.movies.iter().map(|m| m.id).collect();
            self.movie_tex.retain(|id, _| open.contains(id));
            if w.movies.iter().any(|m| m.rate != 0.0) {
                ctx.request_repaint_after(std::time::Duration::from_millis(10));
            }
            (views, ObjRef::Card(w.card_id), closing, stopped)
        };
        for id in closing {
            self.send(Event::CloseMovie(id));
        }
        // Stopping at its end changed its rate.
        for (name, id) in stopped {
            self.send(Event::Send { target: card, message: "movieRateChanged".into(), args: vec![name, id.to_string(), "0".into()] });
        }
        for v in views {
            let actions = self.movie_window(ctx, &v);
            self.movie_actions(&v, card, actions);
        }
    }

    /// The movie's newest frame as a texture.
    fn movie_texture(&mut self, ctx: &egui::Context, v: &View) -> Option<egui::TextureId> {
        let frame = v.player.frame()?;
        let image = || egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
        match self.movie_tex.get_mut(&v.id) {
            Some(t) if t.serial == frame.serial => {}
            Some(t) => {
                t.tex.set(image(), egui::TextureOptions::LINEAR);
                t.serial = frame.serial;
            }
            None => {
                let tex = ctx.load_texture(format!("movie-{}", v.id), image(), egui::TextureOptions::LINEAR);
                self.movie_tex.insert(v.id, MovieTexture { serial: frame.serial, tex });
            }
        }
        self.movie_tex.get(&v.id).map(|t| t.tex.id())
    }

    fn movie_window(&mut self, ctx: &egui::Context, v: &View) -> Vec<Action> {
        let tex = self.movie_texture(ctx, v);
        let bar = v.style.title_bar() as f32;
        let r = v.rect;
        let content = Rect::from_min_size(
            self.card_origin + vec2(r.left as f32, r.top as f32),
            vec2(r.width().max(1) as f32, r.height().max(1) as f32),
        );
        // The frame around it: title bar above, borders and shadow outside.
        let (border, shadow) = match v.style {
            Style::Borderless => (0.0, 0.0),
            Style::Dialog => (4.0, 0.0),
            Style::Shadow => (1.0, 2.0),
            _ => (1.0, 0.0),
        };
        let outer = Rect::from_min_max(content.min - vec2(border, border + bar), content.max + vec2(border + shadow, border + shadow));
        let id = egui::Id::unique(("movie-window", v.id));
        let order = if v.floating { egui::Order::Foreground } else { egui::Order::Middle };
        let mut actions = Vec::new();
        egui::Area::new(id).order(order).fixed_pos(outer.min).show(ctx, |ui| {
            ui.allocate_exact_size(outer.size(), Sense::hover());
            let painter = ui.painter().clone();
            let line = Stroke::new(1.0, Color32::BLACK);
            if shadow > 0.0 {
                painter.rect_filled(Rect::from_min_max(outer.min + vec2(shadow, shadow), outer.max), 0.0, Color32::BLACK);
            }
            let framed = Rect::from_min_max(outer.min, outer.max - vec2(shadow, shadow));
            if border > 0.0 {
                painter.rect_filled(framed, 0.0, Color32::WHITE);
                painter.rect_stroke(framed, 0.0, line, StrokeKind::Inside);
                if v.style == Style::Dialog {
                    painter.rect_stroke(framed.shrink(2.0), 0.0, Stroke::new(2.0, Color32::BLACK), StrokeKind::Inside);
                }
            }
            if bar > 0.0 {
                let title = Rect::from_min_size(framed.min + vec2(1.0, 1.0), vec2(framed.width() - 2.0, bar - 1.0));
                title_bar(&painter, title, v);
                let resp = ui.interact(title, id.with("bar"), Sense::click_and_drag());
                let close_box = close_box(title);
                if resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| close_box.expand(2.0).contains(p)) {
                    actions.push(Action::Close);
                } else if resp.dragged() {
                    actions.push(Action::Move(resp.drag_delta()));
                }
            }
            painter.rect_filled(content, 0.0, Color32::WHITE);
            let m = v.movie_rect;
            let picture = Rect::from_min_size(content.min + vec2(m.left as f32, m.top as f32), vec2(m.width() as f32, m.height() as f32));
            let movie_area = if v.controller { Rect::from_min_max(content.min, pos2(content.max.x, content.max.y - CONTROLLER as f32)) } else { content };
            if let Some(tex) = tex {
                let shown = ui.painter_at(movie_area);
                shown.image(tex, picture, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            let resp = ui.interact(movie_area, id.with("movie"), Sense::click());
            if v.controller {
                let strip = Rect::from_min_max(pos2(content.min.x, content.max.y - CONTROLLER as f32), content.max);
                self.controller(ui, id, strip, v, &mut actions);
            } else if v.badge && v.pass_click && picture.height() > 20.0 {
                let badge = Rect::from_min_size(pos2(picture.min.x + 4.0, picture.max.y - 16.0).max(movie_area.min), vec2(18.0, 12.0));
                draw_badge(&painter, badge);
                if resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| badge.contains(p)) {
                    actions.push(Action::ShowController);
                    return;
                }
            }
            if resp.clicked() || resp.double_clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    actions.push(Action::Click(pos2(p.x - content.min.x, p.y - content.min.y)));
                }
                if v.pass_click {
                    actions.push(if resp.double_clicked() { Action::Play } else if v.playing { Action::Pause } else { Action::Toggle });
                }
            }
        });
        actions
    }

    /// QuickTime's controller: the sound, play and pause, the slider, and
    /// a frame back and forward.
    fn controller(&mut self, ui: &mut egui::Ui, id: egui::Id, strip: Rect, v: &View, actions: &mut Vec<Action>) {
        let painter = ui.painter().clone();
        painter.rect_filled(strip, 0.0, GRAY);
        painter.hline(strip.x_range(), strip.top() + 0.5, Stroke::new(1.0, Color32::BLACK));
        let h = strip.height();
        let steps = strip.width() >= 120.0;
        let sound = Rect::from_min_size(strip.min, vec2(h, h));
        let play = Rect::from_min_size(strip.min + vec2(h, 0.0), vec2(h, h));
        let back = Rect::from_min_size(pos2(strip.max.x - 2.0 * h, strip.min.y), vec2(h, h));
        let forward = Rect::from_min_size(pos2(strip.max.x - h, strip.min.y), vec2(h, h));
        let track_end = if steps { back.min.x } else { strip.max.x };
        let track = Rect::from_min_max(pos2(play.max.x + 4.0, strip.center().y - 2.0), pos2(track_end - 6.0, strip.center().y + 3.0));
        for (r, what) in [(sound, 0), (play, 1), (back, 2), (forward, 3)] {
            if what >= 2 && !steps {
                continue;
            }
            painter.vline(r.max.x - 0.5, strip.y_range(), Stroke::new(1.0, DARK));
            if what == 2 {
                painter.vline(r.min.x + 0.5, strip.y_range(), Stroke::new(1.0, DARK));
            }
            let c = r.center();
            match what {
                0 => {
                    // A speaker, crossed out when muted.
                    painter.rect_filled(Rect::from_center_size(c - vec2(3.0, 0.0), vec2(3.0, 4.0)), 0.0, Color32::BLACK);
                    painter.add(egui::Shape::convex_polygon(vec![c + vec2(-2.0, -2.0), c + vec2(2.0, -5.0), c + vec2(2.0, 5.0), c + vec2(-2.0, 2.0)], Color32::BLACK, Stroke::NONE));
                    if v.muted {
                        painter.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], Stroke::new(1.5, Color32::from_rgb(200, 0, 0)));
                    } else {
                        painter.line_segment([c + vec2(4.0, -2.0), c + vec2(4.0, 2.0)], Stroke::new(1.0, Color32::BLACK));
                    }
                }
                1 if v.playing => {
                    painter.rect_filled(Rect::from_center_size(c - vec2(2.5, 0.0), vec2(2.0, 8.0)), 0.0, Color32::BLACK);
                    painter.rect_filled(Rect::from_center_size(c + vec2(2.5, 0.0), vec2(2.0, 8.0)), 0.0, Color32::BLACK);
                }
                1 => {
                    painter.add(egui::Shape::convex_polygon(vec![c + vec2(-3.0, -5.0), c + vec2(4.0, 0.0), c + vec2(-3.0, 5.0)], Color32::BLACK, Stroke::NONE));
                }
                2 => {
                    painter.add(egui::Shape::convex_polygon(vec![c + vec2(2.0, -4.0), c + vec2(2.0, 4.0), c + vec2(-3.0, 0.0)], Color32::BLACK, Stroke::NONE));
                    painter.vline(c.x - 4.0, (c.y - 4.0)..=(c.y + 4.0), Stroke::new(1.0, Color32::BLACK));
                }
                _ => {
                    painter.add(egui::Shape::convex_polygon(vec![c + vec2(-2.0, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.0, 4.0)], Color32::BLACK, Stroke::NONE));
                    painter.vline(c.x + 4.0, (c.y - 4.0)..=(c.y + 4.0), Stroke::new(1.0, Color32::BLACK));
                }
            }
            let resp = ui.interact(r, id.with(("button", what)), Sense::click());
            if resp.clicked() {
                actions.push(match what {
                    0 => Action::Mute,
                    1 => Action::Toggle,
                    2 => Action::Step(-1.0 / 30.0),
                    _ => Action::Step(1.0 / 30.0),
                });
            }
        }
        // The slider: the whole movie, the thumb where it is.
        painter.rect_filled(track, CornerRadius::same(2), Color32::WHITE);
        painter.rect_stroke(track, CornerRadius::same(2), Stroke::new(1.0, DARK), StrokeKind::Inside);
        let fraction = if v.duration > 0.0 { (v.time / v.duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let x = track.min.x + fraction * track.width();
        let thumb = Rect::from_center_size(pos2(x, strip.center().y), vec2(7.0, 11.0));
        painter.add(egui::Shape::convex_polygon(
            vec![thumb.left_top(), thumb.right_top(), pos2(thumb.right(), thumb.bottom() - 3.0), pos2(thumb.center().x, thumb.bottom()), pos2(thumb.left(), thumb.bottom() - 3.0)],
            Color32::from_gray(160),
            Stroke::new(1.0, Color32::BLACK),
        ));
        let zone = Rect::from_min_max(pos2(track.min.x - 4.0, strip.min.y), pos2(track.max.x + 4.0, strip.max.y));
        let resp = ui.interact(zone, id.with("slider"), Sense::click_and_drag());
        if (resp.is_pointer_button_down_on() || resp.dragged())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let f = ((p.x - track.min.x) / track.width().max(1.0)).clamp(0.0, 1.0) as f64;
            actions.push(Action::Scrub(f * v.duration));
        }
    }

    /// Do what the user did to a movie window.
    fn movie_actions(&mut self, v: &View, card: ObjRef, actions: Vec<Action>) {
        let mut messages = Vec::new();
        let scrubbing = self.movie_scrub.is_some_and(|s| s.id == v.id);
        let mut scrubbed = false;
        {
            let mut w = lock(&self.rt.world);
            let Some(m) = w.movies.iter_mut().find(|m| m.id == v.id) else { return };
            let before = m.rate;
            for a in actions {
                match a {
                    Action::Move(d) => m.rect = m.rect.translate(d.x.round() as i32, d.y.round() as i32),
                    Action::Close => {
                        drop(w);
                        self.send(Event::CloseMovie(v.id));
                        return;
                    }
                    Action::Toggle if m.rate != 0.0 => m.pause(),
                    Action::Toggle | Action::Play => m.play(),
                    Action::Pause => m.pause(),
                    Action::Mute => {
                        m.volume = -m.volume;
                        if m.volume == 0 {
                            m.volume = 256;
                        }
                        m.player.set_volume(m.volume.max(0) as f64 / 256.0);
                    }
                    Action::Step(d) => {
                        let t = (m.time() + d).clamp(m.start, m.end);
                        m.go_to(t);
                    }
                    Action::Scrub(t) => {
                        if !scrubbing {
                            self.movie_scrub = Some(Scrub { id: v.id, was_playing: m.rate != 0.0 });
                        }
                        m.go_to(t);
                        scrubbed = true;
                    }
                    Action::ShowController => m.show_controller(true),
                    Action::Click(p) => {
                        let time = m.time_text(m.time());
                        messages.push(("mouseDownInMovie", vec![m.name.clone(), m.id.to_string(), format!("{},{}", p.x as i32, p.y as i32), time, "movie".into(), m.id.to_string()]));
                    }
                }
            }
            // The slider let go: it says where the movie went, and plays on
            // if it was playing.
            if scrubbing
                && !scrubbed
                && let Some(s) = self.movie_scrub.take()
            {
                messages.push(("movieTimeChanged", vec![m.name.clone(), m.id.to_string(), m.time_text(m.time())]));
                if s.was_playing {
                    m.set_rate(1.0);
                }
            }
            if m.rate != before && !scrubbed && !scrubbing {
                messages.push(("movieRateChanged", vec![m.name.clone(), m.id.to_string(), crate::hypertalk::value::format_number(m.rate, "0.####")]));
            }
        }
        for (message, args) in messages {
            self.send(Event::Send { target: card, message: message.into(), args });
        }
    }
}

fn close_box(title: Rect) -> Rect {
    let s = (title.height() - 4.0).clamp(7.0, 11.0);
    Rect::from_min_size(pos2(title.min.x + 6.0, title.center().y - s / 2.0), vec2(s, s))
}

/// A title bar's stripes, close box, and name for the styles that show it.
fn title_bar(painter: &egui::Painter, title: Rect, v: &View) {
    let line = Stroke::new(1.0, Color32::BLACK);
    painter.rect_filled(title, 0.0, Color32::WHITE);
    for y in (2..title.height() as i32 - 1).step_by(2) {
        let y = title.top() + y as f32 + 0.5;
        painter.hline(title.left() + 1.0..=title.right() - 1.0, y, line);
    }
    painter.hline(title.x_range(), title.bottom() + 0.5, line);
    let close = close_box(title);
    painter.rect_filled(close.expand(1.0), 0.0, Color32::WHITE);
    painter.rect_stroke(close, 0.0, line, StrokeKind::Inside);
    if v.style.titled() {
        let font = FontId::proportional((title.height() - 6.0).clamp(9.0, 12.0));
        let galley = painter.layout_no_wrap(v.name.clone(), font, Color32::BLACK);
        let w = galley.size().x.min(title.width() - 50.0);
        let at = pos2(title.center().x - w / 2.0, title.center().y - galley.size().y / 2.0);
        painter.rect_filled(Rect::from_min_size(at - vec2(6.0, 0.0), vec2(w + 12.0, galley.size().y)), 0.0, Color32::WHITE);
        painter.with_clip_rect(Rect::from_min_size(at, vec2(w, galley.size().y))).galley(at, galley, Color32::BLACK);
    }
}

/// QuickTime's badge: a little strip of film, clicked to show the controller.
fn draw_badge(painter: &egui::Painter, r: Rect) {
    painter.rect_filled(r, CornerRadius::same(2), Color32::WHITE);
    painter.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
    let film = r.shrink2(vec2(4.0, 2.5));
    painter.rect_filled(film, 0.0, Color32::BLACK);
    for i in 0..3 {
        let x = film.min.x + 1.5 + i as f32 * 3.5;
        painter.rect_filled(Rect::from_min_size(pos2(x, film.min.y + 1.0), vec2(1.5, 1.0)), 0.0, Color32::WHITE);
        painter.rect_filled(Rect::from_min_size(pos2(x, film.max.y - 2.0), vec2(1.5, 1.0)), 0.0, Color32::WHITE);
    }
}

/// The movie textures, by movie window id.
pub type MovieTextures = HashMap<u32, MovieTexture>;
