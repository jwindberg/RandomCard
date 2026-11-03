//! More of Frédéric Rinaldi's externals, the ones that aren't about files
//! (those are in files.rs): LineCount, FullSort, DateSort, GetPassword,
//! Prompt, GetMode, MacType, Switch, CapsLock, ClipInfo, ATalkZones,
//! GlobalList and WindName. Apple carried his collection on its Developer CDs, a
//! demo stack for each. RandomCard does them itself, by the same names and
//! with the same answers. As his did, each answers "!" with its notice and
//! "?" with its syntax, and reports a problem as text starting "Error :".

use super::interp::{Interp, R, to_int};
use super::world::{Dialog, DialogReply, ObjRef, ScriptWindow, WindowKind};
use crate::model::{PartKind, Rect};

/// Each one's usual name and the syntax it answers to "?".
const SYNTAX: &[(&str, &str, &str)] = &[
    ("linecount", "LineCount", "LineCount(\"<field descriptor>\")"),
    (
        "fullsort",
        "FullSort",
        "FullSort(<text> [,\"s=<sep>\"] [,\"t=a|n|d|t|i|u\"] [,\"d=a|d|r|l\"] [,\"c=char|word|item x [to y|*]\"] [,<sort list>])",
    ),
    ("datesort", "DateSort", "DateSort(<container>)"),
    ("getpassword", "GetPassword", "GetPassword([<prompt>])"),
    ("prompt", "Prompt", "Prompt 1 [,<message> [,<vertical position> [,<width>]]] | Prompt 2,<message> | Prompt 3"),
    ("getmode", "GetMode", "GetMode()"),
    ("mactype", "MacType", "MacType()"),
    ("switch", "Switch", "Switch(<container>)"),
    ("capslock", "CapsLock", "CapsLock()"),
    ("clipinfo", "ClipInfo", "ClipInfo()"),
    ("atalkzones", "ATalkZones", "ATalkZones()"),
    ("globallist", "GlobalList", "GlobalList([<ListAll>])"),
    ("windname", "WindName", "WindName <title string>"),
];

/// The answer to "!" or "?", if that's what was asked.
fn help(name: &str, args: &[String]) -> Option<String> {
    let (_, proper, syntax) = SYNTAX.iter().find(|(n, _, _)| *n == name)?;
    match args.first().map(|a| a.trim()) {
        Some("!") => Some(format!("{proper} 2.0 — built into RandomCard, after Frédéric Rinaldi's external")),
        Some("?") => Some(syntax.to_string()),
        Some("=") if name == "switch" => Some("The text with its lines made items, or its items made lines".into()),
        _ => None,
    }
}

/// The window Prompt shows its message in.
const PROMPT_WINDOW: &str = "Prompt";

