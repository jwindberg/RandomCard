//! Built-in HyperTalk commands, navigation and menu actions.

use std::time::Duration;

use crate::hypertalk::ast::*;
use crate::hypertalk::parser::parse_statements;
use crate::hypertalk::value::*;
use crate::model::styles;
use crate::model::{Background, Card, Id, Layer, Part, PartKind, Picture, Rect};

use super::dates;
use super::interp::{Ctl, Interp, OpenFile, R, err, to_int, to_number};
use super::menus;
use super::undo::UndoEntry;
use super::world::{Dialog, DialogReply, EditOp, FieldRange, ObjRef, Tool, Transition, UiRequest};

impl Interp {
    pub fn command(&mut self, c: &Cmd) -> R<()> {
        match c {
            Cmd::Put { value, prep, dest, messages } => {
                let v = self.eval(value)?;
                match dest {
                    Some(Expr::Menu(r)) => {
                        let messages = match messages {
                            Some(m) => Some(self.eval(m)?),
                            None => None,
                        };
                        self.put_into_menu(r, *prep, &v, messages.as_deref())
                    }
                    Some(d) => self.put_into(d, *prep, &v),
                    None => {
                        let mut w = self.w();
                        match prep {
                            Prep::Into => w.msg = v,
                            Prep::Before => w.msg.insert_str(0, &v),
                            Prep::After => w.msg.push_str(&v),
                        }
                        w.msg_visible = true;
                        Ok(())
                    }
                }
            }
            Cmd::Get(e) => {
                let v = self.eval(e)?;
                self.set_var("it", v);
                Ok(())
            }
            Cmd::Set { prop, obj, value } => {
                let v = self.eval(value)?;
                let prop = prop.to_ascii_lowercase();
                match obj {
                    None => self.set_global_property(&prop, &v),
                    // Windows and palettes aren't supported.
                    Some(e) if is_menu(e) => Ok(()),
                    Some(Expr::Menu(r)) => self.set_menu_property(r, &prop, &v),
                    Some(e @ (Expr::Chunk(..) | Expr::Selection)) => self.set_chunk_property(e, &prop, &v),
                    Some(e) => {
                        let o = self.resolve_dynamic(e)?;
                        self.set_property(o, &prop, &v)
                    }
                }
            }
            Cmd::Add { value, dest } => self.arith(dest, value, |a, b| Ok(a + b)),
            Cmd::Subtract { value, dest } => self.arith(dest, value, |a, b| Ok(a - b)),
            Cmd::Multiply { dest, value } => self.arith(dest, value, |a, b| Ok(a * b)),
            Cmd::Divide { dest, value } => self.arith(dest, value, |a, b| {
                if b == 0.0 { err("Can't divide by zero") } else { Ok(a / b) }
            }),
            Cmd::Answer { prompt, buttons } => {
                let prompt = self.eval(prompt)?;
                let mut labels = Vec::new();
                for b in buttons {
                    labels.push(self.eval(b)?);
                }
                if labels.is_empty() {
                    labels.push("OK".into());
                }
                let reply = self.dialog(Dialog::Answer { prompt, buttons: labels.clone() })?;
                let it = match reply {
                    DialogReply::Button(b) => b,
                    DialogReply::Text(t) => t.unwrap_or_else(|| labels[0].clone()),
                };
                self.set_var("it", it);
                Ok(())
            }
            Cmd::Ask { prompt, default, password } => {
                let prompt = self.eval(prompt)?;
                let default = match default {
                    Some(d) => self.eval(d)?,
                    None => String::new(),
                };
                let reply = self.dialog(Dialog::Ask { prompt, default, password: *password })?;
                match reply {
                    DialogReply::Text(Some(t)) | DialogReply::Button(t) => {
                        self.set_var("it", t);
                        self.result.clear();
                    }
                    DialogReply::Text(None) => {
                        self.set_var("it", String::new());
                        self.result = "Cancel".into();
                    }
                }
                Ok(())
            }
            Cmd::AnswerFile(prompt) | Cmd::AskFile { prompt, .. } => {
                let title = self.eval(prompt)?;
                let mut dialog = rfd::FileDialog::new().set_title(&title);
                if let Cmd::AskFile { default: Some(d), .. } = c {
                    dialog = dialog.set_file_name(self.eval(d)?);
                }
                let picked = if matches!(c, Cmd::AnswerFile(_)) { dialog.pick_file() } else { dialog.save_file() };
                match picked {
                    Some(p) => {
                        self.set_var("it", p.display().to_string());
                        self.result.clear();
                    }
                    None => {
                        self.set_var("it", String::new());
                        self.result = "Cancel".into();
                    }
                }
                Ok(())
            }
            Cmd::Beep(n) => {
                let n = match n {
                    Some(e) => to_int(&self.eval(e)?)?.clamp(1, 20) as u32,
                    None => 1,
                };
                self.w().beeps += n;
                for _ in 0..n {
                    self.audio.play("beep", crate::sound::beep_tone(), 22050.0);
                }
                Ok(())
            }
            Cmd::Go(dest) => self.go(dest),
            Cmd::Wait(e, unit) => {
                let n = self.eval_number(e)?.max(0.0);
                let secs = match unit {
                    TimeUnit::Ticks => n / 60.0,
                    TimeUnit::Seconds => n,
                };
                self.sleep(Duration::from_secs_f64(secs))
            }
            Cmd::WaitUntil(cond) | Cmd::WaitWhile(cond) => {
                let until = matches!(c, Cmd::WaitUntil(_));
                loop {
                    if self.eval_bool(cond)? == until {
                        return Ok(());
                    }
                    self.sleep(Duration::from_millis(10))?;
                }
            }
            Cmd::Send { msg, target } => {
                let text = self.eval(msg)?;
                let target = match target {
                    Some(t) => self.resolve_dynamic(t)?,
                    None => self.frame().me,
                };
                self.send_text(&text, target)
            }
            Cmd::Do(e) => {
                let text = self.eval(e)?;
                let stmts = parse_statements(&text).or_else(err)?;
                self.exec_block(&stmts)
            }
            Cmd::Hide(what) => self.show_hide(what, false, None),
            Cmd::Show { what, at } => {
                let at = match at {
                    Some(e) => Some(self.eval(e)?),
                    None => None,
                };
                self.show_hide(what, true, at)
            }
            Cmd::ShowCards(n) => {
                let count = match n {
                    Some(e) => to_int(&self.eval(e)?)?.max(0) as usize,
                    None => self.w().stack.cards.len(),
                };
                for _ in 0..count {
                    self.go_card_id(self.next_card_id(), false)?;
                    self.sleep(Duration::from_millis(60))?;
                }
                Ok(())
            }
            Cmd::LockScreen => {
                self.w().lock_screen = true;
                Ok(())
            }
            Cmd::UnlockScreen(effect) => {
                if let Some(e) = effect {
                    let name = self.eval(&e.name)?;
                    self.pending_effect = Some((name, e.speed.clone()));
                }
                self.w().lock_screen = false;
                self.play_pending_effect()
            }
            Cmd::LockMessages => {
                self.lock_messages = true;
                Ok(())
            }
            Cmd::UnlockMessages => {
                self.lock_messages = false;
                Ok(())
            }
            Cmd::VisualEffect(e) => {
                let name = self.eval(&e.name)?;
                self.pending_effect = Some((name, e.speed.clone()));
                Ok(())
            }
            Cmd::PushCard(e) => {
                let id = match e {
                    None => self.w().card_id,
                    Some(e) => match self.resolve_dynamic(e)? {
                        ObjRef::Card(id) => id,
                        _ => return err("Can only push cards"),
                    },
                };
                self.pushed_cards.push(id);
                Ok(())
            }
            Cmd::PopCard(into) => {
                let Some(id) = self.pushed_cards.pop() else { return Ok(()) };
                match into {
                    Some((prep, dest)) => {
                        let name = self.name_of(ObjRef::Card(id), NameLength::Long);
                        self.put_into(dest, *prep, &name)
                    }
                    None => self.go_card_id(id, true),
                }
            }
            Cmd::Find { mode, what, field } => {
                let text = self.eval(what)?;
                let field = match field {
                    Some(f) => Some(self.resolve_dynamic(f)?),
                    None => None,
                };
                self.find(&text, *mode, field)
            }
            Cmd::SortChunks { kind, container, ascending, style, by } => {
                self.sort_chunks(*kind, container, *ascending, *style, by.as_ref())
            }
            Cmd::SortCards { ascending, style, by, marked, of } => {
                let bg = match of {
                    Some(e) => match self.resolve_dynamic(e)? {
                        ObjRef::Background(bg) => Some(bg),
                        ObjRef::Stack => None,
                        _ => return err("Can only sort the cards of a background or stack"),
                    },
                    None => None,
                };
                self.sort_cards(*ascending, *style, by, *marked, bg)
            }
            Cmd::Delete(e) => self.delete(e),
            Cmd::ChooseTool(e) => {
                let v = self.eval(e)?;
                let Some(t) = Tool::parse(&v) else { return err(format!("No such tool \"{v}\"")) };
                let mut w = self.w();
                w.tool = t;
                w.focus = None;
                Ok(())
            }
            Cmd::DoMenu(e) => {
                let item = self.eval(e)?;
                let card = self.current_card_ref();
                if !self.send_message(card, "doMenu", std::slice::from_ref(&item))? {
                    self.perform_menu(&item)?;
                }
                Ok(())
            }
            Cmd::Mark(what) => self.mark(what, true),
            Cmd::Unmark(what) => self.mark(what, false),
            Cmd::OpenFile(e) => {
                let name = self.eval(e)?;
                let content = std::fs::read_to_string(&name).unwrap_or_default();
                self.files.insert(name, OpenFile { content, pos: 0, dirty: false });
                Ok(())
            }
            Cmd::CloseFile(e) => {
                let name = self.eval(e)?;
                self.close_file(&name)
            }
            Cmd::ReadFile { file, until, count, at } => {
                let name = self.eval(file)?;
                let until = match until {
                    Some(u) => Some(self.eval(u)?),
                    None => None,
                };
                let count = match count {
                    Some(n) => Some(to_int(&self.eval(n)?)?.max(0) as usize),
                    None => None,
                };
                let at = match at {
                    Some(a) => Some(to_int(&self.eval(a)?)?.max(1) as usize - 1),
                    None => None,
                };
                let Some(f) = self.files.get_mut(&name) else {
                    return err(format!("File \"{name}\" is not open"));
                };
                if let Some(a) = at {
                    f.pos = a;
                }
                let rest: String = f.content.chars().skip(f.pos).collect();
                let chunk: String = match (&until, count) {
                    (_, Some(n)) => rest.chars().take(n).collect(),
                    (Some(u), _) if !u.is_empty() && u != "eof" => match rest.find(u.as_str()) {
                        Some(i) => rest[..i + u.len()].to_string(),
                        None => rest,
                    },
                    _ => rest,
                };
                f.pos += chunk.chars().count();
                self.result = if f.pos >= f.content.chars().count() { "eof".into() } else { String::new() };
                self.set_var("it", chunk);
                Ok(())
            }
            Cmd::WriteFile { data, file, at, at_end } => {
                let data = self.eval(data)?;
                let name = self.eval(file)?;
                let at = match at {
                    Some(a) => Some(to_int(&self.eval(a)?)?.max(1) as usize - 1),
                    None => None,
                };
                let Some(f) = self.files.get_mut(&name) else {
                    return err(format!("File \"{name}\" is not open"));
                };
                let pos = if *at_end { f.content.chars().count() } else { at.unwrap_or(f.pos) };
                let mut chars: Vec<char> = f.content.chars().collect();
                let pos = pos.min(chars.len());
                // Writing overwrites from the position onward, like HyperCard.
                chars.truncate(pos);
                f.content = chars.into_iter().collect::<String>() + &data;
                f.pos = f.content.chars().count();
                f.dirty = true;
                Ok(())
            }
            Cmd::Enable(e) | Cmd::Disable(e) if is_menu(e) => Ok(()),
            Cmd::Enable(Expr::Menu(r)) => self.enable_menu(r, true),
            Cmd::Disable(Expr::Menu(r)) => self.enable_menu(r, false),
            Cmd::CreateMenu(e) => {
                let name = self.eval(e)?;
                self.create_menu(&name)
            }
            Cmd::StartUsing(e) => {
                let name = self.eval(e)?;
                self.start_using(&name)
            }
            Cmd::StopUsing(e) => {
                let name = self.eval(e)?;
                self.stop_using(&name)
            }
            Cmd::ResetMenuBar => {
                self.reset_menu_bar();
                Ok(())
            }
            Cmd::Enable(e) | Cmd::Disable(e) => {
                let o = self.resolve_dynamic(e)?;
                self.set_property(o, "enabled", if matches!(c, Cmd::Enable(_)) { "true" } else { "false" })
            }
            Cmd::EditScript(e) => {
                let o = self.resolve_dynamic(e)?;
                self.w().requests.push(UiRequest::EditScript(o));
                Ok(())
            }
            Cmd::Select(Select::Empty) => {
                let mut w = self.w();
                w.focus = None;
                w.set_selection(None);
                Ok(())
            }
            Cmd::Select(Select::Part(e)) => {
                let obj = self.resolve_dynamic(e)?;
                let ObjRef::Part { layer, id, card } = obj else { return err("Can only select a button or field") };
                let mut w = self.w();
                let Some(kind) = w.stack.part(card, layer, id).map(|p| p.kind) else { return err("No such part") };
                w.tool = if kind == PartKind::Button { Tool::Button } else { Tool::Field };
                w.focus = None;
                w.requests.push(UiRequest::SelectPart(obj));
                Ok(())
            }
            Cmd::Select(Select::Text(prep, e)) => self.select_text(*prep, e),
            Cmd::ClickAt(e) => {
                let v = self.eval(e)?;
                let Some((x, y)) = parse_point(&v) else { return err(format!("Expected a point, not \"{v}\"")) };
                if self.w().tool.is_paint() {
                    self.paint_click(x, y);
                    return Ok(());
                }
                self.click_at(x, y)
            }
            Cmd::Drag { from, to } => {
                let (a, b) = (self.eval(from)?, self.eval(to)?);
                let (Some(a), Some(b)) = (parse_point(&a), parse_point(&b)) else {
                    return err("Expected points after drag from … to …");
                };
                let mut w = self.w();
                if let Some(tool) = w.tool.drag_tool() {
                    let settings = w.paint_settings();
                    if let Some(pic) = w.layer_picture_mut() {
                        crate::paint::apply_drag(pic, tool, a, b, &settings);
                    }
                    w.picture_rev += 1;
                    w.dirty = true;
                }
                Ok(())
            }
            Cmd::Type(e) => {
                let text = self.eval(e)?;
                let mut w = self.w();
                // With the text tool, type paints text where `click at` put the cursor.
                if w.tool == Tool::Text
                    && let Some((x, y)) = w.text_point
                {
                    let (font, size, style) = (w.paint_font.clone(), w.paint_text_size, w.paint_text_style.clone());
                    if let Some(pic) = w.layer_picture_mut() {
                        let next = crate::paint::draw_text(pic, x, y, &text, size, &font, &style);
                        w.text_point = Some(next);
                    }
                    w.picture_rev += 1;
                    w.dirty = true;
                    return Ok(());
                }
                if let Some((layer, id)) = w.focus {
                    let card = w.card_id;
                    let cur = w.stack.field_text(card, layer, id);
                    w.stack.set_field_text(card, layer, id, cur + &text);
                }
                Ok(())
            }
            Cmd::Convert { value, to } => {
                let v = self.eval(value)?;
                let Some(dt) = dates::parse_date(&v) else {
                    return err(format!("Invalid date: \"{v}\""));
                };
                let Some(out) = dates::convert(dt, to) else {
                    return err(format!("Can't convert to \"{to}\""));
                };
                let is_container = matches!(value, Expr::Var(_) | Expr::Chunk(..) | Expr::Object(_));
                if is_container {
                    self.put_into(value, Prep::Into, &out)
                } else {
                    self.set_var("it", out);
                    Ok(())
                }
            }
            Cmd::Play { sound, tempo, notes } => {
                let name = self.eval(sound)?;
                let tempo = match tempo {
                    Some(t) => self.eval_number(t)? as f32,
                    None => crate::sound::DEFAULT_TEMPO,
                };
                let mut text = Vec::new();
                for n in notes {
                    text.push(self.eval(n)?);
                }
                // The stack's own sound, else RandomCard's stand-in for a HyperCard voice.
                let found = self.w().stack.find_sound(&name).cloned();
                let Some(snd) = found.or_else(|| crate::sound::builtin(&name)) else {
                    self.result = format!("Can't find sound \"{name}\"");
                    self.w().beeps += 1;
                    return Ok(());
                };
                let samples = crate::sound::render(&snd, &crate::sound::parse_notes(&text.join(" ")), tempo);
                self.audio.play(&snd.name, samples, snd.rate);
                Ok(())
            }
            Cmd::PlayStop => {
                self.audio.stop();
                Ok(())
            }
            Cmd::Dial(number) => {
                let number = self.eval(number)?;
                self.audio.play("dial", crate::sound::dial_tones(&number), 22050.0);
                Ok(())
            }
            Cmd::Speak(e) => {
                let text = self.eval(e)?;
                // Use a system speech synthesizer when one is installed.
                for prog in ["spd-say", "espeak", "say"] {
                    if std::process::Command::new(prog).arg(&text).spawn().is_ok() {
                        break;
                    }
                }
                Ok(())
            }
            Cmd::SaveStack(name) => {
                let name = match name {
                    Some(n) => Some(self.eval(n)?),
                    None => None,
                };
                self.save_stack(name)
            }
            Cmd::ArrowKey(e) => {
                let dir = self.eval(e)?;
                let card = self.current_card_ref();
                if !self.send_message(card, "arrowKey", std::slice::from_ref(&dir))? {
                    self.arrow_key(&dir)?;
                }
                Ok(())
            }
            Cmd::NoOp => Ok(()),
            Cmd::DebugCheckpoint => {
                self.pause_next = true;
                Ok(())
            }
        }
    }

