//! Resolving object references (`card field "Name"`, `next card`, `me`) and
//! reading or writing their properties.

use crate::hypertalk::ast::*;
use crate::hypertalk::value::*;
use crate::model::{Id, Layer, Part, PartKind, PartStyle, Rect, TextAlign};

use super::interp::{Ctl, Interp, R, err, to_bool, to_int};
use super::world::{ObjRef, UiRequest};

/// Cards or backgrounds as (id, name) pairs, in order.
type Named = Vec<(Id, String)>;

impl Interp {
    pub fn resolve(&mut self, spec: &ObjSpec) -> R<ObjRef> {
        Ok(match spec {
            ObjSpec::Me => self.frame().me,
            ObjSpec::Target => self.frame().target,
            ObjSpec::MsgBox => ObjRef::MsgBox,
            ObjSpec::HyperCard => ObjRef::HyperCard,
            ObjSpec::Stack(None) => ObjRef::Stack,
            ObjSpec::Stack(Some(e)) => {
                let name = self.eval(e)?;
                match self.stack_ref(&name)? {
                    Some(o) => o,
                    None => return err(format!("No such stack \"{name}\"")),
                }
            }
            ObjSpec::Card(cs) => ObjRef::Card(self.resolve_card(cs)?),
            ObjSpec::Background(bs) => ObjRef::Background(self.resolve_bg(bs)?),
            ObjSpec::Part { kind, layer, which, card } => {
                let card = match card {
                    Some(cs) => self.resolve_card(cs)?,
                    None => self.w().card_id,
                };
                self.resolve_part(*kind, *layer, which, card)?
            }
        })
    }

    /// Resolve an expression that should name an object, either syntactically
    /// (`card 3`) or as text held in a variable ("card 3").
    pub fn resolve_dynamic(&mut self, e: &Expr) -> R<ObjRef> {
        if let Expr::Object(spec) = e {
            return self.resolve(spec);
        }
        let text = self.eval(e)?;
        self.parse_object_text(&text)
    }

    fn pick(&mut self, which: &Which, ids: &[Id], names: &[String], what: &str) -> R<Id> {
        let found = match which {
            Which::Id(e) => {
                let id = to_int(&self.eval(e)?)? as Id;
                ids.iter().copied().find(|i| *i == id)
            }
            Which::Ordinal(o) => {
                let idx = self.ordinal_pick(*o, ids.len());
                ids.get(idx.wrapping_sub(1)).copied()
            }
            Which::Expr(e) => {
                let v = self.eval(e)?;
                match parse_int(&v) {
                    Some(n) => ids.get((n - 1) as usize).copied().filter(|_| n >= 1),
                    None => names.iter().position(|n| n.eq_ignore_ascii_case(v.trim())).map(|i| ids[i]),
                }
            }
        };
        found.ok_or_else(|| {
            Ctl::Error(super::interp::ScriptError { msg: format!("No such {what}"), obj: None, line: None })
        })
    }

    fn ordinal_pick(&self, o: Ordinal, count: usize) -> usize {
        match o {
            Ordinal::Nth(n) => n.max(0) as usize,
            Ordinal::Last => count,
            Ordinal::Middle => count / 2 + 1,
            Ordinal::Any if count > 0 => rand::random_range(1..=count),
            Ordinal::Any => 0,
        }
    }

    pub fn resolve_card(&mut self, cs: &CardSpec) -> R<Id> {
        let (cards, current, marked): (Named, Id, Named) = {
            let w = self.w();
            let all = w.stack.cards.iter().map(|c| (c.id, c.name.clone())).collect();
            let marked = w.stack.cards.iter().filter(|c| c.marked).map(|c| (c.id, c.name.clone())).collect();
            (all, w.card_id, marked)
        };
        self.resolve_card_in(cs, &cards, current, &marked)
    }