impl Interp {
    /// One of these functions' answer, or None if it isn't one.
    pub(super) fn rinaldi_function(&mut self, name: &str, args: &[String]) -> Option<R<String>> {
        if !matches!(
            name,
            "linecount"
                | "fullsort"
                | "datesort"
                | "getpassword"
                | "getmode"
                | "mactype"
                | "switch"
                | "capslock"
                | "clipinfo"
                | "atalkzones"
                | "globallist"
        ) {
            return None;
        }
        if let Some(h) = help(name, args) {
            return Some(Ok(h));
        }
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        Some(Ok(match name {
            "linecount" => self.line_count(&arg(0)),
            "fullsort" => {
                let list = self.globals.get("fullsortlist").cloned().unwrap_or_default();
                match full_sort(args, &list) {
                    Ok((sorted, order)) => {
                        self.globals.insert("fullsortlist".into(), order);
                        sorted
                    }
                    Err(e) => e,
                }
            }
            "datesort" => date_sort(&arg(0)),
            // The number HyperCard's `ask password` gives for what was typed,
            // or empty if canceled.
            "getpassword" => {
                let prompt = if arg(0).trim().is_empty() { "Please enter password :".to_string() } else { arg(0) };
                match self.dialog(Dialog::Ask { prompt, default: String::new(), password: true }) {
                    Ok(DialogReply::Text(Some(t)) | DialogReply::Button(t)) if !t.is_empty() => {
                        crate::model::password::first_hash(&t).to_string()
                    }
                    Ok(_) => String::new(),
                    Err(e) => return Some(Err(e)),
                }
            }
            // A line for the screen: color or black and white, depth, the
            // screen's number, the most it can show, and its size.
            "getmode" => {
                let r = self.w().screen_rect;
                format!("C,32,1,C,32,{},{}", r.right - r.left, r.bottom - r.top)
            }
            // The computer, the system's version, the processor, whether
            // there's a floating-point unit and color, and the keyboard. His
            // list of Macs ended in 1990, so the computer is the system's
            // name, as SysEnv gives it.
            "mactype" => match self.external_function("sysenv", &[])? {
                Ok(env) => {
                    let first = env.lines().next().unwrap_or_default().to_string();
                    let mut items: Vec<&str> = first.split(',').collect();
                    items.resize(5, "");
                    format!("{},{},{},true,true,Unknown keyboard", items[0], items[1], items[2])
                }
                Err(e) => return Some(Err(e)),
            },
            "switch" => switch(&arg(0), &self.item_delimiter),
            "capslock" => self.w().mouse.caps_lock.to_string(),
            "clipinfo" => self.w().clip_types.clone(),
            // RandomCard has no AppleTalk, so no node address to ask about.
            "atalkzones" => "Error : Couldn't get node address".into(),
            "globallist" => match arg(0).trim().to_ascii_lowercase().as_str() {
                "" | "true" | "false" => {
                    let mut names: Vec<&String> = self.globals.keys().collect();
                    names.sort();
                    names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(",")
                }
                _ => "Error : Parameter must be true or false".into(),
            },
            _ => unreachable!(),
        }))
    }

    /// One of these commands, or None if it isn't one. Problems go in the
    /// result.
    pub(super) fn rinaldi_command(&mut self, name: &str, args: &[String]) -> Option<R<()>> {
        if !matches!(name, "prompt" | "windname") {
            return None;
        }
        if let Some(h) = help(name, args) {
            self.result = h;
            return Some(Ok(()));
        }
        if name == "windname" {
            // The window's title, until another stack opens.
            let mut w = self.w();
            let serial = w.stack_serial;
            w.window_title = Some((serial, args.join(",")));
            return Some(Ok(()));
        }
        self.result = self.prompt(args).err().unwrap_or_default();
        Some(Ok(()))
    }

    /// LineCount: a field's lines as it shows them, wrapped lines and all.
    fn line_count(&mut self, desc: &str) -> String {
        let Ok(ObjRef::Part { layer, id, card }) = self.parse_object_text(desc) else {
            return "Error : Bad field param".into();
        };
        let w = self.w();
        match w.stack.part(card, layer, id) {
            Some(p) if p.kind == PartKind::Field => crate::print::field_lines(&w.stack, card, layer, p).len().to_string(),
            _ => "Error : Bad field param".into(),
        }
    }

    /// Prompt: a message in a small window while a script works. 1 shows it
    /// (centered on the card, or with its top at a given height), 2 changes
    /// the message, 3 takes it away.
    fn prompt(&mut self, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(|a| a.trim().to_string()).unwrap_or_default();
        let mut w = self.w();
        let at = w.script_windows.iter().position(|t| t.name == PROMPT_WINDOW);
        match arg(0).as_str() {
            "1" => {
                let message = args.get(1).cloned().unwrap_or_default();
                let (cw, ch) = (w.stack.width as i32, w.stack.height as i32);
                let text = crate::model::metrics::metrics("Geneva", 12, "plain").text_width(&message) as i32;
                let width = match arg(3) {
                    a if a.is_empty() => if message.is_empty() { 300 } else { text + 40 },
                    a => to_int(&a).map_err(|_| "Error : Bad Coordinate".to_string())? as i32,
                };
                let height = 40;
                let top = match arg(2) {
                    a if a.is_empty() => (ch - height) / 2,
                    a => to_int(&a).map_err(|_| "Error : Bad Coordinate".to_string())? as i32,
                };
                let left = (cw - width) / 2;
                let window = ScriptWindow {
                    name: PROMPT_WINDOW.into(),
                    kind: WindowKind::Text,
                    text: message,
                    rect: Rect::new(left, top, left + width, top + height),
                    font: "Geneva".into(),
                    size: 12,
                    visible: true,
                    dirty: false,
                    locked: true,
                    palette: None,
                };
                match at {
                    Some(i) => w.script_windows[i] = window,
                    None => w.script_windows.push(window),
                }
                Ok(())
            }
            "2" => {
                let Some(i) = at else { return Err("Error : Pointer Not Declared".into()) };
                if args.len() < 2 {
                    return Err("Error : Missing Parameter(s)".into());
                }
                w.script_windows[i].text = args[1].clone();
                Ok(())
            }
            "3" => {
                let Some(i) = at else { return Err("Error : Pointer Not Declared".into()) };
                w.script_windows.remove(i);
                Ok(())
            }
            "" => Err("Error : Missing Parameter(s)".into()),
            _ => Err("Error : Bad Mode Parameter".into()),
        }
    }
}