    fn arith(&mut self, dest: &Expr, value: &Expr, f: impl Fn(f64, f64) -> R<f64>) -> R<()> {
        let v = self.eval_number(value)?;
        let cur = self.eval(dest)?;
        let cur = to_number(&cur)?;
        let r = f(cur, v)?;
        let s = self.fmt(r);
        self.put_into(dest, Prep::Into, &s)
    }

    /// `send "foo 1,2" to btn 1`: parse the text as a message and arguments.
    pub fn send_text(&mut self, text: &str, target: ObjRef) -> R<()> {
        let stmts = parse_statements(text).or_else(err)?;
        for s in &stmts {
            match &s.kind {
                StmtKind::Message { name, args } => {
                    let mut vals = Vec::new();
                    for a in args {
                        vals.push(self.eval(a)?);
                    }
                    if !self.send_message(target, name, &vals)? {
                        let lname = name.to_ascii_lowercase();
                        if lname == "domenu" || lname == "arrowkey" {
                            let card = self.current_card_ref();
                            let saved = std::mem::replace(&mut self.frame_mut().target, card);
                            let r = match lname.as_str() {
                                "domenu" => self.perform_menu(vals.first().map(String::as_str).unwrap_or("")),
                                _ => self.arrow_key(vals.first().map(String::as_str).unwrap_or("")),
                            };
                            self.frame_mut().target = saved;
                            r?;
                        }
                    }
                }
                // Built-in commands sent to an object simply run.
                _ => self.exec(s)?,
            }
        }
        Ok(())
    }

