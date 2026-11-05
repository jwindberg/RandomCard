//! AddColor, HyperCard 2.x's Color Tools XCMD, built in: `addColor
//! function, cd|bg, parameters…` as the HyperCard 2.4 "Color Tools" chapter
//! describes it. Each card and background keeps a list of color items
//! (buttons, fields, rectangles and pictures), back to front; scripts
//! address them by index, from 1 at the back. Temporary items
//! (`colorButton`…) show in front until the next `colorCard`.
//!
//! One difference: changes show straight away rather than at the next
//! `colorCard`, which every color handler calls anyway.

use crate::model::Id;
use crate::model::Layer;
use crate::model::Rect;
use crate::model::color::{ColorItem, ColorTarget, Rgb};

use super::interp::{Interp, R, to_int};
use super::world::Transition;

/// AddColor's switches and its temporary items.
#[derive(Clone, Debug)]
pub struct AddColorState {
    /// `addColor remove` turned color off in this stack (by stack serial),
    /// until `addColor install`.
    pub removed_in: Option<u64>,
    /// `addColor disable`: every other call is ignored until `enable`.
    pub disabled: bool,
    /// `colorBackground` turned this card's own colors off.
    pub card_off: Option<Id>,
    /// `colorCardLayered`: rectangles and pictures behind every part color.
    pub layered: bool,
    /// Temporary items in front, on this card, until the next `colorCard`.
    pub temp: Vec<(Id, Layer, ColorItem)>,
    /// `addColor install, depth`, for `addColor depth`.
    pub depth: u32,
}

impl Default for AddColorState {
    fn default() -> Self {
        AddColorState { removed_in: None, disabled: false, card_off: None, layered: false, temp: Vec::new(), depth: 8 }
    }
}

/// The type numbers getObjectClicked reports, and getObjectType's names.
fn type_number(t: &ColorTarget) -> u8 {
    match t {
        ColorTarget::Button { .. } => 1,
        ColorTarget::Field { .. } => 2,
        ColorTarget::Rect { .. } => 3,
        ColorTarget::Picture { file: false, .. } => 4,
        ColorTarget::Picture { file: true, .. } => 5,
    }
}

const TYPE_NAMES: [&str; 5] = ["buttonType", "fieldType", "rectType", "pictType", "pictFileType"];

/// A color as AddColor writes it, 0 to 65535 a channel.
fn parse_color16(s: &str) -> Option<Rgb> {
    let n: Vec<f64> = s.split(',').map(|v| v.trim().parse::<f64>()).collect::<Result<_, _>>().ok()?;
    let [r, g, b] = n.as_slice() else { return None };
    let c = |v: f64| (v.clamp(0.0, 65535.0) / 257.0).round() as u8;
    Some([c(*r), c(*g), c(*b)])
}

fn color16_text([r, g, b]: Rgb) -> String {
    format!("{},{},{}", r as u32 * 257, g as u32 * 257, b as u32 * 257)
}

fn parse_rect(s: &str) -> Option<Rect> {
    let n: Vec<i32> = s.split(',').map(|v| v.trim().parse::<f64>().map(|f| f.round() as i32)).collect::<Result<_, _>>().ok()?;
    match n.as_slice() {
        [l, t, r, b] => Some(Rect::new(*l, *t, *r, *b)),
        // A point: a picture at full size, its size not known here.
        [x, y] => Some(Rect::new(*x, *y, *x, *y)),
        _ => None,
    }
}

/// An AddColor transition: the nearest HyperCard visual effect.
fn effect_name(e: &str) -> &'static str {
    let e = e.trim().to_ascii_lowercase();
    match e.as_str() {
        "fromleft" => "wipe right",
        "fromright" => "wipe left",
        "fromtop" | "fromtopleft" | "fromtopright" => "wipe down",
        "frombottom" | "frombottomleft" | "frombottomright" => "wipe up",
        "irisopen" => "iris open",
        "irisclose" => "iris close",
        "barndooropen" => "barn door open",
        "barndoorclose" => "barn door close",
        "rectopen" => "zoom open",
        "rectclose" => "zoom close",
        _ if e.contains("checker") => "checkerboard",
        _ if e.contains("venetian") || e.contains("comb") || e.contains("rake") => "venetian blinds",
        _ => "dissolve",
    }
}