    fn resolve_card_in(
        &mut self,
        cs: &CardSpec,
        cards: &[(Id, String)],
        current: Id,
        marked: &[(Id, String)],
    ) -> R<Id> {
        let pos = cards.iter().position(|c| c.0 == current).unwrap_or(0);
        let n = cards.len();
        if n == 0 {
            return err("No such card");
        }
        match cs {
            CardSpec::Position(Position::This) => Ok(current),
            CardSpec::Position(Position::Next) => Ok(cards[(pos + 1) % n].0),
            CardSpec::Position(Position::Prev) => Ok(cards[(pos + n - 1) % n].0),
            CardSpec::Which(which) => {
                let ids: Vec<Id> = cards.iter().map(|c| c.0).collect();
                let names: Vec<String> = cards.iter().map(|c| c.1.clone()).collect();
                if let Which::Id(e) = which {
                    // Card ids are stack-wide, not limited to a background.
                    let id = to_int(&self.eval(e)?)? as Id;
                    if self.w().stack.card(id).is_some() {
                        return Ok(id);
                    }
                    return err("No such card");
                }
                self.pick(which, &ids, &names, "card")
            }
            CardSpec::Marked(which) => {
                let ids: Vec<Id> = marked.iter().map(|c| c.0).collect();
                let names: Vec<String> = marked.iter().map(|c| c.1.clone()).collect();
                self.pick(which, &ids, &names, "marked card")
            }
            CardSpec::MarkedPosition(p) => {
                let order: Vec<usize> = match p {
                    Position::Next => (1..=n).map(|i| (pos + i) % n).collect(),
                    Position::Prev => (1..=n).map(|i| (pos + n - i) % n).collect(),
                    Position::This => vec![pos],
                };
                order
                    .into_iter()
                    .map(|i| cards[i].0)
                    .find(|id| marked.iter().any(|m| m.0 == *id))
                    .ok_or_else(|| Ctl::Error(super::interp::ScriptError {
                        msg: "No marked cards".into(),
                        obj: None,
                        line: None,
                    }))
            }
            CardSpec::Recent => {
                let w = self.w();
                Ok(if w.history_pos > 0 { w.history[w.history_pos - 1] } else { current })
            }
            CardSpec::Back => {
                let w = self.w();
                if w.history_pos > 0 { Ok(w.history[w.history_pos - 1]) } else { Ok(current) }
            }
            CardSpec::Forth => {
                let w = self.w();
                Ok(w.history.get(w.history_pos + 1).copied().unwrap_or(current))
            }
            CardSpec::OfStack(inner, name) => {
                // Only cards of the current stack can be referred to.
                let name = self.eval(name)?;
                match self.stack_ref(&name)? {
                    Some(ObjRef::Stack) => self.resolve_card_in(inner, cards, current, marked),
                    _ => err(format!("Can't refer to cards of stack \"{name}\" unless you go there")),
                }
            }
            CardSpec::OfBackground(inner, bs) => {
                let bg = self.resolve_bg(bs)?;
                let (subset, sub_marked): (Named, Named) = {
                    let w = self.w();
                    let s: Vec<(Id, String)> = w
                        .stack
                        .cards
                        .iter()
                        .filter(|c| c.background == bg)
                        .map(|c| (c.id, c.name.clone()))
                        .collect();
                    let m = w
                        .stack
                        .cards
                        .iter()
                        .filter(|c| c.background == bg && c.marked)
                        .map(|c| (c.id, c.name.clone()))
                        .collect();
                    (s, m)
                };
                // "This card" within another background means its first card.
                let cur = if subset.iter().any(|c| c.0 == current) {
                    current
                } else {
                    subset.first().map(|c| c.0).unwrap_or(current)
                };
                self.resolve_card_in(inner, &subset, cur, &sub_marked)
            }
        }
    }

    pub fn resolve_bg(&mut self, bs: &BgSpec) -> R<Id> {
        let (bgs, current): (Vec<(Id, String)>, Id) = {
            let w = self.w();
            (w.stack.backgrounds.iter().map(|b| (b.id, b.name.clone())).collect(), w.bg_id())
        };
        let pos = bgs.iter().position(|b| b.0 == current).unwrap_or(0);
        let n = bgs.len();
        match bs {
            BgSpec::Position(Position::This) => Ok(current),
            BgSpec::Position(Position::Next) => Ok(bgs[(pos + 1) % n].0),
            BgSpec::Position(Position::Prev) => Ok(bgs[(pos + n - 1) % n].0),
            BgSpec::Which(which) => {
                let ids: Vec<Id> = bgs.iter().map(|b| b.0).collect();
                let names: Vec<String> = bgs.iter().map(|b| b.1.clone()).collect();
                self.pick(which, &ids, &names, "background")
            }
        }
    }

    pub(super) fn resolve_part(&mut self, kind: Option<PartKind>, layer: Option<Layer>, which: &Which, card: Id) -> R<ObjRef> {
        let default_layer = match kind {
            Some(PartKind::Field) => Layer::Background,
            _ => Layer::Card,
        };
        let what = match kind {
            Some(k) => format!("{} {}", layer.unwrap_or(default_layer).name(), k.name()),
            None => "part".to_string(),
        };
        let try_layer = |me: &mut Self, l: Layer| -> R<Id> {
            let (ids, names): (Vec<Id>, Vec<String>) = {
                let w = me.w();
                w.stack
                    .parts(card, l)
                    .iter()
                    .filter(|p| kind.is_none_or(|k| p.kind == k))
                    .map(|p| (p.id, p.name.clone()))
                    .unzip()
            };
            me.pick(which, &ids, &names, &what)
        };
        let primary = layer.unwrap_or(default_layer);
        match try_layer(self, primary) {
            Ok(id) => Ok(ObjRef::Part { layer: primary, id, card }),
            Err(Ctl::Error(e)) => {
                // Without an explicit layer, fall back to the other layer for names and ids.
                if layer.is_none() && !matches!(which, Which::Ordinal(_)) {
                    let other = if primary == Layer::Card { Layer::Background } else { Layer::Card };
                    let numeric = matches!(which, Which::Expr(_));
                    if let Ok(id) = try_layer(self, other) {
                        let by_name = !numeric || {
                            // Only fall back for names, not numbers.
                            if let Which::Expr(e) = which {
                                let v = self.eval(e)?;
                                parse_int(&v).is_none()
                            } else {
                                true
                            }
                        };
                        if by_name {
                            return Ok(ObjRef::Part { layer: other, id, card });
                        }
                    }
                }
                Err(Ctl::Error(e))
            }
            Err(e) => Err(e),
        }
    }

    // ----- values -----

    pub fn object_value(&mut self, o: ObjRef) -> R<String> {
        match o {
            ObjRef::Part { layer, id, card } => Ok(self.w().stack.field_text(card, layer, id)),
            ObjRef::MsgBox => Ok(self.w().msg.clone()),
            _ => self.get_property(o, "name", NameLength::Default),
        }
    }