    fn show_hide(&mut self, what: &ShowHide, show: bool, at: Option<String>) -> R<()> {
        match what {
            ShowHide::MsgBox => self.w().msg_visible = show,
            ShowHide::MessageWatcher => self.w().requests.push(UiRequest::MessageWatcher(show)),
            ShowHide::VariableWatcher => self.w().requests.push(UiRequest::VariableWatcher(show)),
            ShowHide::MenuBar => self.w().menubar_visible = show,
            ShowHide::TitleBar => {}
            ShowHide::CardPicture => {
                let mut w = self.w();
                let id = w.card_id;
                if let Some(c) = w.stack.card_mut(id) {
                    c.show_pict = show;
                }
            }
            ShowHide::BgPicture => {
                let mut w = self.w();
                let id = w.bg_id();
                if let Some(b) = w.stack.background_mut(id) {
                    b.show_pict = show;
                }
            }
            ShowHide::PictureOf(e) => {
                let o = self.resolve_dynamic(e)?;
                self.set_property(o, "showpict", if show { "true" } else { "false" })?;
            }
            ShowHide::Object(e) if is_menu(e) => {}
            ShowHide::Object(e) => {
                let o = self.resolve_dynamic(e)?;
                if o == ObjRef::MsgBox {
                    self.w().msg_visible = show;
                    return Ok(());
                }
                self.set_property(o, "visible", if show { "true" } else { "false" })?;
                if let Some(at) = at {
                    self.set_property(o, "loc", &at)?;
                }
            }
        }
        Ok(())
    }

    // ----- navigation -----

    fn next_card_id(&self) -> Id {
        let w = self.w();
        let i = w.card_index();
        w.stack.cards[(i + 1) % w.stack.cards.len()].id
    }

    fn go(&mut self, dest: &GoDest) -> R<()> {
        self.result.clear();
        let target = match dest {
            GoDest::Card(CardSpec::Back) => return self.go_history(-1),
            GoDest::Card(CardSpec::Forth) => return self.go_history(1),
            GoDest::Home => return self.go_home(),
            GoDest::Stack(name) => {
                let name = self.eval(name)?;
                return self.go_stack(&name, None);
            }
            GoDest::Card(CardSpec::OfStack(inner, name)) => {
                let name = self.eval(name)?;
                return self.go_stack(&name, Some(inner));
            }
            GoDest::Card(cs) => match self.resolve_card(cs) {
                Ok(id) => Some(id),
                Err(Ctl::Error(_)) => None,
                Err(e) => return Err(e),
            },
            GoDest::Expr(e) => {
                let v = self.eval(e)?;
                match self.go_target_from_text(&v)? {
                    Some(id) => Some(id),
                    None => {
                        // `go "stack Other"` or `go "path/to/stack"`
                        let t = v.trim();
                        let name = match t.get(..6) {
                            Some(p) if p.eq_ignore_ascii_case("stack ") => &t[6..],
                            _ => t,
                        };
                        if self.stack_ref(name)?.is_some() {
                            return self.go_stack(name, None);
                        }
                        None
                    }
                }
            }
        };
        match target {
            Some(id) => self.go_card_id(id, true),
            None => {
                self.result = "No such card.".into();
                Ok(())
            }
        }
    }

    fn go_target_from_text(&mut self, v: &str) -> R<Option<Id>> {
        let t = v.trim();
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("stack ") || lower == "stack" {
            let name = t[5..].trim().trim_matches('"').to_string();
            let w = self.w();
            if name.is_empty() || name.eq_ignore_ascii_case(&w.stack.name) {
                return Ok(Some(w.card_id));
            }
            return Ok(None);
        }
        match self.parse_object_text(t) {
            Ok(ObjRef::Card(id)) => Ok(Some(id)),
            Ok(ObjRef::Background(bg)) => Ok(self.w().stack.cards.iter().find(|c| c.background == bg).map(|c| c.id)),
            Ok(ObjRef::Stack) => Ok(Some(self.w().card_id)),
            Ok(_) => Ok(None),
            Err(Ctl::Error(_)) => {
                // A bare card name.
                let w = self.w();
                Ok(w.stack.cards.iter().find(|c| !c.name.is_empty() && c.name.eq_ignore_ascii_case(t)).map(|c| c.id))
            }
            Err(e) => Err(e),
        }
    }

    /// A stack file by name or path: as given, then beside the current
    /// stack, then beside Home.
    fn find_stack_file(&self, v: &str) -> Option<std::path::PathBuf> {
        let t = v.trim();
        let lower = t.to_ascii_lowercase();
        let name = if lower.starts_with("stack ") { t[6..].trim().trim_matches('"') } else { t.trim_matches('"') };
        if name.is_empty() {
            return None;
        }
        let w = self.w();
        let dirs = [w.stack.path.as_ref(), w.home_path.as_ref()].into_iter().flatten().filter_map(|p| p.parent());
        let mut candidates = vec![std::path::PathBuf::from(name), std::path::PathBuf::from(format!("{name}.rcstack"))];
        for dir in dirs {
            candidates.push(dir.join(name));
            candidates.push(dir.join(format!("{name}.rcstack")));
        }
        candidates.into_iter().find(|p| p.is_file())
    }

