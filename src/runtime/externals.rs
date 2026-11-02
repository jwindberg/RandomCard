//! Externals that RandomCard does itself. HyperCard stacks called XCMDs and
//! XFCNs, compiled 68000 code in their resource forks, for things HyperCard
//! lacked. RandomCard can't run that code, so it does the job instead and
//! the external's name keeps working (see XCMDS.md). As in HyperCard, these
//! are tried after the message path, so a script handler with the same name
//! always wins.

use std::path::PathBuf;

use crate::hypertalk::value::parse_point;

use crate::model::metrics;
use crate::print;

use super::interp::{Interp, R, to_int, to_number};
use crate::hypertalk::ast::NameLength;
use super::world::{Dialog, DialogReply};

/// Which file dialog to show.
pub enum FileDialog {
    Open,
    Save(String),
    Folder,
}

impl Interp {
    /// Where a pop-up menu goes: the point given, or the mouse.
    fn menu_point(&self, left: Option<&String>, top: Option<&String>) -> (i32, i32) {
        let w = self.w();
        let (mx, my) = (w.mouse.x, w.mouse.y);
        let n = |v: Option<&String>| v.and_then(|v| to_int(v).ok()).map(|n| n as i32);
        (n(left).unwrap_or(mx), n(top).unwrap_or(my))
    }

    /// Show a pop-up menu and wait for the choice: an item number from 1,
    /// or None if nothing was chosen. Dividers and disabled items can't be.
    fn menu_choice(&mut self, items: Vec<String>, (left, top): (i32, i32), checked: i64) -> R<Option<u32>> {
        if items.is_empty() {
            return Ok(None);
        }
        let checked = checked.clamp(0, items.len() as i64) as u32;
        Ok(match self.dialog(Dialog::Menu { items, left, top, checked })? {
            DialogReply::Text(Some(n)) | DialogReply::Button(n) => n.parse().ok(),
            DialogReply::Text(None) => None,
        })
    }

    /// Show a system file dialog. With no UI (scripts run in tests), it's
    /// canceled straight away.
    pub fn file_dialog(&self, title: &str, kind: FileDialog) -> Option<PathBuf> {
        self.dialog_rx.as_ref()?;
        let dialog = rfd::FileDialog::new().set_title(title);
        match kind {
            FileDialog::Open => dialog.pick_file(),
            FileDialog::Save(name) => dialog.set_file_name(name).save_file(),
            FileDialog::Folder => dialog.pick_folder(),
        }
    }