    pub fn set_object_value(&mut self, o: ObjRef, value: String) -> R<()> {
        let mut w = self.w();
        match o {
            ObjRef::Part { layer, id, card } => {
                w.stack.set_field_text(card, layer, id, value);
                w.dirty = true;
                Ok(())
            }
            ObjRef::MsgBox => {
                w.msg = value;
                w.msg_visible = true;
                Ok(())
            }
            _ => {
                drop(w);
                err(format!("Can't put text into {}", self.object_name(o)))
            }
        }
    }

    // ----- names -----

    pub fn object_name(&self, o: ObjRef) -> String {
        self.name_of(o, NameLength::Default)
    }

    pub fn name_of(&self, o: ObjRef, len: NameLength) -> String {
        let w = self.w();
        let s = &w.stack;
        // A stack converted from HyperCard is still its file on the disc until
        // it's saved, as HyperCard's long name gave it.
        let stack_long = || match s.path.as_ref().or(s.imported_from.as_ref()) {
            Some(p) => format!("stack \"{}\"", super::paths::shown(p)),
            None => format!("stack \"{}\"", s.name),
        };
        let card_desc = |id: Id| match s.card(id) {
            Some(c) if !c.name.is_empty() => format!("card \"{}\"", c.name),
            _ => format!("card id {id}"),
        };
        match o {
            ObjRef::Stack => match len {
                NameLength::Short => s.name.clone(),
                NameLength::Long => stack_long(),
                _ => format!("stack \"{}\"", s.name),
            },
            ObjRef::OtherStack(k) => match w.other(k) {
                Some(o) => match (len, o.path.as_ref().or(o.imported_from.as_ref())) {
                    (NameLength::Short, _) => o.name.clone(),
                    (NameLength::Long, Some(p)) => format!("stack \"{}\"", super::paths::shown(p)),
                    _ => format!("stack \"{}\"", o.name),
                },
                None => "unknown object".into(),
            },
            ObjRef::Card(id) => {
                let name = s.card(id).map(|c| c.name.clone()).unwrap_or_default();
                match len {
                    NameLength::Short if !name.is_empty() => name,
                    NameLength::Long => format!("{} of {}", card_desc(id), stack_long()),
                    _ => card_desc(id),
                }
            }
            ObjRef::Background(id) => {
                let name = s.background(id).map(|b| b.name.clone()).unwrap_or_default();
                let desc = if name.is_empty() { format!("bkgnd id {id}") } else { format!("bkgnd \"{name}\"") };
                match len {
                    NameLength::Short if !name.is_empty() => name,
                    NameLength::Long => format!("{desc} of {}", stack_long()),
                    _ => desc,
                }
            }
            ObjRef::Part { layer, id, card } => {
                let Some(p) = s.part(card, layer, id) else { return "unknown object".into() };
                let desc = if p.name.is_empty() {
                    format!("{} {} id {}", layer.name(), p.kind.name(), id)
                } else {
                    format!("{} {} \"{}\"", layer.name(), p.kind.name(), p.name)
                };
                match len {
                    NameLength::Short if !p.name.is_empty() => p.name.clone(),
                    NameLength::Long => format!("{desc} of {} of {}", card_desc(card), stack_long()),
                    _ => desc,
                }
            }
            ObjRef::MsgBox => "message box".into(),
            ObjRef::HyperCard => "RandomCard".into(),
        }
    }

    // ----- counting -----

    pub fn count(&mut self, c: &Countable) -> R<String> {
        let n = match c {
            Countable::Cards(None) => self.w().stack.cards.len(),
            Countable::Cards(Some(e)) => {
                let o = self.resolve_dynamic(e)?;
                match o {
                    ObjRef::Background(bg) => self.w().stack.cards_of_bg(bg).len(),
                    _ => self.w().stack.cards.len(),
                }
            }
            Countable::MarkedCards => self.w().stack.cards.iter().filter(|c| c.marked).count(),
            Countable::Menus => self.w().menus.len(),
            Countable::MenuItems(m) => self.count_menu_items(m)?,
            Countable::Backgrounds => self.w().stack.backgrounds.len(),
            Countable::Parts { kind, layer, of } => {
                let card = match of.as_deref() {
                    Some(spec) => match self.resolve(spec)? {
                        ObjRef::Card(id) => id,
                        ObjRef::Background(bg) => {
                            let w = self.w();
                            let Some(c) = w.stack.cards.iter().find(|c| c.background == bg) else {
                                return Ok("0".into());
                            };
                            c.id
                        }
                        _ => return err("Expected a card or background"),
                    },
                    None => self.w().card_id,
                };
                let w = self.w();
                w.stack.parts(card, *layer).iter().filter(|p| kind.is_none_or(|k| p.kind == k)).count()
            }
            Countable::Chunks(kind, e) => {
                let v = self.eval(e)?;
                count_chunks(&v, *kind, &self.item_delimiter)
            }
        };
        Ok(n.to_string())
    }

    // ----- properties -----