impl Interp {
    pub(super) fn addcolor(&mut self, args: &[String]) -> R<()> {
        let arg = |i: usize| args.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        let function = arg(0).to_ascii_lowercase();
        if self.w().addcolor.disabled && function != "enable" {
            return Ok(());
        }
        self.result = match self.addcolor_call(&function, args) {
            Ok(v) => v,
            Err(e) => e,
        };
        Ok(())
    }

    /// One AddColor function: Ok with what goes in the result, or Err with
    /// an error for it.
    fn addcolor_call(&mut self, function: &str, args: &[String]) -> Result<String, String> {
        let arg = |i: usize| args.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        // An unquoted cd or bg can arrive as the card's or background's
        // name (`card "One"`, `bkgnd id 2`), as RandomCard evaluates them.
        let layer = || {
            let a = arg(1).to_ascii_lowercase();
            let first = a.split_whitespace().next().unwrap_or("");
            match first {
                "cd" | "card" => Ok(Layer::Card),
                "bg" | "bkgnd" | "background" => Ok(Layer::Background),
                _ => Err(format!("Expected cd or bg, not \"{a}\"")),
            }
        };
        let number = |i: usize| to_int(&arg(i)).map_err(|_| format!("Expected a number, not \"{}\"", arg(i)));
        let color = |i: usize| parse_color16(&arg(i)).ok_or_else(|| format!("Expected a color like \"65535,0,0\", not \"{}\"", arg(i)));
        let rect = |i: usize| parse_rect(&arg(i)).ok_or_else(|| format!("Expected a point or rectangle, not \"{}\"", arg(i)));
        let bevel = |i: usize| -> Result<u16, String> { Ok(to_int(&arg(i)).unwrap_or(0).clamp(0, 32) as u16) };
        let transparent = |i: usize| arg(i).eq_ignore_ascii_case("t");
        let picture = |file: bool| -> Result<ColorTarget, String> {
            Ok(ColorTarget::Picture { rect: rect(3)?, name: arg(2), transparent: transparent(4), file })
        };
        match function {
            "install" => {
                let mut w = self.w();
                w.addcolor.removed_in = None;
                if let Ok(d) = to_int(&arg(1)) {
                    w.addcolor.depth = d.clamp(1, 32) as u32;
                }
                Ok(String::new())
            }
            "remove" => {
                let mut w = self.w();
                w.addcolor.removed_in = Some(w.stack_serial);
                Ok(String::new())
            }
            "enable" => {
                self.w().addcolor.disabled = false;
                Ok(String::new())
            }
            "disable" => {
                self.w().addcolor.disabled = true;
                Ok(String::new())
            }
            "depth" => Ok(self.w().addcolor.depth.to_string()),
            "getbitscall" => Ok("0".into()),
            "colorcard" | "colorcardlayered" | "colorbackground" => {
                {
                    let mut w = self.w();
                    let card = w.card_id;
                    w.addcolor.temp.clear();
                    w.addcolor.layered = function == "colorcardlayered";
                    w.addcolor.card_off = (function == "colorbackground").then_some(card);
                }
                let effect = arg(1);
                if !effect.is_empty() {
                    let ticks = to_int(&arg(2)).unwrap_or(60).clamp(0, 600);
                    self.color_effect(effect_name(&effect), ticks as f32 / 60.0)?;
                }
                Ok(String::new())
            }
            // Permanent items, placed at an index (0 the back, -1 the front).
            "addbutton" | "addfield" => {
                let (layer, id, c, b) = (layer()?, number(2)? as Id, color(3)?, bevel(4)?);
                let target = if function == "addbutton" {
                    ColorTarget::Button { id, color: c, bevel: b }
                } else {
                    ColorTarget::Field { id, color: c, bevel: b }
                };
                self.add_color_item(layer, target, to_int(&arg(5)).unwrap_or(-1))
            }
            "addrect" => {
                let target = ColorTarget::Rect { rect: rect(2)?, color: color(3)?, bevel: bevel(4)? };
                self.add_color_item(layer()?, target, to_int(&arg(5)).unwrap_or(-1))
            }
            "addpict" | "addpictfile" => {
                let target = picture(function == "addpictfile")?;
                self.add_color_item(layer()?, target, to_int(&arg(5)).unwrap_or(-1))
            }
            // Temporary items, in front until the next colorCard.
            "colorbutton" | "colorfield" | "colorrect" | "colorpict" | "colorpictfile" => {
                let layer = layer()?;
                let target = match function {
                    "colorbutton" => ColorTarget::Button { id: number(2)? as Id, color: color(3)?, bevel: bevel(4)? },
                    "colorfield" => ColorTarget::Field { id: number(2)? as Id, color: color(3)?, bevel: bevel(4)? },
                    "colorrect" => ColorTarget::Rect { rect: rect(2)?, color: color(3)?, bevel: bevel(4)? },
                    _ => picture(function == "colorpictfile")?,
                };
                {
                    let mut w = self.w();
                    let card = w.card_id;
                    w.addcolor.temp.push((card, layer, ColorItem { target, enabled: true }));
                }
                // Pictures take an effect, after the opacity.
                let effect = if function.starts_with("colorpict") { arg(5) } else { String::new() };
                if !effect.is_empty() {
                    let ticks = to_int(&arg(6)).unwrap_or(60).clamp(0, 600);
                    self.color_effect(effect_name(&effect), ticks as f32 / 60.0)?;
                }
                Ok(String::new())
            }
            "getbuttonindex" | "getfieldindex" => {
                let (layer, id) = (layer()?, number(2)? as Id);
                let button = function == "getbuttonindex";
                let items = self.color_items(layer);
                let at = items.iter().position(|i| match i.target {
                    ColorTarget::Button { id: b, .. } => button && b == id,
                    ColorTarget::Field { id: f, .. } => !button && f == id,
                    _ => false,
                });
                Ok(at.map_or(-1, |i| i as i64 + 1).to_string())
            }
            "removebutton" | "removefield" => {
                let (layer, id) = (layer()?, number(2)? as Id);
                let button = function == "removebutton";
                self.edit_color_items(layer, |items| {
                    items.retain(|i| match i.target {
                        ColorTarget::Button { id: b, .. } => !(button && b == id),
                        ColorTarget::Field { id: f, .. } => !(!button && f == id),
                        _ => true,
                    })
                });
                Ok(String::new())
            }
            "getobjectclicked" => {
                let layer = layer()?;
                let p = parse_rect(&arg(2)).ok_or_else(|| format!("Expected a point, not \"{}\"", arg(2)))?;
                let only = TYPE_NAMES.iter().position(|t| t.eq_ignore_ascii_case(&arg(3))).map(|i| i as u8 + 1);
                let parts = self.layer_parts(layer);
                let items = self.color_items(layer);
                let hit = items.iter().enumerate().rev().find(|(_, i)| {
                    let rect = item_rect(&i.target, &parts);
                    i.enabled
                        && only.is_none_or(|t| t == type_number(&i.target))
                        && rect.is_some_and(|r| r.contains(p.left, p.top))
                });
                Ok(match hit {
                    Some((at, i)) => format!("{},{}", at + 1, type_number(&i.target)),
                    None => "-1,-1".into(),
                })
            }
            "getobjecttype" => self.with_color_item(layer()?, number(2)?, |i, _| TYPE_NAMES[type_number(&i.target) as usize - 1].into()),
            "getobjectcolor" => self.with_color_item(layer()?, number(2)?, |i, _| match &i.target {
                ColorTarget::Button { color, .. } | ColorTarget::Field { color, .. } | ColorTarget::Rect { color, .. } => color16_text(*color),
                ColorTarget::Picture { transparent, .. } => format!("0,0,0,{}", if *transparent { "t" } else { "o" }),
            }),
            "getobjectbevel" => self.with_color_item(layer()?, number(2)?, |i, _| match &i.target {
                ColorTarget::Button { bevel, .. } | ColorTarget::Field { bevel, .. } | ColorTarget::Rect { bevel, .. } => bevel.to_string(),
                ColorTarget::Picture { .. } => "0".into(),
            }),
            "getobjectbounds" => self.with_color_item(layer()?, number(2)?, |i, parts| {
                item_rect(&i.target, parts).map(|r| format!("{},{},{},{}", r.left, r.top, r.right, r.bottom)).unwrap_or_default()
            }),
            "getpictname" => self.with_color_item(layer()?, number(2)?, |i, _| match &i.target {
                ColorTarget::Picture { name, .. } => name.clone(),
                _ => String::new(),
            }),
            "changeobjectcolor" => {
                let c = color(3)?;
                self.change_color_item(layer()?, number(2)?, |t| match t {
                    ColorTarget::Button { color, .. } | ColorTarget::Field { color, .. } | ColorTarget::Rect { color, .. } => *color = c,
                    ColorTarget::Picture { .. } => {}
                })
            }
            "changeobjectbevel" => {
                let b = bevel(3)?;
                self.change_color_item(layer()?, number(2)?, |t| match t {
                    ColorTarget::Button { bevel, .. } | ColorTarget::Field { bevel, .. } | ColorTarget::Rect { bevel, .. } => *bevel = b,
                    ColorTarget::Picture { .. } => {}
                })
            }
            "changeobjectbounds" => {
                let r = rect(3)?;
                self.change_color_item(layer()?, number(2)?, |t| match t {
                    ColorTarget::Rect { rect, .. } | ColorTarget::Picture { rect, .. } => *rect = r,
                    _ => {}
                })
            }
            "changeobjecttransparency" => {
                let on = transparent(3);
                self.change_color_item(layer()?, number(2)?, |t| {
                    if let ColorTarget::Picture { transparent, .. } = t {
                        *transparent = on;
                    }
                })
            }
            "enableobject" | "disableobject" => {
                let on = function == "enableobject";
                let (layer, index) = (layer()?, number(2)?);
                self.with_color_index(layer, index, |items, at| items[at].enabled = on)?;
                Ok(String::new())
            }
            "removeobject" => {
                let (layer, index) = (layer()?, number(2)?);
                self.with_color_index(layer, index, |items, at| {
                    items.remove(at);
                })?;
                Ok(String::new())
            }
            "moveforward" | "movebackward" | "movetofront" | "movetoback" => {
                let (layer, index) = (layer()?, number(2)?);
                let to = self.with_color_index(layer, index, |items, at| {
                    let to = match function {
                        "moveforward" => (at + 1).min(items.len() - 1),
                        "movebackward" => at.saturating_sub(1),
                        "movetofront" => items.len() - 1,
                        _ => 0,
                    };
                    let item = items.remove(at);
                    items.insert(to, item);
                    to
                })?;
                Ok((to + 1).to_string())
            }
            // Buttons and fields in front of rectangles and pictures, in
            // the order of their parts.
            "sort" => {
                let layer = layer()?;
                let order: Vec<(crate::model::PartKind, Id)> = self.layer_parts(layer).iter().map(|p| (p.kind, p.id)).collect();
                self.edit_color_items(layer, |items| {
                    items.sort_by_key(|i| match i.target {
                        ColorTarget::Button { id, .. } => {
                            1 + order.iter().position(|o| *o == (crate::model::PartKind::Button, id)).unwrap_or(0)
                        }
                        ColorTarget::Field { id, .. } => {
                            1 + order.iter().position(|o| *o == (crate::model::PartKind::Field, id)).unwrap_or(0)
                        }
                        _ => 0,
                    })
                });
                Ok(String::new())
            }
            // Forget the colors of deleted buttons and fields.
            "compact" => {
                let layer = layer()?;
                let parts: Vec<(crate::model::PartKind, Id)> = self.layer_parts(layer).iter().map(|p| (p.kind, p.id)).collect();
                self.edit_color_items(layer, |items| {
                    items.retain(|i| match i.target {
                        ColorTarget::Button { id, .. } => parts.contains(&(crate::model::PartKind::Button, id)),
                        ColorTarget::Field { id, .. } => parts.contains(&(crate::model::PartKind::Field, id)),
                        _ => true,
                    })
                });
                Ok(String::new())
            }
            other => Err(format!("AddColor has no function \"{other}\"")),
        }
    }

