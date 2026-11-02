//! Files and folders, for scripts: copying, deleting, moving and renaming
//! files, listing folders and reading their details. These are the file
//! externals of Frédéric Rinaldi's collection (FileCopy, EraseFile, FullMove,
//! FullRename, GetDir, GetFInfo, FullSFPut, GetBootVol, GetSysFolder), which
//! Apple carried on its Developer CDs, done by RandomCard itself and called
//! by the same names, with the same answers.
//!
//! As his did, each answers "!" with its notice and "?" with its syntax, and
//! reports a problem as text starting "Error :": in the result for a
//! command, as the answer for a function. A plain file name means a file in
//! the current stack's folder.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::externals::FileDialog;
use super::interp::{Interp, R};

/// The syntax each answers to "?".
const SYNTAX: &[(&str, &str)] = &[
    ("filecopy", "FileCopy <source>[, <destination>][, replace]"),
    ("erasefile", "EraseFile <file>"),
    ("fullmove", "FullMove <file or folder>, <destination folder>"),
    ("fullrename", "FullRename <file or folder>, <new name>"),
    ("getdir", "GetDir(<folder>[, <mode>][, <filter 1>...[, <filter N>]])"),
    ("getfinfo", "GetFInfo(<file>)"),
    ("fullsfput", "FullSFPut([<prompt>[, <default name>[, <folder>[, <max length>]]]])"),
    ("getbootvol", "GetBootVol()"),
    ("getsysfolder", "GetSysFolder()"),
];

/// The usual name of each, for its notice.
fn proper_name(name: &str) -> &'static str {
    match name {
        "filecopy" => "FileCopy",
        "erasefile" => "EraseFile",
        "fullmove" => "FullMove",
        "fullrename" => "FullRename",
        "getdir" => "GetDir",
        "getfinfo" => "GetFInfo",
        "fullsfput" => "FullSFPut",
        "getbootvol" => "GetBootVol",
        _ => "GetSysFolder",
    }
}

/// The answer to "!" or "?", if that's what was asked.
fn help(name: &str, args: &[String]) -> Option<String> {
    let syntax = SYNTAX.iter().find(|(n, _)| *n == name)?.1;
    match args.first().map(|a| a.trim()) {
        Some("!") => Some(format!("{} 2.0 — built into RandomCard, after Frédéric Rinaldi's external", proper_name(name))),
        Some("?") => Some(syntax.to_string()),
        _ => None,
    }
}

/// Rinaldi's wording for a failure.
fn error(e: &std::io::Error, path: &Path) -> String {
    use std::io::ErrorKind::*;
    match e.kind() {
        NotFound if path.parent().is_some_and(|p| !p.as_os_str().is_empty() && !p.exists()) => "Error : Directory not found".into(),
        NotFound => "Error : File not found".into(),
        AlreadyExists => "Error : Duplicate file name".into(),
        PermissionDenied | ReadOnlyFilesystem => "Error : Volume is locked".into(),
        InvalidInput | InvalidFilename => "Error : Bad Name".into(),
        _ => "Error : I/O Error".into(),
    }
}

impl Interp {
    /// A path from a script: as given, or a plain name in the stack's folder.
    fn script_path(&self, given: &str) -> PathBuf {
        let given = given.trim();
        let p = PathBuf::from(given);
        if p.is_absolute() || given.contains(std::path::MAIN_SEPARATOR) {
            return p;
        }
        let w = self.w();
        match w.stack.path.as_ref().or(w.stack.imported_from.as_ref()).and_then(|s| s.parent()) {
            Some(dir) => dir.join(given),
            None => p,
        }
    }