    pub fn get_global_property(&mut self, name: &str) -> R<String> {
        let w = self.w();
        Ok(match name {
            "itemdelimiter" | "itemdel" => self.item_delimiter.clone(),
            "numberformat" => self.number_format.clone(),
            "httpheaders" => self.http_headers.clone(),
            "httptimeout" => self.fmt(self.http_timeout),
            "responseheaders" => self.response_headers.clone(),
            "lockscreen" => bool_str(w.lock_screen),
            "lockmessages" | "lockmsgs" => bool_str(self.lock_messages),
            "userlevel" => w.level().to_string(),
            "editbkgnd" => bool_str(w.edit_bg),
            "filled" => bool_str(w.fill_shapes),
            "linesize" => w.line_size.to_string(),
            "pattern" => w.pattern.to_string(),
            "brush" => w.brush.to_string(),
            "polysides" => w.poly_sides.to_string(),
            "pencolor" => crate::model::color::color_text(rgb(w.line_color)),
            "brushcolor" => crate::model::color::color_text(rgb(w.fill_color)),
            "textfont" => w.paint_font.clone(),
            "textsize" => w.paint_text_size.to_string(),
            "textstyle" => w.paint_text_style.clone(),
            "cursor" => "1".into(),
            "dragspeed" => "0".into(),
            "textarrows" | "blindtyping" | "powerkeys" | "usermodify" => "true".into(),
            "lockrecent" | "grid" | "centered" | "multiple" | "messagewatcher" | "variablewatcher" => {
                "false".into()
            }
            "language" => "English".into(),
            "tracedelay" => w.trace_delay.to_string(),
            "version" => "2.41".into(),
            "name" => "RandomCard".into(),
            _ => {
                drop(w);
                return err(format!("No such property \"{name}\""));
            }
        })
    }

    pub fn set_global_property(&mut self, name: &str, v: &str) -> R<()> {
        match name {
            "itemdelimiter" | "itemdel" => {
                self.item_delimiter = if v.is_empty() { ",".into() } else { v.to_string() };
            }
            "numberformat" => self.number_format = v.to_string(),
            "httpheaders" => self.http_headers = v.to_string(),
            "httptimeout" => self.http_timeout = super::interp::to_number(v)?.clamp(0.1, 3600.0),
            "lockscreen" => self.w().lock_screen = to_bool(v)?,
            "lockmessages" | "lockmsgs" => self.lock_messages = to_bool(v)?,
            "editbkgnd" => self.w().edit_bg = to_bool(v)?,
            "filled" => self.w().fill_shapes = to_bool(v)?,
            "linesize" => self.w().line_size = to_int(v)?.clamp(1, 8) as u32,
            "pattern" => self.w().pattern = to_int(v)?.clamp(1, 40) as usize,
            "brush" => self.w().brush = to_int(v)?.clamp(1, 32) as u32,
            "polysides" => self.w().poly_sides = to_int(v)?.clamp(3, 50) as u32,
            // The paint colors: lines and text, and the pattern.
            "pencolor" | "brushcolor" => {
                let Some([r, g, b]) = crate::model::color::parse_color(v) else {
                    return err(format!("Expected a color but found \"{v}\""));
                };
                let c = crate::model::Pixel::rgba(r, g, b, 255);
                let mut w = self.w();
                if name == "pencolor" { w.line_color = c } else { w.fill_color = c }
            }
            // With no object, the text properties are those of painted text.
            "textfont" => self.w().paint_font = v.to_string(),
            "textsize" => self.w().paint_text_size = to_int(v)?.clamp(6, 128) as u32,
            "textstyle" => self.w().paint_text_style = v.to_string(),
            "userlevel" => {
                // A stack's protection caps how high the level can go.
                let mut w = self.w();
                let limit = w.level_limit();
                w.user_level = (to_int(v)?.clamp(1, 5) as u8).min(limit);
                w.show_level_at_home();
                // Set from Home, as by its User Preferences card, the level
                // is kept, as HyperCard kept it in Home.
                if w.is_home(&w.stack) {
                    let level = w.user_level;
                    w.requests.push(UiRequest::SaveUserLevel(level));
                }
            }
            "tracedelay" => self.w().trace_delay = to_int(v)?.clamp(0, 600) as u32,
            "cursor" | "dragspeed" | "textarrows" | "blindtyping" | "powerkeys" | "usermodify"
            | "lockrecent" | "grid" | "centered" | "multiple" | "language" | "messagewatcher"
            | "variablewatcher" | "textalign" | "textheight" | "scripttextfont" | "scripttextsize" | "debugger"
            | "scripteditor" => {}
            _ => return err(format!("No such property \"{name}\"")),
        }
        Ok(())
    }