    /// Play a visual effect to the colors as they now are.
    fn color_effect(&mut self, effect: &str, seconds: f32) -> Result<(), String> {
        if self.w().lock_screen {
            return Ok(());
        }
        {
            let mut w = self.w();
            w.transition_seq += 1;
            let seq = w.transition_seq;
            w.transition = Some(Transition { effect: effect.into(), seconds, seq });
        }
        self.sleep(std::time::Duration::from_secs_f32(seconds)).map_err(|e| format!("{e:?}"))
    }

    fn layer_parts(&self, layer: Layer) -> Vec<crate::model::Part> {
        let w = self.w();
        w.stack.parts(w.card_id, layer).to_vec()
    }

    fn color_items(&self, layer: Layer) -> Vec<ColorItem> {
        let w = self.w();
        match layer {
            Layer::Card => w.stack.card(w.card_id).map(|c| c.colors.clone()),
            Layer::Background => w.stack.background(w.bg_id()).map(|b| b.colors.clone()),
        }
        .unwrap_or_default()
    }

    fn edit_color_items<T>(&mut self, layer: Layer, f: impl FnOnce(&mut Vec<ColorItem>) -> T) -> Option<T> {
        let mut w = self.w();
        let (card, bg) = (w.card_id, w.bg_id());
        let items = match layer {
            Layer::Card => &mut w.stack.card_mut(card)?.colors,
            Layer::Background => &mut w.stack.background_mut(bg)?.colors,
        };
        let out = f(items);
        w.dirty = true;
        Some(out)
    }

