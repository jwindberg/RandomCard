//! Texas, Leonard Buck's 1990 external, from Apple's Developer Info
//! Assistant: windows of messages beside the card window. A discussion
//! window ("Disc") shows a thread of messages one at a time, with arrows
//! and a pop-up to move through them and a Reply button; a message window
//! ("Send", whose style reads back as "New") shows a message ready to go,
//! with Send and an enclosure; a feedback window ("Feed") is text to write
//! and send.
//!
//! `Texas name, text, style` opens one. `WindIsTexas(name)` and
//! `FrontIfTexas()` tell scripts which windows are Texas's. The window's
//! buttons send the stack texasReplyMsg name, texasSendMsg(name, from),
//! texasEnclosePkg name and texasUnstuffPkg name; closing one sends
//! texasClose name, from, while it can still be read.
//!
//! A message is a header of 13 lines ("Source:", "Thread ID:", "Thread
//! Title:", "Message Title:" ... "Included File:"), a blank line, and its
//! text. In a discussion, each header line ends with a delimiter, " Ω≈∑"
//! (Mac Roman BD C5 B7), which is how one message is told from the next.

use super::interp::{Interp, R, err};
use super::world::{Dialog, DialogReply, ScriptWindow, WindowKind};
use crate::model::Rect;

/// How many lines a message's header has.
const HEADER_LINES: usize = 13;

/// The window's style: what it's for, and which buttons it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexasStyle {
    /// A discussion: a thread of messages, read one at a time.
    Disc,
    /// A message ready to send ("Send" makes one).
    New,
    /// Feedback, written in the window.
    Feed,
}

impl TexasStyle {
    pub fn named(s: &str) -> Option<TexasStyle> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "disc" | "discussion" => TexasStyle::Disc,
            "send" | "new" => TexasStyle::New,
            "feed" | "feedback" => TexasStyle::Feed,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            TexasStyle::Disc => "Disc",
            TexasStyle::New => "New",
            TexasStyle::Feed => "Feed",
        }
    }
}

/// What a Texas window keeps besides its text (the whole of it).
#[derive(Clone, Debug)]
pub struct Texas {
    pub style: TexasStyle,
    /// The message shown, from 0.
    pub shown: usize,
    /// A message window's enclosure, set by the stack: "TRUE && file", or
    /// empty for none.
    pub enclosure: String,
    /// The stack has been sent texasClose: closing it again doesn't send
    /// it twice.
    pub closing: bool,
}

/// What the user did in a Texas window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexasAction {
    /// The close box: a message not sent yet asks first.
    Close,
    Reply,
    Send,
    /// The enclosure: add one to a message, or unpack a discussion's.
    Enclosure,
}

/// The delimiter at the end of each header line in a discussion.
pub fn delimiter() -> String {
    let high = |b: u8| crate::import::MAC_ROMAN_HIGH[(b - 0x80) as usize];
    format!(" {}{}{}", high(0xBD), high(0xC5), high(0xB7))
}

/// One message of a window's text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    /// The header's lines, without their delimiters; empty if it has none.
    pub header: Vec<String>,
    /// The text after the header and the blank line under it.
    pub body: String,
}

impl Message {
    /// A header line's value: `field("Message Title")`.
    pub fn field(&self, name: &str) -> Option<String> {
        self.header.iter().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case(name).then(|| v.trim().to_string())
        })
    }

    pub fn title(&self) -> Option<String> {
        self.field("Message Title").filter(|t| !t.is_empty())
    }
}

fn is_header_start(lines: &[&str], i: usize) -> bool {
    lines[i].starts_with("Source:") && lines.get(i + 1).is_some_and(|l| l.starts_with("Thread ID:"))
}

/// A window's text as messages. Text with no header is one message, all
/// text.
pub fn messages(whole: &str) -> Vec<Message> {
    let delim = delimiter();
    let lines: Vec<&str> = whole.split('\n').collect();
    let starts: Vec<usize> = (0..lines.len()).filter(|&i| is_header_start(&lines, i)).collect();
    if starts.is_empty() {
        return vec![Message { header: Vec::new(), body: whole.to_string() }];
    }
    let mut out = Vec::new();
    // Text before the first header belongs to no message.
    for (n, &start) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(lines.len());
        let mut i = start;
        let mut header = Vec::new();
        while i < end && header.len() < HEADER_LINES && !lines[i].is_empty() {
            header.push(lines[i].strip_suffix(delim.as_str()).unwrap_or(lines[i]).to_string());
            i += 1;
        }
        // The blank line under the header.
        if i < end && lines[i].is_empty() {
            i += 1;
        }
        let mut body = lines[i..end].join("\n");
        // The return that ends a message before the next one's header.
        if n + 1 < starts.len() && body.ends_with('\n') {
            body.pop();
        }
        out.push(Message { header, body });
    }
    out
}