    pub fn get_property(&mut self, o: ObjRef, name: &str, len: NameLength) -> R<String> {
        let name = name.to_ascii_lowercase();
        let name = name.as_str();
        if name == "name" {
            return Ok(self.name_of(o, len));
        }
        if name == "script" {
            return Ok(self.script_text(o).unwrap_or_default());
        }
        let w = self.w();
        let s = &w.stack;
        let full_rect = Rect::new(0, 0, s.width as i32, s.height as i32);
        let rect_props = |r: Rect| -> Option<String> {
            Some(match name {
                "rect" | "rectangle" => format!("{},{},{},{}", r.left, r.top, r.right, r.bottom),
                "loc" | "location" => {
                    let (x, y) = r.center();
                    format!("{x},{y}")
                }
                "left" => r.left.to_string(),
                "top" => r.top.to_string(),
                "right" => r.right.to_string(),
                "bottom" => r.bottom.to_string(),
                "width" => r.width().to_string(),
                "height" => r.height().to_string(),
                "topleft" => format!("{},{}", r.left, r.top),
                "botright" | "bottomright" => format!("{},{}", r.right, r.bottom),
                _ => return None,
            })
        };
        let v = match o {
            ObjRef::Part { layer, id, card } => {
                let Some(p) = s.part(card, layer, id) else {
                    drop(w);
                    return err("No such object");
                };
                if let Some(v) = rect_props(p.rect) {
                    return Ok(v);
                }
                match name {
                    "id" => p.id.to_string(),
                    "number" => {
                        let n = s.parts(card, layer).iter().filter(|q| q.kind == p.kind).position(|q| q.id == id);
                        (n.unwrap_or(0) + 1).to_string()
                    }
                    "partnumber" => {
                        (s.parts(card, layer).iter().position(|q| q.id == id).unwrap_or(0) + 1).to_string()
                    }
                    "visible" => bool_str(p.visible),
                    "style" => p.style.name().to_string(),
                    "textfont" => p.text_font.clone(),
                    // Empty when the part has none, for black and white.
                    "forecolor" | "backcolor" | "bordercolor" => {
                        let c = match name {
                            "forecolor" => p.fore_color,
                            "backcolor" => p.back_color,
                            _ => p.border_color,
                        };
                        c.map(crate::model::color::color_text).unwrap_or_default()
                    }
                    "textsize" => p.text_size.to_string(),
                    "textheight" => p.line_height().to_string(),
                    "fixedlineheight" => bool_str(p.fixed_line_height),
                    "textstyle" => p.text_style.clone(),
                    "textalign" => p.text_align.name().into(),
                    "hilite" | "highlight" | "hilight" | "highlite" => bool_str(s.hilite(card, layer, id)),
                    "autohilite" | "autohighlight" | "autohilight" => bool_str(p.auto_hilite),
                    "showname" => bool_str(p.show_name),
                    "enabled" => bool_str(p.enabled),
                    "family" => if p.family == 0 { String::new() } else { p.family.to_string() },
                    "sharedhilite" => bool_str(p.shared_hilite),
                    "icon" => p.icon.to_string(),
                    "titlewidth" => p.title_width.to_string(),
                    // A popup's chosen item, as HyperCard 2.2 reports it.
                    "selectedtext" | "selectedline" | "selectedchunk" if p.kind == PartKind::Button => {
                        let n = p.selected_line as usize;
                        let line = p.contents.split('\n').nth(n.wrapping_sub(1));
                        let Some(line) = line.filter(|_| n > 0) else { return Ok(String::new()) };
                        let number = s.parts(card, layer).iter().filter(|q| q.kind == p.kind).position(|q| q.id == id);
                        let button = format!("{} button {}", layer.name(), number.unwrap_or(0) + 1);
                        match name {
                            "selectedtext" => line.to_string(),
                            "selectedline" => format!("line {n} of {button}"),
                            _ => {
                                let start: usize = p.contents.split('\n').take(n - 1).map(|l| l.chars().count() + 1).sum();
                                format!("char {} to {} of {button}", start + 1, start + line.chars().count())
                            }
                        }
                    }
                    "locktext" => bool_str(p.lock_text),
                    "dontwrap" => bool_str(p.dont_wrap),
                    "autotab" => bool_str(p.auto_tab),
                    "widemargins" => bool_str(p.wide_margins),
                    "showlines" => bool_str(p.show_lines),
                    "sharedtext" => bool_str(p.shared_text),
                    "dontsearch" => bool_str(p.dont_search),
                    "dontsave" => bool_str(p.dont_save),
                    "autoselect" => bool_str(p.auto_select),
                    "multiplelines" => bool_str(p.multiple_lines),
                    "scroll" => p.scroll.to_string(),
                    "contents" | "text" => s.field_text(card, layer, id),
                    "owner" => {
                        drop(w);
                        let owner =
                            if layer == Layer::Card { ObjRef::Card(card) } else { ObjRef::Background(self.bg_of(card)) };
                        return Ok(self.name_of(owner, NameLength::Default));
                    }
                    _ => {
                        drop(w);
                        return err(format!("{} has no property \"{name}\"", self.object_name(o)));
                    }
                }
            }
            ObjRef::Card(id) => {
                let Some(c) = s.card(id) else {
                    drop(w);
                    return err("No such card");
                };
                if let Some(v) = rect_props(full_rect) {
                    return Ok(v);
                }
                match name {
                    "id" => c.id.to_string(),
                    "number" => (s.card_index(id).unwrap_or(0) + 1).to_string(),
                    "marked" => bool_str(c.marked),
                    "showpict" => bool_str(c.show_pict),
                    "cantdelete" => bool_str(c.cant_delete),
                    "dontsearch" => bool_str(c.dont_search),
                    "owner" => {
                        let bg = c.background;
                        drop(w);
                        return Ok(self.name_of(ObjRef::Background(bg), NameLength::Default));
                    }
                    _ => {
                        drop(w);
                        return err(format!("Cards have no property \"{name}\""));
                    }
                }
            }
            ObjRef::Background(id) => {
                let Some(b) = s.background(id) else {
                    drop(w);
                    return err("No such background");
                };
                if let Some(v) = rect_props(full_rect) {
                    return Ok(v);
                }
                match name {
                    "id" => b.id.to_string(),
                    "number" => (s.bg_index(id).unwrap_or(0) + 1).to_string(),
                    "showpict" => bool_str(b.show_pict),
                    "cantdelete" => bool_str(b.cant_delete),
                    "dontsearch" => bool_str(b.dont_search),
                    _ => {
                        drop(w);
                        return err(format!("Backgrounds have no property \"{name}\""));
                    }
                }
            }
            ObjRef::Stack => {
                // The stack's rect is its card window's, on screen. A
                // window's loc is its top left corner.
                let (x, y) = w.window_origin;
                let r = full_rect.translate(x, y);
                if matches!(name, "loc" | "location") {
                    return Ok(format!("{},{}", r.left, r.top));
                }
                if let Some(v) = rect_props(r) {
                    return Ok(v);
                }
                match name {
                    "cantmodify" => bool_str(s.cant_modify),
                    "cantdelete" => bool_str(s.protection.cant_delete),
                    "cantabort" => bool_str(s.protection.cant_abort),
                    "cantpeek" => bool_str(s.protection.cant_peek),
                    "privateaccess" => bool_str(s.protection.private_access),
                    "size" => s.size().to_string(),
                    // Space Compact Stack would free: text and styles left by
                    // deleted background fields, and unused backgrounds.
                    "freesize" => s.free_size().to_string(),
                    // How far the card is scrolled in its window (HyperCard
                    // 2): RandomCard always shows the whole card.
                    "scroll" => w.card_scroll.clone(),
                    _ => {
                        drop(w);
                        return err(format!("Stacks have no property \"{name}\""));
                    }
                }
            }
            ObjRef::OtherStack(k) => match (w.other(k), name) {
                (Some(o), "size") => serde_json::to_string(o).map(|j| j.len()).unwrap_or(0).to_string(),
                (Some(_), "freesize") => "0".into(),
                (Some(o), "cantmodify") => bool_str(o.cant_modify),
                (Some(o), "number") => o.cards.len().to_string(),
                _ => {
                    drop(w);
                    return err(format!("Can't get the {name} of that stack"));
                }
            },
            ObjRef::MsgBox => match name {
                "visible" => bool_str(w.msg_visible),
                "contents" | "text" => w.msg.clone(),
                "rect" | "rectangle" => {
                    let r = w.msg_rect;
                    format!("{},{},{},{}", r.left, r.top, r.right, r.bottom)
                }
                // A window's loc is its top left corner.
                "loc" | "location" => format!("{},{}", w.msg_rect.left, w.msg_rect.top),
                _ => {
                    drop(w);
                    return err(format!("The message box has no property \"{name}\""));
                }
            },
            ObjRef::HyperCard => {
                drop(w);
                return self.get_global_property(name);
            }
        };
        Ok(v)
    }