    fn add_color_item(&mut self, layer: Layer, target: ColorTarget, index: i64) -> Result<String, String> {
        self.edit_color_items(layer, |items| {
            let at = if index < 0 { items.len() } else { (index.max(1) as usize - 1).min(items.len()) };
            items.insert(at, ColorItem { target, enabled: true });
        })
        .ok_or("There's no card or background here")?;
        Ok(String::new())
    }

    /// Something about the item at an index (from 1), or empty for a bad
    /// index, as AddColor answers.
    fn with_color_item(&self, layer: Layer, index: i64, f: impl FnOnce(&ColorItem, &[crate::model::Part]) -> String) -> Result<String, String> {
        let items = self.color_items(layer);
        let parts = self.layer_parts(layer);
        Ok(usize::try_from(index - 1).ok().and_then(|i| items.get(i)).map(|i| f(i, &parts)).unwrap_or_default())
    }

    fn with_color_index<T>(&mut self, layer: Layer, index: i64, f: impl FnOnce(&mut Vec<ColorItem>, usize) -> T) -> Result<T, String> {
        let bad = || format!("There's no color item {index}");
        self.edit_color_items(layer, |items| {
            let at = usize::try_from(index - 1).ok().filter(|i| *i < items.len())?;
            Some(f(items, at))
        })
        .flatten()
        .ok_or_else(bad)
    }

