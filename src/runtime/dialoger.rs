//! Dialoger, Leonard Buck's external for dialogs built from a stack's
//! 'DLOG' and 'DITL' resources (later sold as Dialoger Professional, by
//! theResult Software). The Developer Info Assistant and Tools Advisor make
//! their dialogs with it.
//!
//! `Dialoger(name & return & options, items…)` shows the dialog called
//! `name`. Options are lines: `float` (it stays open and Dialoger returns at
//! once), `DefaultItem 2`, `CancelItem 2`, `Title "…"`, and `on Handler`, the
//! hit handler. Each item argument is `"N◊setting"`: a setting, data on the
//! lines after it, or a value. `StyledText font,size,style` styles text,
//! `userList font,size[,SingleSelection]` and `userPop n` make lists and
//! pop-ups of the lines that follow, `userBox Gray` a box, `PictButton` a
//! picture or icon that clicks, `hide`, `noDismiss`; a value sets a check
//! box (1 or true) or an item's text. An argument without "N◊" is, second,
//! the text for ^0 to ^3, and later, the item at its place.
//!
//! A modal dialog's answer is its items' values, a line each: a button's
//! name if it closed the dialog, a check box's true or false, a list's
//! chosen line, a pop-up's choice, and text items' text. `dContents(dialog,
//! item, "all" | "long" | "rect")` reads an item while it's open.

use super::interp::{Interp, R};
use super::world::{Dialog, DialogReply, ScriptWindow, WindowKind};
use crate::model::Rect;
use crate::model::dialogs::{DialogItemKind, DialogTemplate};

/// The marker between an item's number and its setting: "4◊userPop".
pub const MARK: char = '◊';

/// How an item looks and acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Button,
    CheckBox,
    Radio,
    Static,
    Edit,
    Icon,
    Picture,
    List,
    Popup,
    Box,
    User,
}

/// One of an open dialog's items.
#[derive(Clone, Debug)]
pub struct Item {
    pub look: Look,
    pub rect: Rect,
    /// Clicks on it are reported (the DITL's "enabled").
    pub enabled: bool,
    /// Not grayed out (`inactivate`).
    pub active: bool,
    pub visible: bool,
    pub text: String,
    /// A check box or radio button's state.
    pub on: bool,
    /// An icon's or picture's id.
    pub id: i16,
    /// A list's or pop-up's lines.
    pub lines: Vec<String>,
    /// The chosen lines, from 0.
    pub selected: Vec<usize>,
    /// How many lines a list lets the user choose: 0 none, 1 one, 2 any.
    pub choose: u8,
    /// Its font, size and style, from StyledText or userList.
    pub font: Option<(String, u32, String)>,
    /// It closes the dialog when clicked: items 1 and 2 (OK and Cancel),
    /// unless told otherwise.
    pub dismisses: bool,
    /// A picture or icon that clicks like a button.
    pub pict_button: bool,
    /// A picture or icon button that's one of a set of choices.
    pub radio: bool,
    /// The item a double-click in a list counts as (SetDblClick); 0 for
    /// the list itself.
    pub double_click: usize,
    /// Text to select and type in: the UI gives it the keyboard.
    pub focus: bool,
    /// A box's pattern: Black, Gray, LtGray…
    pub pattern: String,
}

/// An open Dialoger dialog.
#[derive(Clone, Debug)]
pub struct Dialoger {
    pub title: String,
    pub proc_id: i16,
    pub close_box: bool,
    pub items: Vec<Item>,
    /// The hit handler: sent dialog, item, part.
    pub handler: String,
    /// It floats, and Dialoger returned when it opened.
    pub floating: bool,
    /// The item Return chooses, from 1; 0 for none.
    pub default_item: usize,
    /// The item Escape chooses.
    pub cancel_item: usize,
    /// The text for ^0 to ^3.
    pub params: Vec<String>,
    /// The item that closed it.
    pub hit: usize,
    pub dismissed: bool,
}