impl ScriptWindow {
    /// The message shown in a Texas window.
    pub fn texas_message(&self) -> Option<Message> {
        let t = self.texas.as_ref()?;
        let all = messages(&self.text);
        let n = t.shown.min(all.len().saturating_sub(1));
        all.into_iter().nth(n)
    }

    /// A Texas window's own property, or None.
    pub fn texas_property(&self, name: &str) -> Option<String> {
        let t = self.texas.as_ref()?;
        let shown = self.texas_message().unwrap_or_default();
        Some(match name {
            "style" => t.style.name().into(),
            "whole" => self.text.clone(),
            "text" | "contents" => shown.body,
            "messagename" => shown.title().unwrap_or_else(|| self.name.clone()),
            "title" => self.name.clone(),
            "messagelist" => messages(&self.text)
                .iter()
                .enumerate()
                .map(|(i, m)| m.title().unwrap_or_else(|| format!("Message {}", i + 1)))
                .collect::<Vec<_>>()
                .join("\n"),
            "enclosure" => {
                let e = match t.style {
                    TexasStyle::Disc => shown.field("Included File").unwrap_or_default(),
                    _ => t.enclosure.clone(),
                };
                if e.is_empty() { "FALSE".into() } else { e }
            }
            "owner" => "Texas".into(),
            "properties" => "style,whole,text,messageName,messageList,enclosure,newMessage,title,textFont,textSize".into(),
            _ => return None,
        })
    }

    /// Set a Texas window's own property; false if it isn't one.
    pub fn set_texas_property(&mut self, name: &str, v: &str) -> bool {
        let Some(t) = self.texas.as_mut() else { return false };
        match name {
            "whole" => {
                self.text = v.to_string();
                t.shown = 0;
            }
            // The shown message's text, under its header.
            "text" | "contents" => {
                let mut all = messages(&self.text);
                let n = t.shown.min(all.len().saturating_sub(1));
                all[n].body = v.to_string();
                self.text = join(&all);
            }
            // Another message in a discussion, shown.
            "newmessage" => {
                if !self.text.is_empty() && !self.text.ends_with('\n') {
                    self.text.push('\n');
                }
                self.text.push_str(v);
                t.shown = messages(&self.text).len() - 1;
            }
            "enclosure" => {
                t.enclosure = if v.trim().eq_ignore_ascii_case("false") { String::new() } else { v.to_string() };
            }
            "title" => self.name = v.to_string(),
            _ => return false,
        }
        self.dirty = false;
        true
    }
}