    /// A file function's answer, or None if it isn't one.
    pub(super) fn file_function(&mut self, name: &str, args: &[String]) -> Option<R<String>> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        if !matches!(name, "getdir" | "getfinfo" | "fullsfput" | "getbootvol" | "getsysfolder") {
            return None;
        }
        if let Some(h) = help(name, args) {
            return Some(Ok(h));
        }
        Some(Ok(match name {
            "getdir" => {
                let dir = self.script_path(&arg(0));
                get_dir(&dir, &arg(1), &args[args.len().min(2)..])
            }
            "getfinfo" => {
                let p = self.script_path(&arg(0));
                finder_info(&p)
            }
            // FullSFPut(prompt, name, folder, max length): a Save dialog,
            // answering the name, then the folder it goes in.
            "fullsfput" => {
                let prompt = if arg(0).is_empty() { "Save as:".to_string() } else { arg(0) };
                match self.file_dialog(&prompt, FileDialog::Save(arg(1))) {
                    Some(p) => {
                        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let folder = p.parent().map(|d| format!("{}{}", d.display(), std::path::MAIN_SEPARATOR)).unwrap_or_default();
                        format!("{name}\n{folder}")
                    }
                    None => String::new(),
                }
            }
            // The disk the computer started from.
            "getbootvol" => {
                if cfg!(target_os = "windows") {
                    std::env::var("SystemDrive").map(|d| format!("{d}\\")).unwrap_or_else(|_| "C:\\".into())
                } else {
                    "/".into()
                }
            }
            // The System Folder's place today: where settings go.
            "getsysfolder" => super::settings::config_dir()
                .map(|d| format!("{}{}", d.display(), std::path::MAIN_SEPARATOR))
                .unwrap_or_default(),
            _ => unreachable!(),
        }))
    }

    /// Run a file command, putting any error in the result; None if it isn't one.
    pub(super) fn file_command(&mut self, name: &str, args: &[String]) -> Option<R<()>> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        if !matches!(name, "filecopy" | "erasefile" | "fullmove" | "fullrename") {
            return None;
        }
        if let Some(h) = help(name, args) {
            self.result = h;
            return Some(Ok(()));
        }
        let outcome: Result<(), String> = match name {
            // FileCopy source, destination, replace: into a folder, to a new
            // name, or beside itself as "name copy".
            "filecopy" => {
                let src = self.script_path(&arg(0));
                let mut dest = if arg(1).trim().is_empty() {
                    let name = src.file_name().map(|n| format!("{} copy", n.to_string_lossy())).unwrap_or_default();
                    src.with_file_name(name)
                } else {
                    self.script_path(&arg(1))
                };
                if dest.is_dir() {
                    dest = dest.join(src.file_name().unwrap_or_default());
                }
                let replace = args.iter().skip(2).any(|a| a.trim().eq_ignore_ascii_case("replace") || a.trim() == "true");
                if !src.is_file() {
                    Err(if src.exists() { "Error : Bad Name".into() } else { error(&std::io::ErrorKind::NotFound.into(), &src) })
                } else if dest.exists() && !replace {
                    Err("Error : Duplicate file name".into())
                } else {
                    std::fs::copy(&src, &dest).map(|_| ()).map_err(|e| error(&e, &dest))
                }
            }
            "erasefile" => {
                let p = self.script_path(&arg(0));
                if p.is_dir() {
                    Err("Error : Bad Name".into())
                } else {
                    std::fs::remove_file(&p).map_err(|e| error(&e, &p))
                }
            }
            // FullMove item, folder: into another folder, keeping its name.
            "fullmove" => {
                let src = self.script_path(&arg(0));
                let folder = self.script_path(&arg(1));
                if !folder.is_dir() {
                    Err("Error : Directory not found".into())
                } else {
                    let dest = folder.join(src.file_name().unwrap_or_default());
                    if dest.exists() {
                        Err("Error : Duplicate file name".into())
                    } else {
                        std::fs::rename(&src, &dest).map_err(|e| error(&e, &src))
                    }
                }
            }
            // FullRename item, name: a new name in the same folder.
            "fullrename" => {
                let src = self.script_path(&arg(0));
                let new = arg(1).trim().to_string();
                if new.is_empty() || new.contains(['/', '\\']) {
                    Err("Error : Bad Name".into())
                } else {
                    let dest = src.with_file_name(&new);
                    if dest.exists() {
                        Err("Error : Duplicate file name".into())
                    } else {
                        std::fs::rename(&src, &dest).map_err(|e| error(&e, &src))
                    }
                }
            }
            _ => unreachable!(),
        };
        self.result = outcome.err().unwrap_or_default();
        Some(Ok(()))
    }
}