impl Dialoger {
    fn new(t: &DialogTemplate) -> Dialoger {
        let items: Vec<Item> = t
            .items
            .iter()
            .enumerate()
            .map(|(n, i)| {
                let look = match i.kind {
                    DialogItemKind::Button => Look::Button,
                    DialogItemKind::CheckBox => Look::CheckBox,
                    DialogItemKind::RadioButton => Look::Radio,
                    DialogItemKind::StaticText => Look::Static,
                    DialogItemKind::EditText => Look::Edit,
                    DialogItemKind::Icon => Look::Icon,
                    DialogItemKind::Picture => Look::Picture,
                    // Dialoger's own kinds, kept in the user item.
                    DialogItemKind::User => match i.id {
                        1 => Look::List,
                        2 => Look::Popup,
                        4 => Look::Box,
                        _ => Look::User,
                    },
                    DialogItemKind::Control => Look::User,
                };
                Item {
                    look,
                    rect: i.rect,
                    enabled: i.enabled,
                    active: true,
                    visible: true,
                    text: i.text.clone(),
                    on: false,
                    id: i.id,
                    lines: Vec::new(),
                    selected: if look == Look::Popup { vec![0] } else { Vec::new() },
                    choose: 1,
                    font: None,
                    // Items 1 and 2 (OK and Cancel) close the dialog; other
                    // buttons are the hit handler's to act on.
                    dismisses: n < 2,
                    pict_button: false,
                    radio: false,
                    double_click: 0,
                    focus: false,
                    pattern: if look == Look::Box { "Black".into() } else { String::new() },
                }
            })
            .collect();
        let button = |n: usize| items.get(n.wrapping_sub(1)).is_some_and(|i| i.look == Look::Button);
        let cancel = items
            .iter()
            .position(|i| i.look == Look::Button && i.text.trim().eq_ignore_ascii_case("cancel"))
            .map(|i| i + 1)
            .unwrap_or(0);
        Dialoger {
            title: if t.title.is_empty() { t.name.clone() } else { t.title.clone() },
            proc_id: t.proc_id,
            close_box: t.close_box,
            default_item: if button(1) { 1 } else { 0 },
            cancel_item: cancel,
            items,
            handler: String::new(),
            floating: false,
            params: Vec::new(),
            hit: 0,
            dismissed: false,
        }
    }

    /// The dialog's own settings: from the lines after its name, or "0◊…".
    pub fn option(&mut self, line: &str) {
        let line = line.trim();
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        let n = || rest.parse::<usize>().unwrap_or(0);
        match word.to_ascii_lowercase().as_str() {
            "on" => self.handler = rest.to_string(),
            "float" | "modeless" => self.floating = true,
            "modal" => self.floating = false,
            "defaultitem" | "default" => self.default_item = n(),
            "cancelitem" => self.cancel_item = n(),
            "title" => self.title = rest.trim_matches('"').to_string(),
            "dismiss" => self.dismissed = true,
            _ => {}
        }
    }