/// Switch: a list of lines made items, or of items made lines. Text with
/// more than one line is lines.
fn switch(text: &str, delimiter: &str) -> String {
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.contains('\n') {
        text.lines().collect::<Vec<_>>().join(delimiter)
    } else {
        text.split(delimiter).collect::<Vec<_>>().join("\n")
    }
}

/// A letter folded for the international order: no case, no accents.
fn fold(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ÿ' | 'Ÿ' => 'y',
        c => c.to_ascii_lowercase(),
    }
}

/// The text in the international order: by letters folded, then as is, so
/// "e" comes before "é" and "E".
fn intl_key(s: &str) -> (String, String) {
    (s.chars().map(fold).collect(), s.to_string())
}

/// The first number in some text.
fn first_number(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let start = (0..b.len()).find(|&i| b[i].is_ascii_digit() || (b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)))?;
    let neg = start > 0 && b[start - 1] == b'-';
    let mut end = start;
    let mut dot = false;
    while end < b.len() && (b[end].is_ascii_digit() || (b[end] == b'.' && !dot)) {
        dot |= b[end] == b'.';
        end += 1;
    }
    let n: f64 = s[start..end].trim_end_matches('.').parse().ok()?;
    Some(if neg { -n } else { n })
}

/// Runs of digits in some text, with where each one ends.
fn digit_runs(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            v.push((start, i));
        } else {
            i += 1;
        }
    }
    v
}

/// The first short date in some text (m/d/yy or m/d/yyyy), as year, month
/// and day; a two-digit year is in the 1900s, as on the Macs of the time.
fn first_date(s: &str) -> Option<(i32, i32, i32)> {
    let runs = digit_runs(s);
    runs.windows(3).find_map(|w| {
        let [(a0, a1), (b0, b1), (c0, c1)] = [w[0], w[1], w[2]];
        if &s[a1..b0] != "/" || &s[b1..c0] != "/" {
            return None;
        }
        let (m, d, y): (i32, i32, i32) = (s[a0..a1].parse().ok()?, s[b0..b1].parse().ok()?, s[c0..c1].parse().ok()?);
        let y = if c1 - c0 <= 2 { 1900 + y } else { y };
        ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
    })
}

/// The first time in some text (h:mm), in minutes.
fn first_time(s: &str) -> Option<i32> {
    let runs = digit_runs(s);
    runs.windows(2).find_map(|w| {
        let [(a0, a1), (b0, b1)] = [w[0], w[1]];
        if &s[a1..b0] != ":" || b1 - b0 != 2 {
            return None;
        }
        let (h, m): (i32, i32) = (s[a0..a1].parse().ok()?, s[b0..b1].parse().ok()?);
        let pm = s[b1..].trim_start().to_ascii_lowercase().starts_with("pm");
        Some((h % 12 + if pm { 12 } else { 0 }) * 60 + m)
    })
}

/// A sort key: missing ones sort after the rest.
#[derive(PartialEq, PartialOrd)]
enum Key {
    Text(String, String),
    Number(f64),
    Missing,
}