/// Messages back into a window's text, each header line delimited as in a
/// discussion.
fn join(all: &[Message]) -> String {
    let delim = delimiter();
    all.iter()
        .map(|m| {
            if m.header.is_empty() {
                m.body.clone()
            } else {
                let header: Vec<String> = m.header.iter().map(|l| format!("{l}{delim}")).collect();
                format!("{}\n\n{}", header.join("\n"), m.body)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A name in quotes for a HyperTalk expression.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\" & quote & \""))
}

impl Interp {
    /// `Texas name, text, style`.
    pub(super) fn texas_command(&mut self, args: &[String]) -> R<()> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        let name = arg(0);
        let Some(style) = TexasStyle::named(&arg(2)) else {
            return err(format!("Texas has no window style \"{}\"", arg(2).trim()));
        };
        if name.trim().is_empty() {
            return err("Texas needs a window name");
        }
        let mut w = self.w();
        w.script_windows.retain(|t| !t.name.eq_ignore_ascii_case(&name));
        // Each new one a little down and to the right of the last.
        let n = w.script_windows.iter().filter(|t| t.texas.is_some()).count() as i32;
        let (x, y) = (30 + 20 * (n % 8), 30 + 20 * (n % 8));
        w.script_windows.push(ScriptWindow {
            name,
            kind: WindowKind::Texas,
            text: arg(1),
            rect: Rect::new(x, y, x + 440, y + 320),
            font: "Monaco".into(),
            size: 9,
            visible: true,
            dirty: false,
            locked: style != TexasStyle::Feed,
            palette: None,
            apple: None,
            image: None,
            texas: Some(Box::new(Texas { style, shown: 0, enclosure: String::new(), closing: false })),
            dialoger: None,
        });
        Ok(())
    }

    /// WindIsTexas and FrontIfTexas.
    pub(super) fn texas_function(&mut self, name: &str, args: &[String]) -> Option<R<String>> {
        let w = self.w();
        Some(Ok(match name {
            "windistexas" => {
                let wanted = args.first().map(|a| a.trim().to_string()).unwrap_or_default();
                w.script_windows.iter().any(|t| t.texas.is_some() && t.name.eq_ignore_ascii_case(&wanted)).to_string()
            }
            // The front window's name, if it's a Texas window.
            "frontiftexas" => w
                .script_windows
                .iter()
                .rev()
                .find(|t| t.visible)
                .filter(|t| t.texas.is_some())
                .map(|t| t.name.clone())
                .unwrap_or_default(),
            _ => return None,
        }))
    }

    /// A button or the close box of a Texas window.
    pub(super) fn texas_action(&mut self, name: &str, action: TexasAction) -> R<()> {
        let Some((style, has_enclosure)) = self.w().script_windows.iter().find(|t| t.name == name).and_then(|t| {
            let enclosure = t.texas_property("enclosure").is_some_and(|e| e != "FALSE");
            t.texas.as_ref().map(|x| (x.style, enclosure))
        }) else {
            return Ok(());
        };
        let card = self.current_card_ref();
        match action {
            TexasAction::Reply if style == TexasStyle::Disc => self.dispatch_top(card, "texasReplyMsg", vec![name.into()]),
            TexasAction::Send if style != TexasStyle::Disc => {
                self.send_text_top(&format!("get texasSendMsg({}, \"SendBtn\")", quoted(name)), card)
            }
            TexasAction::Enclosure => match style {
                TexasStyle::Disc if has_enclosure => self.dispatch_top(card, "texasUnstuffPkg", vec![name.into()]),
                TexasStyle::New => self.dispatch_top(card, "texasEnclosePkg", vec![name.into()]),
                _ => Ok(()),
            },
            TexasAction::Close => {
                if style == TexasStyle::Disc {
                    return self.close_texas(name, "noWhere", true);
                }
                let reply = self.dialog(Dialog::Answer {
                    prompt: "Send this message before closing?".into(),
                    buttons: vec!["Cancel".into(), "Don't Send".into(), "Send".into()],
                })?;
                match reply {
                    DialogReply::Button(b) if b == "Send" => {
                        self.send_text_top(&format!("get texasSendMsg({}, \"CloseBox\")", quoted(name)), card)?;
                        // Sent: it closes (the stack may have closed it).
                        if self.get_var("it").unwrap_or_default().trim().eq_ignore_ascii_case("true") {
                            self.close_texas(name, "yesSend", true)?;
                        }
                        Ok(())
                    }
                    DialogReply::Button(b) if b == "Don't Send" => self.close_texas(name, "noSend", true),
                    _ => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }

    /// Tell the stack texasClose name, from, while the window can still be
    /// read, then close it. Once only, however it was closed.
    pub(super) fn close_texas(&mut self, name: &str, from: &str, by_user: bool) -> R<()> {
        let first = {
            let mut w = self.w();
            match w.script_windows.iter_mut().find(|t| t.name.eq_ignore_ascii_case(name)).and_then(|t| t.texas.as_mut()) {
                Some(t) if !t.closing => {
                    t.closing = true;
                    true
                }
                Some(_) => false,
                None => return Ok(()),
            }
        };
        let r = if first {
            let card = self.current_card_ref();
            let args = vec![name.to_string(), from.to_string()];
            if by_user { self.dispatch_top(card, "texasClose", args) } else { self.send_message(card, "texasClose", &args).map(|_| ()) }
        } else {
            Ok(())
        };
        self.w().script_windows.retain(|t| !t.name.eq_ignore_ascii_case(name));
        r
    }

    /// Move through a discussion's messages.
    pub(super) fn texas_show(&mut self, name: &str, n: usize) -> R<()> {
        let mut w = self.w();
        if let Some(win) = w.script_windows.iter_mut().find(|t| t.name == name) {
            let count = messages(&win.text).len();
            if let Some(t) = win.texas.as_mut() {
                t.shown = n.min(count.saturating_sub(1));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn discussion(titles: &[&str]) -> String {
    let d = delimiter();
    titles
        .iter()
        .map(|t| {
            let header = [
                "Source: Test 4.1".to_string(),
                "Thread ID: 1".into(),
                "Thread Title: Thread".into(),
                format!("Message Title: {t}"),
                "Subject: Testing".into(),
                "Message Time: 0".into(),
                "Project: P".into(),
                "Seed: ".into(),
                "Priority: ".into(),
                "Send To: ".into(),
                "Sent From: Me".into(),
                "CC: ".into(),
                "Included File: FALSE".into(),
            ];
            format!("{}\n\nThe text of {t}.", header.iter().map(|l| format!("{l}{d}")).collect::<Vec<_>>().join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