    pub fn bg_of(&self, card: Id) -> Id {
        self.w().stack.card(card).map(|c| c.background).unwrap_or(0)
    }

    pub fn set_property(&mut self, o: ObjRef, name: &str, v: &str) -> R<()> {
        let name = name.to_ascii_lowercase();
        let name = name.as_str();
        if name == "script" {
            self.scripts_changed();
        }
        let bad = |what: &str| -> R<()> { err(format!("Expected {what} here, not \"{v}\"")) };
        match o {
            ObjRef::Part { layer, id, card } => {
                if matches!(name, "hilite" | "highlight" | "hilight" | "highlite") {
                    let on = to_bool(v)?;
                    let mut w = self.w();
                    w.stack.set_hilite(card, layer, id, on);
                    w.dirty = true;
                    return Ok(());
                }
                if matches!(name, "contents" | "text") {
                    return self.set_object_value(o, v.to_string());
                }
                if matches!(name, "textfont" | "textsize" | "textstyle" | "forecolor") {
                    // The whole field takes the new setting, as in HyperCard.
                    self.w().stack.restyle_field(card, layer, id, |s| match name {
                        "textfont" => s.font = None,
                        "textsize" => s.size = None,
                        "forecolor" => s.color = None,
                        _ => s.style = None,
                    });
                }
                if name == "icon" {
                    // By id or by name; empty or 0 removes the icon.
                    let mut w = self.w();
                    let in_stack = |w: &super::world::World| match v.trim().parse::<i32>() {
                        Ok(id) => w.stack.icons.contains_key(&id).then_some(id),
                        Err(_) => w.stack.find_icon(v),
                    };
                    // Not in the stack: one of HyperCard's own icons, which
                    // then joins the stack's. Any number is accepted, as it
                    // always was, even with no icon to show.
                    let icon = if v.trim().is_empty() {
                        Some(0)
                    } else {
                        in_stack(&w)
                            .or_else(|| {
                                let (id, icon) = crate::import::hypercard_icon(v)?;
                                w.stack.icons.insert(id, icon);
                                Some(id)
                            })
                            .or_else(|| v.trim().parse::<i32>().ok())
                    };
                    let Some(icon) = icon else {
                        drop(w);
                        return err(format!("No such icon \"{v}\""));
                    };
                    if let Some(p) = w.stack.part_mut(card, layer, id) {
                        p.icon = icon;
                    }
                    w.dirty = true;
                    return Ok(());
                }
                let mut w = self.w();
                w.dirty = true;
                let Some(p) = w.stack.part_mut(card, layer, id) else { return err("No such object") };
                if set_rect_prop(&mut p.rect, name, v)? {
                    return Ok(());
                }
                match name {
                    "name" => p.name = v.to_string(),
                    "script" => p.script = v.to_string(),
                    "visible" => p.visible = to_bool(v)?,
                    "style" => match PartStyle::parse(v) {
                        Some(st) => p.style = st,
                        None => return bad("a style"),
                    },
                    "textfont" => p.text_font = v.to_string(),
                    "forecolor" | "backcolor" | "bordercolor" => {
                        // Empty (or "none") goes back to black and white.
                        let c = if v.trim().is_empty() || v.trim().eq_ignore_ascii_case("none") {
                            None
                        } else {
                            match crate::model::color::parse_color(v) {
                                Some(c) => Some(c),
                                None => return err(format!("Expected a color but found \"{v}\"")),
                            }
                        };
                        match name {
                            "forecolor" => p.fore_color = c,
                            "backcolor" => p.back_color = c,
                            _ => p.border_color = c,
                        }
                    }
                    "textsize" => {
                        p.text_size = to_int(v)?.clamp(6, 128) as u32;
                        // As in HyperCard, the line height follows the size.
                        p.text_height = 0;
                    }
                    "textheight" => p.text_height = to_int(v)?.clamp(1, 512) as u32,
                    "fixedlineheight" => p.fixed_line_height = to_bool(v)?,
                    "textstyle" => p.text_style = crate::model::styles::normalize_style(v),
                    "textalign" => match TextAlign::parse(v) {
                        Some(a) => p.text_align = a,
                        None => return bad("left, center or right"),
                    },
                    "autohilite" | "autohighlight" | "autohilight" => p.auto_hilite = to_bool(v)?,
                    "showname" => p.show_name = to_bool(v)?,
                    "enabled" => p.enabled = to_bool(v)?,
                    "family" => p.family = if v.trim().is_empty() { 0 } else { to_int(v)?.clamp(0, 15) as u32 },
                    "sharedhilite" => p.shared_hilite = to_bool(v)?,
                    "titlewidth" => p.title_width = to_int(v)?.max(0) as u32,
                    "locktext" => p.lock_text = to_bool(v)?,
                    "dontwrap" => p.dont_wrap = to_bool(v)?,
                    "autotab" => p.auto_tab = to_bool(v)?,
                    "widemargins" => p.wide_margins = to_bool(v)?,
                    "showlines" => p.show_lines = to_bool(v)?,
                    "sharedtext" => p.shared_text = to_bool(v)?,
                    "dontsearch" => p.dont_search = to_bool(v)?,
                    "dontsave" => p.dont_save = to_bool(v)?,
                    "autoselect" => p.auto_select = to_bool(v)?,
                    "multiplelines" => p.multiple_lines = to_bool(v)?,
                    "scroll" => p.scroll = to_int(v)?.max(0) as i32,
                    "id" | "number" | "owner" => return err(format!("Can't set the {name}")),
                    _ => return err(format!("No such property \"{name}\"")),
                }
                Ok(())
            }
            ObjRef::Card(id) => {
                let mut w = self.w();
                w.dirty = true;
                let Some(c) = w.stack.card_mut(id) else { return err("No such card") };
                match name {
                    "name" => c.name = v.to_string(),
                    "script" => c.script = v.to_string(),
                    "marked" => c.marked = to_bool(v)?,
                    "showpict" => c.show_pict = to_bool(v)?,
                    "cantdelete" => c.cant_delete = to_bool(v)?,
                    "dontsearch" => c.dont_search = to_bool(v)?,
                    _ => return err(format!("Can't set the {name} of a card")),
                }
                Ok(())
            }
            ObjRef::Background(id) => {
                let mut w = self.w();
                w.dirty = true;
                let Some(b) = w.stack.background_mut(id) else { return err("No such background") };
                match name {
                    "name" => b.name = v.to_string(),
                    "script" => b.script = v.to_string(),
                    "showpict" => b.show_pict = to_bool(v)?,
                    "cantdelete" => b.cant_delete = to_bool(v)?,
                    "dontsearch" => b.dont_search = to_bool(v)?,
                    _ => return err(format!("Can't set the {name} of a background")),
                }
                Ok(())
            }
            ObjRef::Stack => {
                let mut w = self.w();
                w.dirty = true;
                match name {
                    "name" => w.stack.name = v.to_string(),
                    "script" => w.stack.script = v.to_string(),
                    "cantmodify" => w.stack.cant_modify = to_bool(v)?,
                    "cantdelete" => w.stack.protection.cant_delete = to_bool(v)?,
                    "cantabort" => w.stack.protection.cant_abort = to_bool(v)?,
                    "cantpeek" => w.stack.protection.cant_peek = to_bool(v)?,
                    "privateaccess" => w.stack.protection.private_access = to_bool(v)?,
                    "userlevel" => {}
                    "scroll" => match crate::hypertalk::value::parse_point(v) {
                        Some((x, y)) => w.card_scroll = format!("{x},{y}"),
                        None => return err(format!("Expected a point, not \"{v}\"")),
                    },
                    "rect" | "rectangle" | "loc" | "location" | "topleft" | "left" | "top" | "right" | "bottom" | "botright"
                    | "bottomright" | "width" | "height" => {
                        drop(w);
                        return self.move_card_window(name, v);
                    }
                    _ => return err(format!("Can't set the {name} of a stack")),
                }
                Ok(())
            }
            ObjRef::OtherStack(k) => {
                // Changes to other stacks are saved right away.
                let mut w = self.w();
                let Some(s) = w.other_mut(k) else { return err("No such stack") };
                match name {
                    "name" => s.name = v.to_string(),
                    "script" => s.script = v.to_string(),
                    _ => return err(format!("Can't set the {name} of another stack")),
                }
                if let Some(path) = s.path.clone()
                    && !s.cant_modify
                {
                    s.save(&path).or_else(err)?;
                }
                Ok(())
            }
            ObjRef::MsgBox => {
                match name {
                    "visible" => self.w().msg_visible = to_bool(v)?,
                    "contents" | "text" => self.w().msg = v.to_string(),
                    "rect" | "rectangle" => match parse_rect(v) {
                        Some((l, t, r, b)) => self.w().msg_rect = Rect::new(l, t, r, b),
                        None => return err(format!("Expected a rectangle, not \"{v}\"")),
                    },
                    "loc" | "location" => self.move_msg(v)?,
                    "textfont" | "textsize" => {}
                    _ => return err(format!("Can't set the {name} of the message box")),
                }
                Ok(())
            }
            ObjRef::HyperCard => self.set_global_property(name, v),
        }
    }