    /// An external function's value, or `None` if there's no such external.
    pub(super) fn external_function(&mut self, name: &str, args: &[String]) -> Option<R<String>> {
        if let Some(v) = self.file_function(name, args) {
            return Some(v);
        }
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        let path = |p: Option<PathBuf>| p.map(|p| p.display().to_string()).unwrap_or_default();
        Some(Ok(match name {
            // FileExists came in two versions. The Developer Technical
            // Support stacks' answers a Mac error code (0 when the file is
            // there); the others answer true or false.
            "fileexists" => {
                let there = std::path::Path::new(&arg(0)).exists();
                if self.stack_compares_fileexists_with_zero() {
                    if there { "0" } else { "-43" }.into()
                } else {
                    there.to_string()
                }
            }
            // FileName(type, prompt): an Open dialog.
            "filename" => path(self.file_dialog(&arg(1), FileDialog::Open)),
            // FilePath(type, prompt), from the Rinaldi collection: the same.
            "filepath" => path(self.file_dialog(&arg(1), FileDialog::Open)),
            // NewFileName(prompt, name): a Save dialog.
            "newfilename" => path(self.file_dialog(&arg(0), FileDialog::Save(arg(1)))),
            // MacType(): the kind of computer, as SysEnv's first item.
            "mactype" => match self.external_function("sysenv", &[])? {
                Ok(env) => env.split(',').next().unwrap_or_default().to_string(),
                Err(e) => return Some(Err(e)),
            },
            // PopUpMenu(items, checked item, top, left): a pop-up menu,
            // answering the chosen item's number, or 0.
            "popupmenu" => {
                let at = self.menu_point(args.get(3), args.get(2));
                match self.menu_choice(menu_items(&arg(0)), at, to_int(&arg(1)).unwrap_or(0)) {
                    Ok(n) => n.unwrap_or(0).to_string(),
                    Err(e) => return Some(Err(e)),
                }
            }
            // PopMenu(top, left, checked item, items), by Appropriate
            // Technology: answers "true,item,menu" or empty. Another PopMenu
            // takes (point, lines) and answers the line number, or 0.
            "popmenu" => {
                if args.len() <= 2 {
                    let at = parse_point(&arg(0)).unwrap_or_else(|| self.menu_point(None, None));
                    match self.menu_choice(menu_items(&arg(1)), at, 0) {
                        Ok(n) => n.unwrap_or(0).to_string(),
                        Err(e) => return Some(Err(e)),
                    }
                } else {
                    let at = self.menu_point(args.get(1), args.first());
                    match self.menu_choice(menu_items(&arg(3)), at, to_int(&arg(2)).unwrap_or(0)) {
                        Ok(Some(n)) => format!("true,{n},1"),
                        Ok(None) => String::new(),
                        Err(e) => return Some(Err(e)),
                    }
                }
            }
            // INITFinder(how): the system extensions (INITs) running. None
            // that mean anything to a stack run today.
            "initfinder" => String::new(),
            // TotalRAM(): the computer's memory, in bytes.
            "totalram" => total_ram().to_string(),
            // BoardInfo(): NuBus cards, between a heading and a last line.
            // There are none.
            "boardinfo" => "\n".into(),
            // GetVol(): the mounted disks, one per line, as paths.
            "getvol" => volumes().join("\n"),
            // ProgressWindoid(name, loc, [width, text, fraction, font, size]):
            // a small window with a label over a progress bar, opened hidden;
            // scripts show it and set its fraction as work goes on. Answers
            // empty, or an error message starting with "•".
            "progresswindoid" => {
                let name = arg(0).trim().to_string();
                if name.is_empty() {
                    return Some(Ok("• ProgressWindoid needs a window name.".into()));
                }
                if self
                    .w()
                    .script_windows
                    .iter()
                    .any(|t| t.name.eq_ignore_ascii_case(&name))
                {
                    return Some(Ok(format!("• There is already a window named {name}.")));
                }
                let Some((x, y)) = parse_point(&arg(1)) else {
                    return Some(Ok(format!(
                        "• \"{}\" isn't a point for the window's location.",
                        arg(1)
                    )));
                };
                let width = to_int(&arg(2)).map(|w| w.clamp(140, 1200)).unwrap_or(250) as i32;
                let text = if args.len() > 3 {
                    arg(3)
                } else {
                    "Progress:".into()
                };
                let fraction = arg(4).trim().parse::<f64>().unwrap_or(0.0).clamp(0.0, 1.0);
                let font = if arg(5).trim().is_empty() {
                    "Geneva".to_string()
                } else {
                    arg(5)
                };
                let size = to_int(&arg(6)).map(|s| s.clamp(1, 127)).unwrap_or(12) as u32;
                let height = size as i32 + 34;
                self.w().script_windows.push(super::world::ScriptWindow {
                    name,
                    kind: super::world::WindowKind::Progress(fraction),
                    text,
                    rect: crate::model::Rect::new(x, y, x + width, y + height),
                    font,
                    size,
                    visible: false,
                    dirty: false,
                });
                String::new()
            }
            // XVersion(n): a version number, for a stack's headers. RandomCard's.
            "xversion" => env!("CARGO_PKG_VERSION").to_string(),
            // FileN(prompt), from the Rinaldi collection: an Open dialog.
            "filen" => path(self.file_dialog(&arg(0), FileDialog::Open)),
            // FolderPath(prompt) and SelectDir(prompt): a dialog for choosing
            // a folder, answering its path with a separator at the end, as
            // a Mac folder path ended in a colon.
            "folderpath" | "selectdir" => match self.file_dialog(&arg(0), FileDialog::Folder) {
                Some(p) => format!("{}{}", p.display(), std::path::MAIN_SEPARATOR),
                None => String::new(),
            },
            // LeafName(path): the last part of a path, the file or folder's
            // own name. A Mac path ("Disk:Folder:File") works too.
            "leafname" => {
                let p = arg(0);
                let p = p.trim_end_matches(['/', '\\', ':']);
                let sep: &[char] = if p.contains(['/', '\\']) { &['/', '\\'] } else { &[':'] };
                p.rsplit(sep).next().unwrap_or_default().to_string()
            }
            // FolderName(prompt): a dialog for choosing a folder.
            "foldername" => path(self.file_dialog(&arg(0), FileDialog::Folder)),
            // SFPutFile(name), or SFPutFile(prompt, name): a Save dialog.
            "sfputfile" => {
                let (title, name) = if args.len() > 1 { (arg(0), arg(1)) } else { ("Save as:".into(), arg(0)) };
                path(self.file_dialog(&title, FileDialog::Save(name)))
            }
            // SysPrefs(name): the preferences folder, as "0,folder,", or an
            // error in item 3. The folder ends with a separator, so a stack
            // puts the file's name after it.
            "sysprefs" => match super::settings::config_dir().map(|d| (std::fs::create_dir_all(&d), d)) {
                Some((Ok(()), d)) => format!("0,{}{},", d.display(), std::path::MAIN_SEPARATOR),
                _ => ",,FileExistsError".into(),
            },
            // SysEnv(): line 1 is machine, system version, processor, FPU and
            // Color QuickDraw; line 2 starts with the addressing mode.
            "sysenv" | "sysenvironment" => {
                let os = match std::env::consts::OS {
                    "macos" => "Mac",
                    "linux" => "Linux",
                    "windows" => "Windows",
                    "haiku" => "Haiku",
                    other => other,
                };
                let bits = usize::BITS;
                format!("{os},{},{},true,true\n{bits},", self.builtin("systemversion", NameLength::Default, vec![]).ok()?, std::env::consts::ARCH)
            }
            // isnum(value): whether it's a number. Empty counts, so a script
            // asking until it gets a number stops when Cancel is clicked.
            // dayMonthName(form, n), from HyperCard 1.2's DateBook: an odd
            // form names day n (1 is Sunday), an even one month n, and a form
            // above 2 abbreviates. It was an XFCN to get the Mac's own
            // language; RandomCard's dates are in English.
            "daymonthname" => {
                const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
                const MONTHS: [&str; 12] = [
                    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
                    "November", "December",
                ];
                let form = to_int(&arg(0)).unwrap_or(1);
                let n = to_int(&arg(1)).unwrap_or(1).max(1) as usize - 1;
                let names: &[&str] = if form % 2 == 1 { &DAYS } else { &MONTHS };
                let name = names.get(n % names.len()).copied().unwrap_or_default();
                if form > 2 { name.chars().take(3).collect() } else { name.to_string() }
            }
            "isnum" => {
                let s = arg(0);
                (s.trim().is_empty() || to_number(&s).is_ok()).to_string()
            }
            // dialog(message, button…, default button, font, style, size):
            // an alert. RandomCard's alerts use its own font.
            "dialog" => {
                let buttons: Vec<String> =
                    args.iter().skip(1).take_while(|a| to_int(a).is_err()).cloned().collect();
                let buttons = if buttons.is_empty() { vec!["OK".to_string()] } else { buttons };
                match self.dialog(Dialog::Answer { prompt: arg(0), buttons: buttons.clone() }) {
                    Ok(DialogReply::Button(b)) => b,
                    Ok(DialogReply::Text(t)) => t.unwrap_or_else(|| buttons[0].clone()),
                    Err(e) => return Some(Err(e)),
                }
            }
            // StringWidth(text): its width in pixels, in the system font.
            "stringwidth" => self.fmt(metrics::metrics("Chicago", 12, "plain").text_width(&arg(0)) as f64),
            // Main(text): ROT13, an easter egg in the Technical Notes stacks.
            "main" => arg(0).chars().map(rot13).collect(),
            _ => return None,
        }))
    }