/// What GetDir and GetFInfo know about a file or folder.
struct Item {
    name: String,
    path: PathBuf,
    folder: bool,
    data: u64,
    rsrc: u64,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
    files: usize,
    locked: bool,
}

impl Item {
    fn read(path: &Path) -> Option<Item> {
        let meta = std::fs::metadata(path).ok()?;
        let folder = meta.is_dir();
        let rsrc = if folder { 0 } else { std::fs::metadata(rsrc_of(path)).map(|m| m.len()).unwrap_or(0) };
        let files = if folder { listing(path).iter().filter(|p| p.is_file()).count() } else { 0 };
        Some(Item {
            name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string()),
            path: path.to_path_buf(),
            folder,
            data: if folder { 0 } else { meta.len() },
            rsrc,
            created: meta.created().ok(),
            modified: meta.modified().ok(),
            files,
            locked: meta.permissions().readonly(),
        })
    }

    /// One of GetDir's letters: c creator, d data size, e creation date, f
    /// files in a folder, l and s sizes, m modification date, n name, p path,
    /// r resource size, t type, v version, x and y creation and modification
    /// times. Today's files have no Mac type or creator: "????".
    fn field(&self, c: char) -> String {
        match c {
            'c' | 't' => "????".into(),
            'd' => self.data.to_string(),
            'r' => self.rsrc.to_string(),
            'l' | 's' => (self.data + self.rsrc).to_string(),
            'e' => date(self.created),
            'm' => date(self.modified),
            'x' => time(self.created),
            'y' => time(self.modified),
            'f' => self.files.to_string(),
            'n' => self.name.clone(),
            'p' => self.path.display().to_string(),
            'v' => "Unknown".into(),
            _ => String::new(),
        }
    }
}

/// A file's resource fork, where Mac files unpacked here keep it.
fn rsrc_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".rsrc");
    PathBuf::from(s)
}

/// A folder's entries, sorted, without resource forks kept beside their files.
fn listing(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    v.retain(|p| {
        let s = p.to_string_lossy();
        !(s.ends_with(".rsrc") && Path::new(&s[..s.len() - 5]).exists())
    });
    v.sort();
    v
}

fn local(t: Option<SystemTime>) -> Option<chrono::DateTime<chrono::Local>> {
    t.map(chrono::DateTime::<chrono::Local>::from)
}

/// mm/dd/yy, as the Mac's short date had it.
fn date(t: Option<SystemTime>) -> String {
    local(t).map(|d| d.format("%-m/%-d/%y").to_string()).unwrap_or_default()
}

fn time(t: Option<SystemTime>) -> String {
    local(t).map(|d| d.format("%-H:%M").to_string()).unwrap_or_default()
}

/// A date filter's value: m/d/y, with '/', '-' or '.' between.
fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    let parts: Vec<i32> = s.split(['/', '-', '.']).map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let [m, d, y] = parts[..] else { return None };
    let y = if y < 100 { if y < 40 { 2000 + y } else { 1900 + y } } else { y };
    chrono::NaiveDate::from_ymd_opt(y, m as u32, d as u32)
}