    /// Apply an item setting: "N◊…", without the "N◊". Its lines are
    /// settings, and values with the lines after them up to the next
    /// setting.
    pub fn set(&mut self, n: usize, spec: &str) {
        if n == 0 {
            for line in spec.lines() {
                self.option(line);
            }
            return;
        }
        if n > self.items.len() {
            return;
        }
        let lines: Vec<&str> = spec.split('\n').collect();
        let mut at = 0;
        while at < lines.len() {
            let line = lines[at];
            let (word, rest) = line.trim().split_once(char::is_whitespace).unwrap_or((line.trim(), ""));
            let word = word.to_ascii_lowercase();
            let item = &mut self.items[n - 1];
            at += 1;
            match word.as_str() {
                "styledtext" | "usertext" => item.font = font(rest),
                "userlist" => {
                    item.look = Look::List;
                    // Its third part is how many lines it lets the user choose.
                    item.font = font(&rest.split(',').take(2).collect::<Vec<_>>().join(","));
                    let flag = rest.split(',').nth(2).unwrap_or("").trim().to_ascii_lowercase();
                    item.choose = match flag.as_str() {
                        "noselection" => 0,
                        "anyselection" | "contiguousselection" => 2,
                        _ => 1,
                    };
                    item.lines = lines[at..].iter().map(|s| s.to_string()).collect();
                    item.selected.clear();
                    return;
                }
                "userpop" => {
                    item.look = Look::Popup;
                    let chosen = rest.trim().parse::<usize>().unwrap_or(1).max(1);
                    item.lines = lines[at..].iter().map(|s| s.to_string()).collect();
                    item.selected = vec![(chosen - 1).min(item.lines.len().saturating_sub(1))];
                    return;
                }
                "userbox" => {
                    item.look = Look::Box;
                    if !rest.trim().is_empty() {
                        item.pattern = rest.trim().to_string();
                    }
                }
                // PictButton frames,choices,Push: a picture or icon that
                // clicks like a push button (and closes the dialog), or a
                // radio button.
                "pictbutton" => {
                    item.pict_button = true;
                    let how = rest.split(',').nth(2).unwrap_or("push").trim().to_ascii_lowercase();
                    item.radio = how == "radio";
                }
                "hide" => item.visible = false,
                "show" => item.visible = true,
                "nodismiss" => item.dismisses = false,
                "dismiss" => item.dismisses = true,
                "activate" => item.active = true,
                "inactivate" => item.active = false,
                "default" => self.default_item = n,
                "fixedwidth" | "keyscroll" | "selecttext" | "deselecttext" => {}
                // A list's lines, all new.
                "new" => {
                    item.lines = lines[at..].iter().map(|s| s.to_string()).collect();
                    item.selected.clear();
                    return;
                }
                // "add n" and the line after it: a line that becomes line n
                // (at the end without a number, or past the end).
                "add" => {
                    let text = lines.get(at).copied().unwrap_or("").to_string();
                    at += 1;
                    let n = rest.trim().parse::<usize>().unwrap_or(usize::MAX);
                    let i = n.saturating_sub(1).min(item.lines.len());
                    item.lines.insert(i, text);
                    item.selected.clear();
                }
                "delete" => {
                    let (a, b) = range(rest, item.lines.len());
                    if a <= b && a < item.lines.len() {
                        item.lines.drain(a..=b.min(item.lines.len() - 1));
                    }
                    item.selected.clear();
                }
                "replace" => {
                    let text = lines.get(at).copied().unwrap_or("").to_string();
                    at += 1;
                    let (a, _) = range(rest, item.lines.len());
                    if let Some(l) = item.lines.get_mut(a) {
                        *l = text;
                    }
                }
                "swap" => {
                    let (a, b) = range(rest, item.lines.len());
                    if a < item.lines.len() && b < item.lines.len() {
                        item.lines.swap(a, b);
                    }
                }
                // A list's or pop-up's lines chosen; text, selected.
                "select" => match item.look {
                    Look::List if !rest.trim().is_empty() => {
                        let (a, b) = range(rest, item.lines.len());
                        let end = b.min(item.lines.len().saturating_sub(1));
                        if item.choose == 1 {
                            item.selected = if a < item.lines.len() { vec![a] } else { Vec::new() };
                        } else {
                            item.selected = (a..=end).filter(|i| *i < item.lines.len()).collect();
                        }
                    }
                    Look::Popup => {
                        let (a, _) = range(rest, item.lines.len());
                        if a < item.lines.len() {
                            item.selected = vec![a];
                        }
                    }
                    Look::Edit => item.focus = true,
                    _ => {}
                },
                "deselect" => {
                    if rest.trim().is_empty() {
                        item.selected.clear();
                    } else {
                        let (a, b) = range(rest, item.lines.len());
                        item.selected.retain(|i| *i < a || *i > b);
                    }
                }
                "setdblclick" => item.double_click = rest.trim().parse().unwrap_or(0),
                "selflags" => {
                    item.choose = match rest.trim().to_ascii_lowercase().as_str() {
                        "noselection" => 0,
                        "singleselection" => 1,
                        _ => 2,
                    }
                }
                _ => {
                    // A value: this line and the ones after it, up to the
                    // next setting.
                    let mut end = at;
                    while end < lines.len() && !is_setting(lines[end]) {
                        end += 1;
                    }
                    let value = lines[at - 1..end].join("\n");
                    at = end;
                    let item = &mut self.items[n - 1];
                    match item.look {
                        Look::CheckBox | Look::Radio => {
                            let v = value.trim().to_ascii_lowercase();
                            item.on = v == "1" || v == "true";
                        }
                        Look::List => item.lines = value.split('\n').map(String::from).collect(),
                        Look::Popup => {
                            if let Some(i) = item.lines.iter().position(|l| l.eq_ignore_ascii_case(value.trim())) {
                                item.selected = vec![i];
                            }
                        }
                        Look::Button if matches!(value.trim(), "0" | "1") => {}
                        _ => item.text = value,
                    }
                }
            }
        }
    }