    /// Move the message box so its top left corner is at a point.
    /// Set a geometric property of the card window. The card stays its own
    /// size, so this moves the window: to the new top left corner, or for
    /// right, bottom and botRight, to wherever puts that edge there. A rect
    /// that's smaller would show part of the card in HyperCard 2; RandomCard
    /// shows the whole card at the rect's top left.
    fn move_card_window(&mut self, name: &str, v: &str) -> R<()> {
        let (x, y) = {
            let w = self.w();
            let (x, y) = w.window_origin;
            let (cw, ch) = (w.stack.width as i32, w.stack.height as i32);
            let point = || parse_point(v).ok_or(());
            let int = || to_int(v).map(|n| n as i32).map_err(|_| ());
            let moved = match name {
                "rect" | "rectangle" => parse_rect(v).map(|(l, t, _, _)| (l, t)).ok_or(()),
                "loc" | "location" | "topleft" => point(),
                "botright" | "bottomright" => point().map(|(r, b)| (r - cw, b - ch)),
                "left" => int().map(|l| (l, y)),
                "top" => int().map(|t| (x, t)),
                "right" => int().map(|r| (r - cw, y)),
                "bottom" => int().map(|b| (x, b - ch)),
                // The card's size is the stack's, set with Stack Info.
                _ => Ok((x, y)),
            };
            match moved {
                Ok(p) => p,
                Err(()) => return err(format!("Can't set the {name} of a stack to \"{v}\"")),
            }
        };
        let mut w = self.w();
        w.window_origin = (x, y);
        w.requests.push(UiRequest::MoveWindow(x, y));
        Ok(())
    }