    /// The stack a name refers to: the current one, one already loaded, or
    /// one loaded now from its file (the built-in Tour has none). None if
    /// there's no such stack, or the user cancelled its password.
    pub(super) fn stack_ref(&mut self, name: &str) -> R<Option<ObjRef>> {
        let name = name.trim().trim_matches('"');
        {
            let w = self.w();
            if name.is_empty() || super::world::names_stack(&w.stack, name) {
                return Ok(Some(ObjRef::Stack));
            }
            if let Some(k) = w.find_other(name, None) {
                return Ok(Some(ObjRef::OtherStack(k)));
            }
        }
        let path = self.find_stack_file(name);
        if let Some(path) = path {
            let current = self.w().stack.path.clone();
            if super::world::same_path(current.as_deref(), Some(&path)) {
                return Ok(Some(ObjRef::Stack));
            }
            if let Some(k) = self.w().find_other("", Some(&path)) {
                return Ok(Some(ObjRef::OtherStack(k)));
            }
            let Some(stack) = self.load_stack_file(&path)? else { return Ok(None) };
            return Ok(Some(ObjRef::OtherStack(self.w().add_other(stack))));
        }
        if name.eq_ignore_ascii_case("Tour") {
            return Ok(Some(ObjRef::OtherStack(self.w().add_other(crate::model::Stack::demo()))));
        }
        Ok(None)
    }

    /// `start using stack "Library"`: its script joins the message path,
    /// after the current stack and before Home.
    fn start_using(&mut self, name: &str) -> R<()> {
        match self.stack_ref(name)? {
            Some(ObjRef::OtherStack(k)) => {
                {
                    let mut w = self.w();
                    let name = w.other(k).map(|s| s.name.clone()).unwrap_or_default();
                    if !w.is_in_use(&name) {
                        w.in_use.push(name);
                    }
                }
                self.send_system(ObjRef::OtherStack(k), "libraryStack")
            }
            // The current stack is in the path anyway.
            Some(_) => Ok(()),
            None => err(format!("No such stack \"{name}\"")),
        }
    }

    fn stop_using(&mut self, name: &str) -> R<()> {
        let target = self.stack_ref(name)?;
        let full = {
            let w = self.w();
            match target {
                Some(ObjRef::OtherStack(k)) => w.other(k).map(|s| s.name.clone()),
                _ => None,
            }
        };
        let Some(full) = full else { return Ok(()) };
        self.w().in_use.retain(|n| !n.eq_ignore_ascii_case(&full));
        if let Some(ObjRef::OtherStack(k)) = target {
            self.send_system(ObjRef::OtherStack(k), "releaseStack")?;
        }
        Ok(())
    }