    /// An item's value: what the dialog answers for it, and dContents.
    pub fn value(&self, n: usize, form: &str) -> String {
        let Some(item) = self.items.get(n.wrapping_sub(1)) else { return String::new() };
        let form = form.trim().to_ascii_lowercase();
        if form == "rect" {
            let r = item.rect;
            return format!("{},{},{},{}", r.left, r.top, r.right, r.bottom);
        }
        let chosen = || item.selected.iter().filter_map(|&i| item.lines.get(i).cloned()).collect::<Vec<_>>().join("\n");
        match item.look {
            Look::Icon | Look::Picture if item.radio => item.on.to_string(),
            Look::Button | Look::Icon | Look::Picture => {
                if self.hit == n {
                    if item.text.is_empty() { n.to_string() } else { item.text.clone() }
                } else {
                    String::new()
                }
            }
            Look::CheckBox | Look::Radio => item.on.to_string(),
            Look::List => match form.as_str() {
                "all" => item.lines.join("\n"),
                "long" | "alt" | "alternate" => chosen(),
                _ => item.selected.iter().map(|i| (i + 1).to_string()).collect::<Vec<_>>().join(","),
            },
            Look::Popup => match form.as_str() {
                "all" => item.lines.join("\n"),
                "short" => item.selected.first().map(|i| (i + 1).to_string()).unwrap_or_default(),
                _ => chosen(),
            },
            Look::Static | Look::Edit => item.text.clone(),
            Look::Box | Look::User => String::new(),
        }
    }

    /// What the dialog answers: each item's value, a line each.
    pub fn answer(&self) -> String {
        (1..=self.items.len()).map(|n| self.value(n, "")).collect::<Vec<_>>().join("\n")
    }

    /// Static text with ^0 to ^3 replaced.
    pub fn shown_text(&self, text: &str) -> String {
        let mut t = text.to_string();
        for (i, p) in self.params.iter().enumerate().take(4) {
            t = t.replace(&format!("^{i}"), p);
        }
        t
    }

    /// A click on item n closes the dialog.
    pub fn closes(&self, n: usize) -> bool {
        self.items.get(n.wrapping_sub(1)).is_some_and(|i| {
            i.active && i.dismisses && (i.look == Look::Button || (i.pict_button && matches!(i.look, Look::Icon | Look::Picture)))
        })
    }