/// FullSort: lines or items sorted, with the order they went in, for
/// FullSortList. Problems come back as the "Error :" text.
fn full_sort(args: &[String], last_order: &str) -> Result<(String, String), String> {
    let text = args.first().cloned().unwrap_or_default();
    let (mut sep, mut kind, mut dir, mut chunk, mut list) = (None, 'a', 'a', None::<String>, None::<String>);
    for a in args.iter().skip(1) {
        let t = a.trim();
        let lower = t.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("s=") {
            sep = Some(match v {
                "space" => " ".to_string(),
                "return" => "\n".to_string(),
                "tab" => "\t".to_string(),
                "quote" => "\"".to_string(),
                _ => t[2..].to_string(),
            });
        } else if let Some(v) = lower.strip_prefix("t=") {
            kind = v.chars().next().unwrap_or('a');
        } else if let Some(v) = lower.strip_prefix("d=") {
            dir = v.chars().next().unwrap_or('a');
        } else if let Some(v) = lower.strip_prefix("c=") {
            chunk = Some(v.to_string());
        } else if !t.is_empty() {
            list = Some(t.to_string());
        }
    }
    let sep = sep.unwrap_or_else(|| if text.contains('\n') { "\n".into() } else { ",".into() });
    let mut items: Vec<&str> = text.split(sep.as_str()).collect();
    if items.len() > 1 && items.last() == Some(&"") {
        items.pop();
    }
    let chunk = match chunk {
        Some(c) => Some(parse_chunk(&c, &sep)?),
        None => None,
    };
    let mut order: Vec<usize> = (0..items.len()).collect();
    match dir {
        'l' => {
            let given = list.unwrap_or_else(|| last_order.to_string());
            let mut wanted = Vec::new();
            for n in given.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                match n.parse::<usize>() {
                    Ok(i) if (1..=items.len()).contains(&i) => wanted.push(i - 1),
                    _ => return Err(format!("Error : Bad item {n} in Sort list")),
                }
            }
            // Those the list doesn't name keep their places after it.
            let rest: Vec<usize> = order.iter().copied().filter(|i| !wanted.contains(i)).collect();
            order = wanted.into_iter().chain(rest).collect();
        }
        'r' => {
            for i in (1..order.len()).rev() {
                order.swap(i, rand::random_range(0..=i));
            }
        }
        _ => {
            let key = |s: &str| -> Key {
                let s = match &chunk {
                    Some(c) => c.of(s),
                    None => s.to_string(),
                };
                match kind {
                    'n' => first_number(&s).map_or(Key::Missing, Key::Number),
                    'd' => first_date(&s).map_or(Key::Missing, |(y, m, d)| Key::Number((y * 10000 + m * 100 + d) as f64)),
                    't' => first_time(&s).map_or(Key::Missing, |t| Key::Number(t as f64)),
                    'i' => {
                        let (a, b) = intl_key(&s);
                        Key::Text(a, b)
                    }
                    'u' => Key::Text(s.to_lowercase(), String::new()),
                    _ => Key::Text(s, String::new()),
                }
            };
            let keys: Vec<Key> = items.iter().map(|s| key(s)).collect();
            let missing = |k: &Key| *k == Key::Missing;
            order.sort_by(|&a, &b| {
                let (ka, kb) = (&keys[a], &keys[b]);
                // Missing keys go last, whichever way the sort goes.
                match (missing(ka), missing(kb)) {
                    (true, true) => std::cmp::Ordering::Equal,
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => {
                        let o = ka.partial_cmp(kb).unwrap_or(std::cmp::Ordering::Equal);
                        if dir == 'd' { o.reverse() } else { o }
                    }
                }
            });
        }
    }
    let sorted: Vec<&str> = order.iter().map(|&i| items[i]).collect();
    let order: Vec<String> = order.iter().map(|i| (i + 1).to_string()).collect();
    Ok((sorted.join(&sep), order.join(",")))
}

/// The part of an item a sort looks at: "char 2 to 4", "word 3 to *".
struct Chunk {
    kind: char,
    from: usize,
    to: Option<usize>,
}

impl Chunk {
    fn of(&self, s: &str) -> String {
        let pieces: Vec<String> = match self.kind {
            'c' => s.chars().map(String::from).collect(),
            'w' => s.split_whitespace().map(String::from).collect(),
            _ => s.split(',').map(String::from).collect(),
        };
        let from = self.from.saturating_sub(1);
        let to = self.to.unwrap_or(pieces.len()).min(pieces.len());
        let joiner = match self.kind {
            'c' => "",
            'w' => " ",
            _ => ",",
        };
        if from >= to { String::new() } else { pieces[from..to].join(joiner) }
    }
}