    /// Run an external command, or answer `None` if there's no such external.
    pub(super) fn external_command(&mut self, name: &str, args: &[String]) -> Option<R<()>> {
        if let Some(r) = self.file_command(name, args) {
            return Some(r);
        }
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        Some(match name {
            // TEXWindow name, text, left, top, right, bottom, font, size,
            // line height, line length: open a window of editable text, or
            // bring back one of that name. Closing it sends TEXWindowClosing
            // name, changed to the card.
            "texwindow" => {
                let n = |i: usize, d: i64| to_int(&arg(i)).unwrap_or(d) as i32;
                let rect = crate::model::Rect::new(n(2, 40), n(3, 40), n(4, 440), n(5, 300));
                let font = if arg(6).trim().is_empty() { "Geneva".to_string() } else { arg(6) };
                let size = to_int(&arg(7)).unwrap_or(12).clamp(6, 72) as u32;
                let window = super::world::ScriptWindow {
                    name: arg(0),
                    kind: super::world::WindowKind::Text,
                    text: arg(1),
                    rect,
                    font,
                    size,
                    visible: true,
                    dirty: false,
                };
                let mut w = self.w();
                w.script_windows
                    .retain(|t| !t.name.eq_ignore_ascii_case(&window.name));
                w.script_windows.push(window);
                Ok(())
            }
            // INITFinder how, "noDialog": the running INITs, in the result.
            "initfinder" => {
                self.result = String::new();
                Ok(())
            }
            // SwitchXRoo: switch the monitor's depth; with no arguments, a
            // line per monitor in the result.
            "switchxroo" => {
                let r = self.w().screen_rect;
                self.result = format!("Main screen, {}x{}, millions of colors", r.width(), r.height());
                Ok(())
            }
            // MakeSendMe5 to, cc, subject, text (and MakeSendMe4, its older
            // version): an AppleLink message, from the Q & A stack. Now a new
            // message in the user's own mail program, filled in, to send.
            "makesendme5" | "makesendme4" => {
                let url = mailto(&arg(0), &arg(1), &arg(2), &arg(3));
                self.w()
                    .requests
                    .push(super::world::UiRequest::OpenUrl(url));
                self.result = String::new();
                Ok(())
            }
            // printOut text, font, size, style: print text in a font.
            "printout" => {
                let size = to_int(&arg(2)).unwrap_or(10).clamp(1, 127) as u32;
                let font = if arg(1).is_empty() { "Geneva".to_string() } else { arg(1) };
                let style = if arg(3).is_empty() { "plain".to_string() } else { arg(3) };
                let name = format!("{} text", self.w().stack.name);
                let pdf = print::text_pdf_in(&arg(0), &font, size, &style);
                self.emit(&name, pdf)
            }
            // SelectLineXCMD field, line: select a line of a field.
            "selectlinexcmd" => match to_int(&arg(1)) {
                Ok(n) => {
                    let me = self.frame().me;
                    self.send_text(&format!("select line {n} of {}", arg(0)), me)
                }
                Err(_) => Ok(()),
            },
            // An animated About box from 1989.
            "spinabout" => Ok(()),
            // TNPict file, …: shows a PICT file in a window. The stacks pass a
            // bare name ("HIN 017 - Figure 1"), found beside the stack or in
            // a folder there. Errors are reported in the result, as TNPict
            // reported them.
            "tnpict" => {
                self.result = match self.find_beside_stack(&arg(0)) {
                    None => "Error file not found".into(),
                    // RandomCard can't read PICT pictures yet.
                    Some(_) => "Error RandomCard can't show PICT pictures yet.".into(),
                };
                Ok(())
            }
            _ => return None,
        })
    }