    fn change_color_item(&mut self, layer: Layer, index: i64, f: impl FnOnce(&mut ColorTarget)) -> Result<String, String> {
        self.with_color_index(layer, index, |items, at| f(&mut items[at].target))?;
        Ok(String::new())
    }
}

/// Where an item is on the card: its own rectangle, or its part's.
fn item_rect(t: &ColorTarget, parts: &[crate::model::Part]) -> Option<Rect> {
    let part = |id: Id, kind: crate::model::PartKind| parts.iter().find(|p| p.id == id && p.kind == kind).map(|p| p.rect);
    match t {
        ColorTarget::Button { id, .. } => part(*id, crate::model::PartKind::Button),
        ColorTarget::Field { id, .. } => part(*id, crate::model::PartKind::Field),
        ColorTarget::Rect { rect, .. } | ColorTarget::Picture { rect, .. } => Some(*rect),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_rects() {
        assert_eq!(parse_color16("65535, 32767, 0"), Some([255, 127, 0]));
        assert_eq!(color16_text([255, 0, 128]), "65535,0,32896");
        assert_eq!(parse_color16("red"), None);
        assert_eq!(parse_rect("1,2,3,4"), Some(Rect::new(1, 2, 3, 4)));
        assert_eq!(parse_rect("5,6"), Some(Rect::new(5, 6, 5, 6)));
        assert_eq!(effect_name("irisOpen"), "iris open");
        assert_eq!(effect_name("circleCheckerClose"), "checkerboard");
    }
}