    pub(super) fn move_msg(&mut self, v: &str) -> R<()> {
        let Some((x, y)) = parse_point(v) else { return err(format!("Expected a point, not \"{v}\"")) };
        let mut w = self.w();
        let r = w.msg_rect;
        w.msg_rect = Rect::new(x, y, x + r.width(), y + r.bottom - r.top);
        Ok(())
    }

    pub fn part_at(&self, x: i32, y: i32) -> Option<ObjRef> {
        let w = self.w();
        let card = w.card_id;
        let hit = |parts: &[Part], layer: Layer| {
            parts.iter().rev().find(|p| p.visible && p.rect.contains(x, y)).map(|p| ObjRef::Part {
                layer,
                id: p.id,
                card,
            })
        };
        hit(w.stack.parts(card, Layer::Card), Layer::Card)
            .or_else(|| hit(w.stack.parts(card, Layer::Background), Layer::Background))
    }
}

/// Set a geometric property; returns false if `name` isn't one.
fn set_rect_prop(r: &mut Rect, name: &str, v: &str) -> R<bool> {
    let point = || {
        parse_point(v).ok_or_else(|| {
            Ctl::Error(super::interp::ScriptError { msg: format!("Expected a point, not \"{v}\""), obj: None, line: None })
        })
    };
    let int = || to_int(v).map(|n| n as i32);
    match name {
        "rect" | "rectangle" => {
            let Some((l, t, rt, b)) = parse_rect(v) else {
                return err(format!("Expected a rectangle, not \"{v}\""));
            };
            *r = Rect::new(l, t, rt, b);
        }
        "loc" | "location" => {
            let (x, y) = point()?;
            let (cx, cy) = r.center();
            *r = r.translate(x - cx, y - cy);
        }
        "left" => *r = r.translate(int()? - r.left, 0),
        "top" => *r = r.translate(0, int()? - r.top),
        "right" => *r = r.translate(int()? - r.right, 0),
        "bottom" => *r = r.translate(0, int()? - r.bottom),
        "width" => {
            let (cx, _) = r.center();
            let w = int()?.max(0);
            r.left = cx - w / 2;
            r.right = r.left + w;
        }
        "height" => {
            let (_, cy) = r.center();
            let h = int()?.max(0);
            r.top = cy - h / 2;
            r.bottom = r.top + h;
        }
        "topleft" => {
            let (x, y) = point()?;
            *r = r.translate(x - r.left, y - r.top);
        }
        "botright" | "bottomright" => {
            let (x, y) = point()?;
            *r = Rect::new(r.left, r.top, x, y);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// A paint color's red, green and blue.
fn rgb(p: crate::model::Pixel) -> crate::model::color::Rgb {
    [p.0[0], p.0[1], p.0[2]]
}