    /// `go to stack "X"`, optionally to one of its cards.
    fn go_stack(&mut self, name: &str, card: Option<&CardSpec>) -> R<()> {
        match self.stack_ref(name)? {
            Some(ObjRef::OtherStack(k)) => self.go_to_stack(k)?,
            Some(_) => {}
            None => {
                self.result = "No such stack.".into();
                return Ok(());
            }
        }
        if let Some(spec) = card {
            match self.resolve_card(spec) {
                Ok(id) => self.go_card_id(id, true)?,
                Err(Ctl::Error(_)) => self.result = "No such card.".into(),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// `go home`: to the Home stack, or the first card if there's none.
    pub(super) fn go_home(&mut self) -> R<()> {
        let (is_home, key) = {
            let mut w = self.w();
            w.load_home();
            (w.is_home(&w.stack) || w.home_path.is_none(), w.home_key())
        };
        match key {
            Some(k) if !is_home => self.go_to_stack(k),
            _ => {
                let id = self.resolve_card(&CardSpec::Which(Which::Ordinal(Ordinal::Nth(1))))?;
                self.go_card_id(id, true)
            }
        }
    }

    /// Show a loaded stack, sending the close and open messages.
    pub(super) fn go_to_stack(&mut self, key: u32) -> R<()> {
        let card = self.current_card_ref();
        for m in ["closeCard", "closeBackground", "closeStack"] {
            self.send_system(card, m)?;
        }
        let Some(stack) = self.w().take_other(key) else { return Ok(()) };
        self.scripts_changed();
        self.w().replace_stack(stack);
        let card = self.current_card_ref();
        for m in ["openStack", "openBackground", "openCard"] {
            self.send_system(card, m)?;
        }
        Ok(())
    }

    /// Load a stack file, asking for the password of private-access stacks.
    /// Returns None if the user cancels.
    pub fn load_stack_file(&mut self, path: &std::path::Path) -> R<Option<crate::model::Stack>> {
        use crate::import::{NEEDS_PASSWORD, WRONG_PASSWORD, load_any};
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut password: Option<String> = None;
        let mut prompt = format!("\"{name}\" is password protected. Password:");
        loop {
            let stack = match load_any(path, password.as_deref()) {
                Ok(s) => s,
                Err(e) if e == NEEDS_PASSWORD || e == WRONG_PASSWORD => {
                    if e == WRONG_PASSWORD {
                        prompt = "That isn't the password. Try again:".into();
                    }
                    match self.ask_password(&prompt)? {
                        Some(p) => {
                            password = Some(p);
                            continue;
                        }
                        None => return Ok(None),
                    }
                }
                Err(e) => return err(e),
            };
            // RandomCard's own private-access stacks check the password here.
            let p = &stack.protection;
            if p.private_access
                && let Some(hash) = p.password_hash
                && !password.as_deref().is_some_and(|pw| crate::model::password::check(pw, hash))
            {
                match self.ask_password(&prompt)? {
                    Some(pw) if crate::model::password::check(&pw, hash) => return Ok(Some(stack)),
                    Some(_) => {
                        prompt = "That isn't the password. Try again:".into();
                        password = None;
                        continue;
                    }
                    None => return Ok(None),
                }
            }
            return Ok(Some(stack));
        }
    }

    fn ask_password(&mut self, prompt: &str) -> R<Option<String>> {
        let reply = self.dialog(Dialog::Ask { prompt: prompt.into(), default: String::new(), password: true })?;
        Ok(match reply {
            DialogReply::Text(Some(t)) | DialogReply::Button(t) => Some(t),
            DialogReply::Text(None) => None,
        })
    }

    fn go_history(&mut self, delta: i64) -> R<()> {
        let id = {
            let w = self.w();
            let pos = w.history_pos as i64 + delta;
            if pos < 0 || pos >= w.history.len() as i64 {
                return Ok(());
            }
            w.history[pos as usize]
        };
        if self.w().stack.card(id).is_none() {
            return Ok(());
        }
        self.go_card_id(id, false)?;
        let mut w = self.w();
        let pos = (w.history_pos as i64 + delta).clamp(0, w.history.len() as i64 - 1);
        w.history_pos = pos as usize;
        Ok(())
    }

    fn send_system(&mut self, target: ObjRef, msg: &str) -> R<()> {
        if self.lock_messages {
            return Ok(());
        }
        self.send_message(target, msg, &[]).map(|_| ())
    }

    /// Change cards, sending closeCard/openCard (and background) messages.
    pub fn go_card_id(&mut self, id: Id, record: bool) -> R<()> {
        self.go_card_with(id, record, None)
    }

    fn go_card_with(&mut self, id: Id, record: bool, extra: Option<&str>) -> R<()> {
        let (old, old_bg, new_bg) = {
            let w = self.w();
            let Some(c) = w.stack.card(id) else { return err("No such card") };
            (w.card_id, w.bg_id(), c.background)
        };
        if old == id && extra.is_none() {
            return Ok(());
        }
        let old_exists = self.w().stack.card(old).is_some();
        if old_exists && old != id {
            self.send_system(ObjRef::Card(old), "closeCard")?;
            if old_bg != new_bg {
                self.send_system(ObjRef::Card(old), "closeBackground")?;
            }
        }
        {
            let mut w = self.w();
            w.card_id = id;
            w.focus = None;
            if record {
                let pos = w.history_pos;
                w.history.truncate(pos + 1);
                w.history.push(id);
                if w.history.len() > 200 {
                    w.history.remove(0);
                }
                w.history_pos = w.history.len() - 1;
            }
        }
        self.play_pending_effect()?;
        if let Some(m) = extra {
            self.send_system(ObjRef::Card(id), m)?;
        }
        if old_bg != new_bg || !old_exists {
            self.send_system(ObjRef::Card(id), "openBackground")?;
        }
        self.send_system(ObjRef::Card(id), "openCard")
    }

    fn play_pending_effect(&mut self) -> R<()> {
        if self.w().lock_screen {
            return Ok(());
        }
        let Some((name, speed)) = self.pending_effect.take() else { return Ok(()) };
        let seconds = match speed.as_deref() {
            Some("very fast") => 0.15,
            Some("fast") => 0.3,
            Some("slow") => 0.9,
            Some("very slow") => 1.6,
            _ => 0.5,
        };
        {
            let mut w = self.w();
            w.transition_seq += 1;
            let seq = w.transition_seq;
            w.transition = Some(Transition { effect: name.to_ascii_lowercase(), seconds, seq });
        }
        // Let the effect play before the script continues, as HyperCard does.
        self.sleep(Duration::from_secs_f32(seconds))
    }

    pub fn arrow_key(&mut self, dir: &str) -> R<()> {
        let dir = dir.trim().to_ascii_lowercase();
        match dir.as_str() {
            "left" => {
                let id = self.resolve_card(&CardSpec::Position(Position::Prev))?;
                self.go_card_id(id, true)
            }
            "right" => {
                let id = self.next_card_id();
                self.go_card_id(id, true)
            }
            "up" => self.go_history(1),
            "down" => self.go_history(-1),
            _ => err(format!("Invalid arrow key \"{dir}\"")),
        }
    }

    /// `click at` with a paint tool paints a single point.
    fn paint_click(&mut self, x: i32, y: i32) {
        use crate::paint;
        let mut w = self.w();
        let (tool, settings) = (w.tool, w.paint_settings());
        if tool == Tool::Text {
            w.text_point = Some((x, y));
            return;
        }
        let Some(pic) = w.layer_picture_mut() else { return };
        match tool {
            Tool::Bucket => paint::fill(pic, x, y, settings.ink()),
            _ => {
                if let Some(t) = tool.drag_tool() {
                    paint::apply_drag(pic, t, (x, y), (x, y), &settings);
                }
            }
        }
        w.picture_rev += 1;
        w.dirty = true;
    }

    fn click_at(&mut self, x: i32, y: i32) -> R<()> {
        {
            let mut w = self.w();
            w.mouse.click_x = x;
            w.mouse.click_y = y;
        }
        let target = self.part_at(x, y).unwrap_or_else(|| self.current_card_ref());
        if let ObjRef::Part { layer, id, card } = target {
            let toggles = {
                let w = self.w();
                w.stack.part(card, layer, id).map(|p| {
                    p.kind == PartKind::Button
                        && p.auto_hilite
                        && matches!(p.style, crate::model::PartStyle::CheckBox | crate::model::PartStyle::Radio)
                })
            };
            if toggles == Some(true) {
                let mut w = self.w();
                let on = w.stack.hilite(card, layer, id);
                let radio = w.stack.part(card, layer, id).is_some_and(|p| p.style == crate::model::PartStyle::Radio);
                w.stack.set_hilite(card, layer, id, radio || !on);
            }
        }
        self.send_message(target, "mouseDown", &[])?;
        self.send_message(target, "mouseUp", &[])?;
        Ok(())
    }

    // ----- find -----

    fn find(&mut self, what: &str, mode: FindMode, only: Option<ObjRef>) -> R<()> {
        if what.is_empty() {
            return Ok(());
        }
        let needle = what.to_lowercase();
        let only_field = match only {
            Some(ObjRef::Part { layer, id, .. }) => Some((layer, id)),
            Some(_) => return err("Can only find in a field"),
            None => None,
        };
        // Every searchable field on every card, starting with the current card.
        let (fields, previous) = {
            let w = self.w();
            let s = &w.stack;
            let n = s.cards.len();
            let cur = w.card_index();
            let mut fields = Vec::new();
            for i in 0..n {
                let c = &s.cards[(cur + i) % n];
                if c.dont_search {
                    continue;
                }
                for layer in [Layer::Background, Layer::Card] {
                    if layer == Layer::Background && s.background(c.background).is_some_and(|b| b.dont_search) {
                        continue;
                    }
                    for p in s.parts(c.id, layer) {
                        if p.kind == PartKind::Field
                            && !p.dont_search
                            && only_field.is_none_or(|(l, i)| l == layer && i == p.id)
                        {
                            fields.push((c.id, layer, p.id));
                        }
                    }
                }
            }
            (fields, w.found.clone())
        };
        if fields.is_empty() {
            self.result = "not found".into();
            return Ok(());
        }
        // A repeated find continues after the previous match.
        let (first, offset) = previous
            .and_then(|f| {
                let i = fields.iter().position(|x| *x == (f.card, f.layer, f.field))?;
                Some((i, f.end))
            })
            .unwrap_or((0, 0));
        for k in 0..=fields.len() {
            let (card, layer, id) = fields[(first + k) % fields.len()];
            let text = self.w().stack.field_text(card, layer, id).to_lowercase();
            let from = if k == 0 { char_to_byte(&text, offset) } else { 0 };
            if let Some((s, e)) = find_in(&text, &needle, mode, from) {
                let start = text[..s].chars().count();
                let end = start + text[s..e].chars().count();
                if card != self.w().card_id {
                    self.go_card_id(card, true)?;
                }
                self.w().found = Some(FieldRange { card, layer, field: id, start, end });
                self.result.clear();
                return Ok(());
            }
        }
        self.w().found = None;
        self.result = "not found".into();
        self.w().beeps += 1;
        Ok(())
    }

    // ----- the selection -----

    /// The field and characters a chunk names: `word 2 of field 1`,
    /// `text of field "Notes"`, or `the selection` and chunks of it.
    pub(super) fn text_range(&mut self, e: &Expr) -> R<FieldRange> {
        let (chunks, base) = Self::container_parts(e);
        let (card, layer, id, text, offset) = if matches!(base, Expr::Selection) {
            let w = self.w();
            let Some(sel) = w.current_selection().cloned() else { return err("Nothing is selected") };
            let text = w.range_text(&sel);
            (sel.card, sel.layer, sel.field, text, sel.start)
        } else {
            let obj = self.resolve_dynamic(base)?;
            let ObjRef::Part { layer, id, card } = obj else { return err("Expected text in a field here") };
            let w = self.w();
            match w.stack.part(card, layer, id) {
                Some(p) if p.kind == PartKind::Field => (card, layer, id, w.stack.field_text(card, layer, id), 0),
                _ => return err("Expected text in a field here"),
            }
        };
        let resolved = self.resolve_chunks(&chunks, &text)?;
        let (a, b) = if resolved.is_empty() {
            (0, text.len())
        } else {
            crate::hypertalk::chunk::locate(&mut text.clone(), &resolved, &self.item_delimiter, false)
        };
        let (start, end) = (offset + text[..a].chars().count(), offset + text[..b].chars().count());
        Ok(FieldRange { card, layer, field: id, start, end })
    }

    /// `the textStyle of word 2 of field 1`: "mixed" if it varies.
    pub(super) fn chunk_property(&mut self, e: &Expr, prop: &str) -> R<String> {
        let r = self.text_range(e)?;
        let w = self.w();
        let Some(part) = w.stack.part(r.card, r.layer, r.field) else { return Ok(String::new()) };
        let runs = w.stack.field_styles(r.card, r.layer, r.field);
        let mut values: Vec<String> = styles::styles_in(&runs, r.start, r.end)
            .iter()
            .map(|s| match prop {
                "textfont" => s.font.clone().unwrap_or_else(|| part.text_font.clone()),
                "textsize" => s.size.unwrap_or(part.text_size).to_string(),
                "textstyle" => styles::normalize_style(s.style.as_deref().unwrap_or(&part.text_style)),
                _ => String::new(),
            })
            .collect();
        values.dedup();
        Ok(if values.len() == 1 { values.remove(0) } else { "mixed".into() })
    }

    /// `set the textStyle of word 2 of field 1 to bold`.
    fn set_chunk_property(&mut self, e: &Expr, prop: &str, v: &str) -> R<()> {
        let r = self.text_range(e)?;
        let size = if prop == "textsize" { Some(to_int(v)?.clamp(6, 128) as u32) } else { None };
        let mut w = self.w();
        let Some(part) = w.stack.part(r.card, r.layer, r.field).cloned() else { return Ok(()) };
        let text = w.stack.field_text(r.card, r.layer, r.field);
        let mut runs = w.stack.field_styles(r.card, r.layer, r.field);
        let len = text.chars().count();
        // A setting that matches the field's own is left to the field.
        match prop {
            "textfont" => {
                let font = (!v.eq_ignore_ascii_case(&part.text_font)).then(|| v.to_string());
                styles::apply(&mut runs, r.start, r.end, len, |s| s.font = font.clone());
            }
            "textsize" => {
                let size = size.filter(|s| *s != part.text_size);
                styles::apply(&mut runs, r.start, r.end, len, |s| s.size = size);
            }
            "textstyle" => {
                let style = styles::normalize_style(v);
                let style = (style != styles::normalize_style(&part.text_style)).then_some(style);
                styles::apply(&mut runs, r.start, r.end, len, |s| s.style = style.clone());
            }
            _ => return err(format!("Can't set the {prop} of a chunk")),
        }
        w.stack.set_field_content(r.card, r.layer, r.field, text, runs);
        w.dirty = true;
        Ok(())
    }

    /// The Style menu (Plain, Bold, …, sizes) and Font menu, for the
    /// selected text or, with the text tool, for painted text.
    fn text_menu(&mut self, key: &str) -> R<()> {
        let size = key.strip_suffix(" point").unwrap_or(key).parse::<u32>().ok();
        let font = styles::FONTS.iter().find(|f| f.eq_ignore_ascii_case(key)).map(|f| f.to_string());
        if self.w().tool == Tool::Text {
            let mut w = self.w();
            match (size, font) {
                (Some(s), _) => w.paint_text_size = s,
                (_, Some(f)) => w.paint_font = f,
                _ if key == "plain" => w.paint_text_style = "plain".into(),
                _ => {
                    let on = !styles::has_style(&w.paint_text_style, key);
                    w.paint_text_style = styles::with_style(&w.paint_text_style, key, on);
                }
            }
            return Ok(());
        }
        let Some(sel) = self.w().current_selection().cloned().filter(|s| s.start < s.end) else { return Ok(()) };
        let target = Expr::Selection;
        match (size, font) {
            (Some(s), _) => self.set_chunk_property(&target, "textsize", &s.to_string()),
            (_, Some(f)) => self.set_chunk_property(&target, "textfont", &f),
            _ if key == "plain" => self.set_chunk_property(&target, "textstyle", "plain"),
            _ => {
                // Bold toggles: off if all the selection is bold, else on.
                let all_on = {
                    let w = self.w();
                    let part_style = w.stack.part(sel.card, sel.layer, sel.field).map(|p| p.text_style.clone()).unwrap_or_default();
                    let runs = w.stack.field_styles(sel.card, sel.layer, sel.field);
                    styles::styles_in(&runs, sel.start, sel.end)
                        .iter()
                        .all(|s| styles::has_style(s.style.as_deref().unwrap_or(&part_style), key))
                };
                let mut w = self.w();
                let text = w.stack.field_text(sel.card, sel.layer, sel.field);
                let part_style = w.stack.part(sel.card, sel.layer, sel.field).map(|p| p.text_style.clone()).unwrap_or_default();
                let mut runs = w.stack.field_styles(sel.card, sel.layer, sel.field);
                styles::apply(&mut runs, sel.start, sel.end, text.chars().count(), |s| {
                    let style = styles::with_style(s.style.as_deref().unwrap_or(&part_style), key, !all_on);
                    s.style = (style != styles::normalize_style(&part_style)).then_some(style);
                });
                w.stack.set_field_content(sel.card, sel.layer, sel.field, text, runs);
                w.dirty = true;
                Ok(())
            }
        }
    }

    /// `select line 2 of field 1`, `select after text of field "Notes"`.
    fn select_text(&mut self, prep: Prep, e: &Expr) -> R<()> {
        let FieldRange { card, layer, field: id, mut start, mut end } = self.text_range(e)?;
        let lock_text = self.w().stack.part(card, layer, id).is_some_and(|p| p.lock_text);
        match prep {
            Prep::Before => end = start,
            Prep::After => start = end,
            Prep::Into => {}
        }
        let mut w = self.w();
        w.tool = Tool::Browse;
        // An unlocked field opens for typing; a locked one shows the
        // selection hilited, as list fields do.
        w.focus = (!lock_text).then_some((layer, id));
        w.set_selection(Some(FieldRange { card, layer, field: id, start, end }));
        Ok(())
    }

    /// `the selection` as a container: the selected text.
    pub(super) fn selected_text(&self) -> String {
        let w = self.w();
        w.current_selection().map(|r| w.range_text(r)).unwrap_or_default()
    }

    /// Replace the selected text, leaving the insertion point after it.
    pub(super) fn replace_selection(&mut self, value: &str) {
        let mut w = self.w();
        let Some(r) = w.current_selection().cloned() else { return };
        let text = w.stack.field_text(r.card, r.layer, r.field);
        let byte = |i: usize| text.char_indices().nth(i).map(|(b, _)| b).unwrap_or(text.len());
        let mut new = text.clone();
        new.replace_range(byte(r.start)..byte(r.end.max(r.start)), value);
        w.stack.set_field_text(r.card, r.layer, r.field, new);
        w.dirty = true;
        let at = r.start + value.chars().count();
        w.set_selection(Some(FieldRange { start: at, end: at, ..r }));
    }

    // ----- sorting and marking -----

    fn sort_key(style: SortStyle, v: &str) -> (f64, String) {
        match style {
            SortStyle::Numeric => (parse_number(v).unwrap_or(f64::NEG_INFINITY), String::new()),
            SortStyle::DateTime => (
                dates::parse_date(v).map(|d| dates::to_seconds(d) as f64).unwrap_or(f64::NEG_INFINITY),
                String::new(),
            ),
            SortStyle::Text => (0.0, v.to_lowercase()),
        }
    }

    fn sort_chunks(
        &mut self,
        kind: ChunkKind,
        container: &Expr,
        ascending: bool,
        style: SortStyle,
        by: Option<&Expr>,
    ) -> R<()> {
        let text = self.eval(container)?;
        let delim = match kind {
            ChunkKind::Line => "\n".to_string(),
            ChunkKind::Item => self.item_delimiter.clone(),
            ChunkKind::Word => " ".into(),
            ChunkKind::Char => String::new(),
        };
        let pieces: Vec<String> = match kind {
            ChunkKind::Char => text.chars().map(String::from).collect(),
            ChunkKind::Word => text.split_whitespace().map(String::from).collect(),
            _ => {
                let trailing = text.ends_with(delim.as_str());
                let mut v: Vec<String> = text.split(delim.as_str()).map(String::from).collect();
                if trailing {
                    v.pop();
                }
                v
            }
        };
        let mut keyed = Vec::new();
        for p in pieces {
            let key_src = match by {
                Some(e) => {
                    self.set_var("each", p.clone());
                    self.eval(e)?
                }
                None => p.clone(),
            };
            keyed.push((Self::sort_key(style, &key_src), p));
        }
        keyed.sort_by(|a, b| {
            let o = a.0.0.partial_cmp(&b.0.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.1.cmp(&b.0.1));
            if ascending { o } else { o.reverse() }
        });
        let sorted: Vec<String> = keyed.into_iter().map(|(_, p)| p).collect();
        self.put_into(container, Prep::Into, &sorted.join(&delim))
    }

    /// Evaluate an expression in the context of each card (e.g. `field "Name"`).
    fn per_card<T>(&mut self, ids: Vec<Id>, mut f: impl FnMut(&mut Self, Id) -> R<T>) -> R<Vec<(Id, T)>> {
        let original = self.w().card_id;
        let mut out = Vec::new();
        for id in ids {
            self.w().card_id = id;
            let r = f(self, id);
            if r.is_err() {
                self.w().card_id = original;
            }
            out.push((id, r?));
        }
        self.w().card_id = original;
        Ok(out)
    }

    fn sort_cards(&mut self, ascending: bool, style: SortStyle, by: &Expr, marked_only: bool, bg: Option<Id>) -> R<()> {
        // Sort the chosen cards among the slots they occupy.
        let slots: Vec<usize> = {
            let w = self.w();
            (0..w.stack.cards.len())
                .filter(|i| !marked_only || w.stack.cards[*i].marked)
                .filter(|i| bg.is_none_or(|b| w.stack.cards[*i].background == b))
                .collect()
        };
        let ids: Vec<Id> = {
            let w = self.w();
            slots.iter().map(|i| w.stack.cards[*i].id).collect()
        };
        let keys = self.per_card(ids, |me, _| {
            let v = me.eval(by)?;
            Ok(Self::sort_key(style, &v))
        })?;
        let mut w = self.w();
        let mut chosen: Vec<(Card, (f64, String))> =
            slots.iter().zip(keys).map(|(i, (_, k))| (w.stack.cards[*i].clone(), k)).collect();
        chosen.sort_by(|a, b| {
            let o = a.1.0.partial_cmp(&b.1.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.1.cmp(&b.1.1));
            if ascending { o } else { o.reverse() }
        });
        for (slot, (card, _)) in slots.into_iter().zip(chosen) {
            w.stack.cards[slot] = card;
        }
        w.dirty = true;
        Ok(())
    }

    fn mark(&mut self, what: &MarkWhat, on: bool) -> R<()> {
        match what {
            MarkWhat::All => {
                let mut w = self.w();
                for c in &mut w.stack.cards {
                    c.marked = on;
                }
            }
            MarkWhat::Card(e) => match self.resolve_dynamic(e)? {
                ObjRef::Card(id) => {
                    if let Some(c) = self.w().stack.card_mut(id) {
                        c.marked = on;
                    }
                }
                _ => return err("Can only mark cards"),
            },
            MarkWhat::Where(cond) => {
                let ids: Vec<Id> = self.w().stack.cards.iter().map(|c| c.id).collect();
                let results = self.per_card(ids, |me, _| me.eval_bool(cond))?;
                let mut w = self.w();
                for (id, hit) in results {
                    if hit && let Some(c) = w.stack.card_mut(id) {
                        c.marked = on;
                    }
                }
            }
            MarkWhat::Finding(mode, what, field) => {
                let needle = self.eval(what)?.to_lowercase();
                let field = match field {
                    Some(f) => Some(self.resolve_dynamic(f)?),
                    None => None,
                };
                let mut w = self.w();
                let ids: Vec<Id> = w.stack.cards.iter().map(|c| c.id).collect();
                for id in ids {
                    let hit = [Layer::Background, Layer::Card].iter().any(|l| {
                        w.stack.parts(id, *l).iter().any(|p| {
                            p.kind == PartKind::Field
                                && match field {
                                    Some(ObjRef::Part { layer, id: fid, .. }) => *l == layer && p.id == fid,
                                    _ => true,
                                }
                                && find_in(&w.stack.field_text(id, *l, p.id).to_lowercase(), &needle, *mode, 0)
                                    .is_some()
                        })
                    });
                    if hit && let Some(c) = w.stack.card_mut(id) {
                        c.marked = on;
                    }
                }
            }
        }
        self.w().dirty = true;
        Ok(())
    }

    // ----- deleting -----

    fn delete(&mut self, e: &Expr) -> R<()> {
        if is_menu(e) {
            return Ok(());
        }
        if let Expr::Menu(r) = e {
            return self.delete_menu(r);
        }
        if matches!(e, Expr::Chunk(..)) {
            return self.delete_chunk(e);
        }
        if let Expr::Var(name) = e
            && self.get_var(name).is_some()
        {
            // `delete x` empties a variable.
            self.set_var(name, String::new());
            return Ok(());
        }
        match self.resolve_dynamic(e)? {
            ObjRef::Part { layer, id, card } => {
                let msg = {
                    let w = self.w();
                    match w.stack.part(card, layer, id).map(|p| p.kind) {
                        Some(PartKind::Button) => "deleteButton",
                        _ => "deleteField",
                    }
                };
                self.send_system(ObjRef::Part { layer, id, card }, msg)?;
                let mut w = self.w();
                w.record_parts(layer);
                let bg = w.bg_id();
                let parts = match layer {
                    Layer::Card => w.stack.card_mut(card).map(|c| &mut c.parts),
                    Layer::Background => w.stack.background_mut(bg).map(|b| &mut b.parts),
                };
                if let Some(parts) = parts {
                    parts.retain(|p| p.id != id);
                }
                w.dirty = true;
                Ok(())
            }
            ObjRef::Card(id) => self.delete_card(id),
            ObjRef::MsgBox => {
                self.w().msg.clear();
                Ok(())
            }
            o => err(format!("Can't delete {}", self.object_name(o))),
        }
    }

    fn delete_card(&mut self, id: Id) -> R<()> {
        let (count, cant, bg) = {
            let w = self.w();
            let c = w.stack.card(id);
            (w.stack.cards.len(), c.is_some_and(|c| c.cant_delete), c.map(|c| c.background).unwrap_or(0))
        };
        if count <= 1 {
            return err("Can't delete the last card of a stack");
        }
        if cant {
            return err("This card can't be deleted (cantDelete is true)");
        }
        self.send_system(ObjRef::Card(id), "deleteCard")?;
        self.w().record_card_deleted(id);
        let current = self.w().card_id;
        if current == id {
            let idx = self.w().stack.card_index(id).unwrap_or(0);
            let next = {
                let w = self.w();
                if idx + 1 < w.stack.cards.len() { w.stack.cards[idx + 1].id } else { w.stack.cards[idx - 1].id }
            };
            self.go_card_id(next, false)?;
        }
        let mut w = self.w();
        w.stack.cards.retain(|c| c.id != id);
        // A background with no cards left goes too.
        if !w.stack.cards.iter().any(|c| c.background == bg) {
            w.stack.backgrounds.retain(|b| b.id != bg);
        }
        w.history.retain(|h| *h != id);
        if w.history.is_empty() {
            let cid = w.card_id;
            w.history.push(cid);
        }
        w.history_pos = w.history_pos.min(w.history.len() - 1);
        w.dirty = true;
        Ok(())
    }

    // ----- menus -----

    pub fn perform_menu(&mut self, item: &str) -> R<()> {
        // A script's own items: their menuMessage, or nothing to do.
        let custom = {
            let w = self.w();
            menus::find_item(&w.menus, item).map(|i| (i.message.clone(), i.builtin))
        };
        match custom {
            Some((msg, _)) if !msg.is_empty() => {
                let card = self.current_card_ref();
                return self.send_text(&msg, card);
            }
            Some((_, false)) => return Ok(()),
            _ => {}
        }
        let key = menus::item_key(item);
        match key.as_str() {
            "protect stack" => self.request(UiRequest::ProtectStack),
            "preferences" => self.request(UiRequest::Preferences),
            "tool panel" => self.request(UiRequest::ToggleToolPanel),
            "message watcher" => self.request(UiRequest::MessageWatcher(true)),
            "variable watcher" => self.request(UiRequest::VariableWatcher(true)),
            "new card" => {
                let id = {
                    let mut w = self.w();
                    let (width, height, bg, idx) = (w.stack.width, w.stack.height, w.bg_id(), w.card_index());
                    let id = w.stack.alloc_id();
                    let picture = Picture::new(width, height);
                    let card = Card { id, background: bg, picture, show_pict: true, ..Card::default() };
                    w.stack.cards.insert(idx + 1, card);
                    w.record(UndoEntry::CardAdded { id });
                    w.dirty = true;
                    id
                };
                self.go_card_with(id, true, Some("newCard"))
            }
            "delete card" => {
                let id = self.w().card_id;
                self.delete_card(id)
            }
            "cut card" => {
                let id = self.w().card_id;
                let card = self.w().stack.card(id).cloned();
                self.clipboard_card = card;
                self.delete_card(id)
            }
            "copy card" => {
                let id = self.w().card_id;
                let card = self.w().stack.card(id).cloned();
                self.clipboard_card = card;
                Ok(())
            }
            "paste card" => {
                let Some(mut card) = self.clipboard_card.clone() else { return Ok(()) };
                let id = {
                    let mut w = self.w();
                    if w.stack.background(card.background).is_none() {
                        card.background = w.bg_id();
                        card.bg_text.clear();
                        card.bg_styles.clear();
                    }
                    card.id = w.stack.alloc_id();
                    for p in &mut card.parts {
                        p.id = w.stack.alloc_id();
                    }
                    let idx = w.card_index();
                    let id = card.id;
                    w.stack.cards.insert(idx + 1, card);
                    w.record(UndoEntry::CardAdded { id });
                    w.dirty = true;
                    id
                };
                self.go_card_id(id, true)
            }
            "new background" => {
                let (id, _bg) = {
                    let mut w = self.w();
                    let (width, height) = (w.stack.width, w.stack.height);
                    let bg = w.stack.alloc_id();
                    w.stack.backgrounds.push(Background {
                        id: bg,
                        picture: Picture::new(width, height),
                        show_pict: true,
                        ..Background::default()
                    });
                    let id = w.stack.alloc_id();
                    let idx = w.card_index();
                    w.stack.cards.insert(
                        idx + 1,
                        Card { id, background: bg, picture: Picture::new(width, height), show_pict: true, ..Card::default() },
                    );
                    w.record(UndoEntry::CardAdded { id });
                    w.dirty = true;
                    (id, bg)
                };
                self.go_card_with(id, true, Some("newBackground"))?;
                self.send_system(ObjRef::Card(id), "newCard")
            }
            "new button" | "new field" => {
                let kind = if key == "new button" { PartKind::Button } else { PartKind::Field };
                let obj = {
                    let mut w = self.w();
                    let layer = w.current_layer();
                    let (cw, ch) = (w.stack.width as i32, w.stack.height as i32);
                    let id = w.stack.alloc_id();
                    let card = w.card_id;
                    let bg = w.bg_id();
                    w.record_parts(layer);
                    let part = match kind {
                        PartKind::Button => {
                            Part::new_button(id, "New Button", Rect::new(cw / 2 - 60, ch / 2 - 12, cw / 2 + 60, ch / 2 + 12))
                        }
                        PartKind::Field => {
                            Part::new_field(id, "", Rect::new(cw / 2 - 90, ch / 2 - 50, cw / 2 + 90, ch / 2 + 50))
                        }
                    };
                    match layer {
                        Layer::Card => {
                            if let Some(c) = w.stack.card_mut(card) {
                                c.parts.push(part);
                            }
                        }
                        Layer::Background => {
                            if let Some(b) = w.stack.background_mut(bg) {
                                b.parts.push(part);
                            }
                        }
                    }
                    w.tool = if kind == PartKind::Button { Tool::Button } else { Tool::Field };
                    w.dirty = true;
                    let obj = ObjRef::Part { layer, id, card };
                    w.requests.push(UiRequest::SelectPart(obj));
                    obj
                };
                self.send_system(obj, if kind == PartKind::Button { "newButton" } else { "newField" })
            }
            "background" => {
                let mut w = self.w();
                w.edit_bg = !w.edit_bg;
                Ok(())
            }
            "first" | "prev" | "previous" | "next" | "last" => {
                let spec = match key.as_str() {
                    "first" => CardSpec::Which(Which::Ordinal(Ordinal::Nth(1))),
                    "last" => CardSpec::Which(Which::Ordinal(Ordinal::Last)),
                    "next" => CardSpec::Position(Position::Next),
                    _ => CardSpec::Position(Position::Prev),
                };
                let id = self.resolve_card(&spec)?;
                self.go_card_id(id, true)
            }
            "back" => self.go_history(-1),
            "forth" => self.go_history(1),
            "home" => self.go_home(),
            "recent" => {
                let id = self.resolve_card(&CardSpec::Which(Which::Ordinal(Ordinal::Nth(1))))?;
                self.go_card_id(id, true)
            }
            "message" | "message box" => {
                let mut w = self.w();
                w.msg_visible = !w.msg_visible;
                Ok(())
            }
            "save stack" | "save" => self.save_stack(None),
            "save a copy" | "save stack as" | "save as" => {
                self.w().requests.push(UiRequest::SaveStackAs(None));
                Ok(())
            }
            "open stack" | "open" => self.request(UiRequest::OpenStack),
            "import hypercard stack" | "import stack" => self.request(UiRequest::ImportStack),
            "new stack" => self.request(UiRequest::NewStack),
            "quit hypercard" | "quit randomcard" | "quit" => self.request(UiRequest::Quit),
            "button info" | "field info" => self.request(UiRequest::SelectedPartInfo),
            "card info" => self.request(UiRequest::CardInfo),
            "bkgnd info" | "background info" => self.request(UiRequest::BgInfo),
            "stack info" => self.request(UiRequest::StackInfo),
            "card script" => {
                let o = self.current_card_ref();
                self.request(UiRequest::EditScript(o))
            }
            "bkgnd script" | "background script" => {
                let bg = self.w().bg_id();
                self.request(UiRequest::EditScript(ObjRef::Background(bg)))
            }
            "stack script" => self.request(UiRequest::EditScript(ObjRef::Stack)),
            "bring closer" => self.request(UiRequest::BringCloser),
            "send farther" => self.request(UiRequest::SendFarther),
            "bring to front" => self.request(UiRequest::BringToFront),
            "send to back" => self.request(UiRequest::SendToBack),
            "undo" => self.request(UiRequest::Undo),
            "redo" => self.request(UiRequest::Redo),
            // Cut, Copy, Paste and Clear act on the selected part or picture,
            // e.g. "Copy Button", "Paste Field", "Clear Picture".
            _ if edit_op(&key).is_some() => self.request(UiRequest::Edit(edit_op(&key).unwrap())),
            "browse" | "button" | "field" | "select" | "pencil" | "brush" | "eraser" | "line" | "spray"
            | "spray can" | "rectangle" | "round rect" | "bucket" | "oval" => {
                let tool = Tool::parse(&key).unwrap_or(Tool::Browse);
                let mut w = self.w();
                w.tool = tool;
                w.focus = None;
                Ok(())
            }
            "select all" => self.request(UiRequest::SelectAll),
            _ if crate::paint::PaintOp::parse(&key).is_some() => {
                self.request(UiRequest::PaintOp(crate::paint::PaintOp::parse(&key).unwrap()))
            }
            "clear picture" => {
                let mut w = self.w();
                w.record_picture();
                if let Some(pic) = w.layer_picture_mut() {
                    pic.clear();
                }
                w.picture_rev += 1;
                w.dirty = true;
                Ok(())
            }
            _ if is_text_menu_item(&key) => self.text_menu(&key),
            "compact stack" | "about hypercard" | "about randomcard" | "help" => Ok(()),
            _ => {
                self.result = format!("Can't find menu item \"{item}\"");
                Ok(())
            }
        }
    }

    fn request(&mut self, r: UiRequest) -> R<()> {
        self.w().requests.push(r);
        Ok(())
    }

    fn save_stack(&mut self, name: Option<String>) -> R<()> {
        let path = match name {
            Some(n) => {
                let p = std::path::PathBuf::from(&n);
                let p = if p.extension().is_none() { p.with_extension("rcstack") } else { p };
                let dir = self.w().stack.path.as_ref().and_then(|x| x.parent().map(|d| d.to_path_buf()));
                match dir {
                    Some(d) if p.is_relative() => d.join(p),
                    _ => p,
                }
            }
            None if self.w().stack.cant_modify => {
                return err("This stack can't be modified, so it can't be saved. Use Save a Copy instead.");
            }
            None => match self.w().stack.path.clone() {
                Some(p) => p,
                None => {
                    self.w().requests.push(UiRequest::SaveStackAs(None));
                    return Ok(());
                }
            },
        };
        let mut w = self.w();
        w.stack.save(&path).or_else(err)?;
        w.stack.path = Some(path);
        w.dirty = false;
        Ok(())
    }
}

fn edit_op(item: &str) -> Option<EditOp> {
    let mut words = item.split_whitespace();
    let op = match words.next()? {
        "cut" => EditOp::Cut,
        "copy" => EditOp::Copy,
        "paste" => EditOp::Paste,
        "clear" => EditOp::Clear,
        _ => return None,
    };
    match words.next() {
        // "Clear Picture" erases the whole picture (see perform_menu).
        Some("picture") if op == EditOp::Clear => None,
        None | Some("button" | "field" | "picture") if words.next().is_none() => Some(op),
        _ => None,
    }
}

fn is_menu(e: &Expr) -> bool {
    matches!(e, Expr::Call(n, _) if n == "__menu")
}

fn char_to_byte(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map(|(i, _)| i).unwrap_or(s.len())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\''
}

/// Find `needle` in `hay` (both lowercase) from byte `from`, per HyperTalk find modes.
pub fn find_in(hay: &str, needle: &str, mode: FindMode, from: usize) -> Option<(usize, usize)> {
    if from > hay.len() {
        return None;
    }
    let at_word_start = |i: usize| i == 0 || !hay[..i].chars().last().is_some_and(is_word_char);
    let at_word_end = |j: usize| j >= hay.len() || !hay[j..].chars().next().is_some_and(is_word_char);
    let mut start = from;
    while start <= hay.len() {
        let off = hay[start..].find(needle)?;
        let i = start + off;
        let j = i + needle.len();
        let ok = match mode {
            FindMode::Chars | FindMode::String => true,
            FindMode::Normal => at_word_start(i),
            FindMode::Word | FindMode::Whole => at_word_start(i) && at_word_end(j),
        };
        if ok {
            return Some((i, j));
        }
        start = i + hay[i..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    None
}

fn is_text_menu_item(key: &str) -> bool {
    let size = key.strip_suffix(" point").unwrap_or(key).parse::<u32>().is_ok_and(|s| styles::SIZES.contains(&s));
    key == "plain" || styles::STYLES.contains(&key) || size || styles::FONTS.iter().any(|f| f.eq_ignore_ascii_case(key))
}