fn parse_chunk(c: &str, sep: &str) -> Result<Chunk, String> {
    let c = c.trim();
    let (kind, rest) = if let Some(r) = c.strip_prefix("char") {
        ('c', r)
    } else if let Some(r) = c.strip_prefix("word") {
        ('w', r)
    } else if let Some(r) = c.strip_prefix("item") {
        if sep == "," {
            return Err("Error : \"c=item\" cannot be used with item sorting".into());
        }
        ('i', r)
    } else {
        return Err("Error : \"c=\" param must be followed by [char,word,item]".into());
    };
    let name = &c[..4];
    if !rest.starts_with(' ') {
        return Err(format!("Error : \"c={name}\" must be followed by a space"));
    }
    let number = |s: &str| s.trim().parse::<usize>().ok().filter(|&n| n > 0);
    let bad = || format!("Error : \"c={name} \" must be followed by a positive integer");
    let (from, to) = match rest.split_once(" to ") {
        Some((a, b)) => (number(a).ok_or_else(bad)?, if b.trim() == "*" { None } else { Some(number(b).ok_or_else(bad)?) }),
        None => {
            let n = number(rest).ok_or_else(bad)?;
            (n, Some(n))
        }
    };
    Ok(Chunk { kind, from, to })
}

/// DateSort: lines in date order, by the first short date in each. Blank
/// lines go, and lines with no date come last, in the international order.
fn date_sort(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut dated: Vec<(usize, (i32, i32, i32))> = Vec::new();
    let mut undated: Vec<usize> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        match first_date(l) {
            Some(d) => dated.push((i, d)),
            None => undated.push(i),
        }
    }
    dated.sort_by_key(|&(_, d)| d);
    undated.sort_by_key(|&i| intl_key(lines[i]));
    dated.iter().map(|&(i, _)| i).chain(undated).map(|i| lines[i]).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sort(args: &[&str]) -> (String, String) {
        full_sort(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>(), "").unwrap()
    }

    #[test]
    fn full_sort_kinds() {
        assert_eq!(sort(&["pear\nApple\nfig"]), ("Apple\nfig\npear".into(), "2,3,1".into()));
        assert_eq!(sort(&["pear,Apple,fig", "t=upper"]).0, "Apple,fig,pear");
        assert_eq!(sort(&["b,a,c", "d=descending"]).0, "c,b,a");
        assert_eq!(sort(&["item 10\nitem 9\nnone\nitem -2", "t=n"]).0, "item -2\nitem 9\nitem 10\nnone");
        assert_eq!(sort(&["x 3/1/91\ny 12/25/90\nz 1/1/1989", "t=date"]).0, "z 1/1/1989\ny 12/25/90\nx 3/1/91");
        assert_eq!(sort(&["at 10:30\nat 9:05", "t=time"]).0, "at 9:05\nat 10:30");
        assert_eq!(sort(&["élan\nzèbre\neau", "t=intl"]).0, "eau\nélan\nzèbre");
        assert_eq!(sort(&["b a\nc z\na m", "c=word 2"]).0, "b a\na m\nc z");
        assert_eq!(sort(&["3;1;2", "s=;"]).0, "1;2;3");
        // Co-sorting another list in the same order.
        let (_, order) = sort(&["c\na\nb"]);
        assert_eq!(full_sort(&["three\none\ntwo".into(), "d=l".into()], &order).unwrap().0, "one\ntwo\nthree");
        assert!(full_sort(&["a,b".into(), "c=item 1".into()], "").unwrap_err().starts_with("Error"));
        assert!(full_sort(&["a\nb".into(), "c=line 1".into()], "").unwrap_err().starts_with("Error"));
    }

    #[test]
    fn date_sort_lines() {
        assert_eq!(date_sort("toto 1/8/85\n\nzz\ntiti 3/6/88\nmeet 12/1/1984\naa"), "meet 12/1/1984\ntoto 1/8/85\ntiti 3/6/88\naa\nzz");
    }
}