    /// What a click on item n does to the dialog itself: a check box
    /// changes, a radio button is chosen (and the others in the dialog
    /// aren't).
    pub fn click(&mut self, n: usize) {
        let Some(item) = self.items.get_mut(n.wrapping_sub(1)) else { return };
        match item.look {
            Look::CheckBox => item.on = !item.on,
            Look::Radio => {
                for (i, other) in self.items.iter_mut().enumerate() {
                    if other.look == Look::Radio {
                        other.on = i + 1 == n;
                    }
                }
            }
            Look::Icon | Look::Picture if item.radio => {
                for (i, other) in self.items.iter_mut().enumerate() {
                    if other.radio {
                        other.on = i + 1 == n;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Font, size and style from "Geneva,9,b." (the style's letters: b bold,
/// i italic, u underline; "." plain).
fn font(spec: &str) -> Option<(String, u32, String)> {
    let mut parts = spec.split(',');
    let name = parts.next()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let size = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(12);
    let letters = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let mut style = Vec::new();
    for (c, s) in [('b', "bold"), ('i', "italic"), ('u', "underline"), ('o', "outline"), ('s', "shadow")] {
        if letters.contains(c) {
            style.push(s);
        }
    }
    Some((name, size, style.join(",")))
}

/// Lines a to b, from "a" or "a,b", 1-based in the script and from 0 here;
/// 0 counts as the first.
fn range(spec: &str, len: usize) -> (usize, usize) {
    let mut nums = spec.split(',').map(|s| s.trim().parse::<usize>().unwrap_or(1));
    let a = nums.next().unwrap_or(1).max(1) - 1;
    let b = nums.next().map(|b| b.max(1) - 1).unwrap_or(a);
    (a, b.min(len.max(1) - 1).max(a.min(b)))
}

/// Whether a line is a setting rather than a value.
fn is_setting(line: &str) -> bool {
    let word = line.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    matches!(
        word.as_str(),
        "styledtext"
            | "usertext"
            | "userlist"
            | "userpop"
            | "userbox"
            | "pictbutton"
            | "hide"
            | "show"
            | "nodismiss"
            | "dismiss"
            | "activate"
            | "inactivate"
            | "default"
            | "select"
            | "deselect"
            | "selecttext"
            | "deselecttext"
            | "add"
            | "delete"
            | "replace"
            | "swap"
            | "new"
            | "fixedwidth"
            | "setdblclick"
            | "selflags"
            | "keyscroll"
    )
}

/// "4◊userPop" → (4, "userPop").
pub fn split_item(arg: &str) -> Option<(usize, &str)> {
    let (n, rest) = arg.split_once(MARK)?;
    Some((n.trim().parse().ok()?, rest))
}

impl Interp {
    /// `Dialoger(name & options, items…)`: show a dialog from the stack's
    /// resources and answer its items' values, or "Error: …".
    pub(super) fn dialoger(&mut self, args: &[String]) -> R<String> {
        let head = args.first().cloned().unwrap_or_default();
        let mut head_lines = head.split('\n');
        let name = head_lines.next().unwrap_or("").trim().to_string();
        if name.is_empty() {
            return Ok("Error: dialog name is blank".into());
        }
        let template = {
            let w = self.w();
            w.stack.dialogs.values().find(|d| d.name.eq_ignore_ascii_case(&name)).cloned()
        };
        let Some(template) = template else {
            return Ok(format!("Error: unable to find DLOG \"{name}\""));
        };
        let mut d = Dialoger::new(&template);
        for line in head_lines {
            d.option(line);
        }
        for (i, arg) in args.iter().enumerate().skip(1) {
            match split_item(arg) {
                Some((n, spec)) => d.set(n, spec),
                // Second, the text for ^0 to ^3; after that, the item at
                // its place.
                None if i == 1 => d.params = arg.split('\n').map(String::from).collect(),
                None => d.set(i + 1, arg),
            }
        }
        let window = template.name.clone();
        {
            let mut w = self.w();
            w.script_windows.retain(|t| !t.name.eq_ignore_ascii_case(&window));
            let r = template.rect;
            w.script_windows.push(ScriptWindow {
                name: window.clone(),
                kind: WindowKind::Dialoger,
                text: String::new(),
                rect: Rect::new(r.left, r.top, r.right, r.bottom),
                font: String::new(),
                size: 12,
                visible: true,
                dirty: false,
                locked: false,
                palette: None,
                apple: None,
                image: None,
                texas: None,
                dialoger: Some(Box::new(d)),
            });
        }
        // The hit handler hears item 0 first, to set the dialog up.
        self.dialoger_handler(&window, 0, false)?;
        if self.dialoger_state(&window).is_some_and(|d| d.floating) {
            if self.dialoger_state(&window).is_some_and(|d| d.dismissed) {
                self.w().script_windows.retain(|t| t.name != window);
            }
            return Ok(String::new());
        }
        // Modal: wait for a click that closes it.
        loop {
            let reply = self.dialog(Dialog::Dialoger(window.clone()));
            let hit = match reply {
                Ok(DialogReply::Hit(n)) => n,
                // No one to answer, or the script was stopped: canceled.
                Ok(_) => self.dialoger_state(&window).map(|d| d.cancel_item).unwrap_or(0),
                Err(e) => {
                    self.w().script_windows.retain(|t| t.name != window);
                    return Err(e);
                }
            };
            self.dialoger_click(&window, hit);
            // The handler hears it while the dialog is still open, and may
            // close it, or keep a button from closing it.
            if hit > 0 {
                self.dialoger_handler(&window, hit, false)?;
            }
            let Some(d) = self.dialoger_state(&window) else { return Ok(String::new()) };
            if d.closes(hit) || d.dismissed || hit == 0 {
                let answer = {
                    let mut w = self.w();
                    let i = w.script_windows.iter().position(|t| t.name == window);
                    let t = i.map(|i| w.script_windows.remove(i));
                    t.and_then(|t| t.dialoger).map(|mut d| {
                        d.hit = hit;
                        d.answer()
                    })
                };
                return Ok(answer.unwrap_or_default());
            }
        }
    }

    /// A click on an item: a check box turns, a radio button is chosen.
    pub(super) fn dialoger_click(&mut self, window: &str, n: usize) {
        let mut w = self.w();
        if let Some(d) = w.script_windows.iter_mut().find(|t| t.name == window).and_then(|t| t.dialoger.as_mut()) {
            d.click(n);
        }
    }

    /// A click on a floating dialog's item: as in a modal one, and a
    /// button that closes it closes it. Its close box is item 0.
    pub(super) fn dialoger_float_hit(&mut self, window: &str, n: usize) -> R<()> {
        self.dialoger_click(window, n);
        if n > 0 {
            self.dialoger_handler(window, n, true)?;
        }
        let Some(d) = self.dialoger_state(window) else { return Ok(()) };
        if n == 0 || d.closes(n) || d.dismissed {
            self.w().script_windows.retain(|t| t.name != window);
        }
        Ok(())
    }

    /// Send the dialog's hit handler, if it has one, the dialog, the item
    /// and the part: "on ToDTSHits dialog, itemHit, partHit".
    fn dialoger_handler(&mut self, window: &str, n: usize, top: bool) -> R<()> {
        let Some(handler) = self.dialoger_state(window).map(|d| d.handler).filter(|h| !h.is_empty()) else { return Ok(()) };
        let card = self.current_card_ref();
        let args = vec![window.to_string(), n.to_string(), String::new()];
        if top { self.dispatch_top(card, &handler, args) } else { self.send_message(card, &handler, &args).map(|_| ()) }
    }

    /// `dChanger dialog, "N◊…", …`: change an open dialog. "0◊dismiss"
    /// closes it. A problem goes in the result.
    pub(super) fn dchanger(&mut self, args: &[String]) -> R<()> {
        let name = args.first().map(|a| a.trim().to_string()).unwrap_or_default();
        let floating = {
            let mut w = self.w();
            let Some(d) = w.script_windows.iter_mut().find(|t| t.name.eq_ignore_ascii_case(&name)).and_then(|t| t.dialoger.as_mut()) else {
                drop(w);
                self.result = format!("Error: no such dialog exists: {name}");
                return Ok(());
            };
            for arg in args.iter().skip(1) {
                if let Some((n, spec)) = split_item(arg) {
                    d.set(n, spec);
                }
            }
            d.dismissed.then_some(d.floating)
        };
        self.result.clear();
        // A floating dialog closes now; a modal one when its click is done.
        if floating == Some(true) {
            self.w().script_windows.retain(|t| !t.name.eq_ignore_ascii_case(&name));
        }
        Ok(())
    }

    /// An open dialog, by its name.
    pub(super) fn dialoger_state(&self, name: &str) -> Option<Dialoger> {
        let w = self.w();
        w.script_windows.iter().find(|t| t.name.eq_ignore_ascii_case(name.trim())).and_then(|t| t.dialoger.as_deref().cloned())
    }

    /// `dContents(dialog[, item[, form]])`.
    pub(super) fn dcontents(&mut self, args: &[String]) -> R<String> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        let Some(d) = self.dialoger_state(&arg(0)) else {
            // With only a name, whether it's open: 0 when it isn't.
            return Ok(if args.len() <= 1 { "0".into() } else { format!("Error: no such dialog exists: {}", arg(0).trim()) });
        };
        if args.len() <= 1 {
            return Ok("1".into());
        }
        let Ok(n) = arg(1).trim().parse::<usize>() else {
            return Ok(format!("Error: item number is invalid: {}", arg(1).trim()));
        };
        if n == 0 {
            let w = self.w();
            let r = w.script_windows.iter().find(|t| t.name.eq_ignore_ascii_case(arg(0).trim())).map(|t| t.rect).unwrap_or_default();
            return Ok(match arg(2).trim().to_ascii_lowercase().as_str() {
                "loc" | "topleft" => format!("{},{}", r.left, r.top),
                "rect" => format!("{},{},{},{}", r.left, r.top, r.right, r.bottom),
                "width" => r.width().to_string(),
                "height" => r.height().to_string(),
                "title" => d.title.clone(),
                "kind" => if d.floating { "Float" } else { "Modal" }.into(),
                _ => d.answer(),
            });
        }
        Ok(d.value(n, &arg(2)))
    }
}
