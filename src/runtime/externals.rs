//! Externals that RandomCard does itself. HyperCard stacks called XCMDs and
//! XFCNs, compiled 68000 code in their resource forks, for things HyperCard
//! lacked. RandomCard can't run that code, so it does the job instead and
//! the external's name keeps working (see XCMDS.md). As in HyperCard, these
//! are tried after the message path, so a script handler with the same name
//! always wins.

use std::path::PathBuf;

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
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        Some(match name {
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
            // TNPict file, …: shows a PICT file in a window. RandomCard can't
            // read PICT pictures yet, so this reports it as TNPict reported
            // its errors.
            "tnpict" => {
                self.result = "Error RandomCard can't show PICT pictures yet.".into();
                Ok(())
            }
            _ => return None,
        })
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