    /// A file named by a stack: as given, beside the stack, or in a folder
    /// beside the stack (where Apple's CDs put a stack's pictures).
    fn find_beside_stack(&self, name: &str) -> Option<PathBuf> {
        let given = PathBuf::from(name);
        if given.is_absolute() {
            return given.is_file().then_some(given);
        }
        let dir = {
            let w = self.w();
            w.stack.path.clone().or_else(|| w.stack.imported_from.clone())?.parent()?.to_path_buf()
        };
        let here = dir.join(name);
        if here.is_file() {
            return Some(here);
        }
        let mut folders: Vec<PathBuf> =
            std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        folders.sort();
        folders.into_iter().map(|f| f.join(name)).find(|p| p.is_file())
    }

    /// Whether this stack's scripts test FileExists against 0, as the
    /// version answering error codes needs.
    fn stack_compares_fileexists_with_zero(&mut self) -> bool {
        if let Some(b) = self.fileexists_codes {
            return b;
        }
        let w = self.w();
        let s = &w.stack;
        let scripts = std::iter::once(&s.script)
            .chain(s.backgrounds.iter().flat_map(|b| std::iter::once(&b.script).chain(b.parts.iter().map(|p| &p.script))))
            .chain(s.cards.iter().flat_map(|c| std::iter::once(&c.script).chain(c.parts.iter().map(|p| &p.script))));
        let found = scripts.into_iter().any(|script| {
            let lower = script.to_ascii_lowercase();
            lower.match_indices("fileexists(").any(|(i, _)| {
                let rest = &lower[i..];
                let Some(close) = rest.find(')') else { return false };
                let after = rest[close + 1..].trim_start();
                ["= 0", "≠ 0", "<> 0", "is 0", "is not 0"].iter().any(|op| after.starts_with(op))
            })
        });
        drop(w);
        self.fileexists_codes = Some(found);
        found
    }
}