/// GetDir(folder, mode, filters…): the folder's files and/or folders, one
/// per line, each as the "o=" filter lays it out (the name, by default). The
/// mode's first letter picks files (F), folders (D) or both; R as its
/// second searches folders inside too.
fn get_dir(dir: &Path, mode: &str, filters: &[String]) -> String {
    if !dir.is_dir() {
        return if dir.exists() { "Error : Not a directory".into() } else { "Error : Directory not found".into() };
    }
    let mode = mode.trim().to_ascii_uppercase();
    let (files, folders) = match mode.chars().next() {
        Some('F') => (true, false),
        Some('D') => (false, true),
        _ => (true, true),
    };
    let recursive = mode.chars().nth(1) == Some('R');
    let mut output = "n".to_string();
    let mut tests = Vec::new();
    for f in filters.iter().map(|f| f.trim()).filter(|f| !f.is_empty()) {
        let mut chars = f.chars();
        let (Some(key), Some(op)) = (chars.next().map(|c| c.to_ascii_lowercase()), chars.next()) else {
            return "Error : Missing comparison value".into();
        };
        let value: String = chars.collect();
        if key == 'o' {
            if op != '=' {
                return "Error : Only \"o=\" allowed".into();
            }
            output = value.to_lowercase();
            continue;
        }
        if !"cdeflmnrstxy".contains(key) {
            return "Error : First char of filter param must be [c,d,e,f,l,m,n,o,r,s,t,x,y]".into();
        }
        if !"=≠<>≥≤".contains(op) {
            return "Error : Second char of filter param must be [=,≠,>,<,≥,≤]".into();
        }
        if value.is_empty() {
            return format!("Error : Missing comparison value for \"{key}{op}\" test");
        }
        if matches!(key, 'e' | 'm') && parse_date(&value).is_none() {
            return "Error : Not a valid date".into();
        }
        tests.push((key, op, value));
    }
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for p in listing(&d) {
            let Some(item) = Item::read(&p) else { continue };
            if item.folder && recursive {
                stack.push(p.clone());
            }
            if (item.folder && !folders) || (!item.folder && !files) {
                continue;
            }
            if tests.iter().all(|(k, op, v)| passes(&item, *k, *op, v)) {
                out.push(output.chars().map(|c| item.field(c)).collect::<Vec<_>>().join(","));
            }
        }
    }
    out.join("\n")
}

/// Whether an item passes one filter. For names, = is contains, ≠ doesn't
/// contain, < begins with, > ends with, ≥ matches and ≤ doesn't; numbers,
/// dates and times compare.
fn passes(item: &Item, key: char, op: char, value: &str) -> bool {
    use std::cmp::Ordering::*;
    let order = match key {
        'n' => {
            let (n, v) = (item.name.to_lowercase(), value.to_lowercase());
            return match op {
                '=' => n.contains(&v),
                '≠' => !n.contains(&v),
                '<' => n.starts_with(&v),
                '>' => n.ends_with(&v),
                '≥' => n == v,
                _ => n != v,
            };
        }
        'c' | 't' => "????".cmp(value),
        'e' | 'm' => {
            let t = if key == 'e' { item.created } else { item.modified };
            let (Some(d), Some(v)) = (local(t).map(|d| d.date_naive()), parse_date(value)) else { return false };
            d.cmp(&v)
        }
        'x' | 'y' => {
            let t = if key == 'x' { item.created } else { item.modified };
            let v: Vec<u32> = value.split(':').filter_map(|p| p.trim().parse().ok()).collect();
            let Some(d) = local(t) else { return false };
            use chrono::Timelike;
            (d.hour(), d.minute()).cmp(&(v.first().copied().unwrap_or(0), v.get(1).copied().unwrap_or(0)))
        }
        _ => {
            let n: u64 = item.field(key).parse().unwrap_or(0);
            n.cmp(&value.trim().parse().unwrap_or(0))
        }
    };
    match op {
        '=' => order == Equal,
        '≠' => order != Equal,
        '<' => order == Less,
        '>' => order == Greater,
        '≥' => order != Less,
        _ => order != Greater,
    }
}

/// GetFInfo(file): type, creator, creation and modification dates, total,
/// data and resource sizes, then the Finder's 16 flags. Today's files have
/// no Mac type, creator or flags, so those answer "????" and false, except
/// invisible (a name starting with a period) and locked (read-only).
fn finder_info(path: &Path) -> String {
    let Some(item) = Item::read(path) else {
        return error(&std::io::ErrorKind::NotFound.into(), path);
    };
    if item.folder {
        return "Error : Bad Name".into();
    }
    let mut v = vec![
        "????".to_string(),
        "????".to_string(),
        date(item.created),
        date(item.modified),
        (item.data + item.rsrc).to_string(),
        item.data.to_string(),
        item.rsrc.to_string(),
    ];
    // onDesk, bFOwnAppl, reserved, bfNever, bfAlways, shared, reserved,
    // Inited, Changed, Busy, NoCopy, System, hasBundle, Invisible, Locked.
    let mut flags = [false; 16];
    flags[14] = item.name.starts_with('.');
    flags[15] = item.locked;
    v.extend(flags.iter().map(|f| f.to_string()));
    v.join(",")
}