fn rot13(c: char) -> char {
    match c {
        'a'..='z' => (((c as u8 - b'a') + 13) % 26 + b'a') as char,
        'A'..='Z' => (((c as u8 - b'A') + 13) % 26 + b'A') as char,
        _ => c,
    }
}

/// A pop-up menu's items: one per line, or separated by commas when
/// they're all on one line. "(-" is a divider, as in HyperCard's menus.
fn menu_items(list: &str) -> Vec<String> {
    let list = list.trim_end_matches(['\n', '\r']);
    let items: Vec<&str> = if list.contains(['\n', '\r']) { list.split(['\n', '\r']).collect() } else { list.split(',').collect() };
    items
        .into_iter()
        .map(|i| i.trim_start_matches(';').trim())
        .filter(|i| !i.is_empty())
        .map(|i| if i == "(-" { "-".to_string() } else { i.to_string() })
        .collect()
}

/// The computer's memory in bytes, where the system says; 0 if unknown.
fn total_ram() -> u64 {
    if let Ok(info) = std::fs::read_to_string("/proc/meminfo")
        && let Some(kb) = info.lines().find_map(|l| l.strip_prefix("MemTotal:"))
    {
        return kb.trim().trim_end_matches("kB").trim().parse::<u64>().unwrap_or(0) * 1024;
    }
    if let Ok(out) = std::process::Command::new("sysctl").args(["-n", "hw.memsize", "hw.physmem"]).output() {
        return String::from_utf8_lossy(&out.stdout).lines().find_map(|l| l.trim().parse().ok()).unwrap_or(0);
    }
    0
}

/// A `mailto:` URL for a message, with every part encoded.
fn mailto(to: &str, cc: &str, subject: &str, body: &str) -> String {
    fn enc(s: &str) -> String {
        s.replace("\r\n", "\n")
            .replace('\r', "\n")
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'@' => {
                    (b as char).to_string()
                }
                b'\n' => "%0D%0A".to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    }
    let mut parts = Vec::new();
    for (key, value) in [("cc", cc), ("subject", subject), ("body", body)] {
        if !value.trim().is_empty() {
            parts.push(format!("{key}={}", enc(value)));
        }
    }
    let query = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };
    format!("mailto:{}{query}", enc(to.trim()))
}

/// The mounted disks: on a Mac the volumes in /Volumes; on Linux and the
/// like the root and whatever is mounted under /media, /run/media and
/// /mnt; on Windows the drive letters.
fn volumes() -> Vec<String> {
    let mut out = Vec::new();
    let list = |dir: &str| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .map(|p| p.display().to_string())
            .collect();
        v.sort();
        v
    };
    if cfg!(target_os = "macos") {
        out = list("/Volumes");
    } else if cfg!(target_os = "windows") {
        out = (b'A'..=b'Z')
            .map(|c| format!("{}:\\", c as char))
            .filter(|d| std::path::Path::new(d).exists())
            .collect();
    } else {
        out.push("/".to_string());
        let user = std::env::var("USER").unwrap_or_default();
        for dir in [
            format!("/media/{user}"),
            format!("/run/media/{user}"),
            "/mnt".to_string(),
        ] {
            out.extend(list(&dir));
        }
    }
    out
}
