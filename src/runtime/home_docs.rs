//! HyperCard 1.2's Home and its Documents stack: `getHomeInfo`, which fills
//! the globals Home's cards set (the user's name and the search paths),
//! `GetDocs`, which gives the Documents stack a card for each file in a
//! folder, and `GetCreator`, which names the program for a creator code.
//! HyperCard did the first in Home's script and the other two as XCMDs in
//! the Documents stack.

use std::path::{Path, PathBuf};

use super::interp::{Interp, R};
use crate::model::{Card, Picture};

/// The Documents stack's fields, filled for each file.
const FIELDS: [&str; 7] = ["Name", "Where", "Type", "Creator", "Created", "Modified", "Size"];

impl Interp {
    /// getHomeInfo: the globals userName, stacks, applications and
    /// documents, from Home's User Preferences and search-path cards, one
    /// folder a line. Run at startup too, as Home's startUp did.
    pub fn home_info(&mut self) {
        let (name, paths) = {
            let mut w = self.w();
            w.load_home();
            let Some(home) = w.home_mut() else { return };
            let name = home
                .cards
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case("User Preferences"))
                .and_then(|c| c.parts.iter().find(|p| p.name.eq_ignore_ascii_case("User Name")).map(|p| p.contents.clone()))
                .unwrap_or_default();
            let paths: Vec<(String, String)> = ["Stacks", "Applications", "Documents"]
                .iter()
                .map(|k| (k.to_lowercase(), crate::model::home::search_paths(home, k).join("\n")))
                .collect();
            (name, paths)
        };
        self.globals.insert("username".into(), name);
        for (k, v) in paths {
            self.globals.insert(k, v);
        }
    }

    /// GetDocs folder: a card for each file in the folder that the stack
    /// has no card for yet, on the background with the Documents fields.
    /// ":" is HyperCard's own folder, which for RandomCard is Home's.
    pub(super) fn get_docs(&mut self, folder: &str) -> R<()> {
        let folder = folder.trim();
        let dir: PathBuf = if folder.is_empty() || folder == ":" {
            let w = self.w();
            let home = w.home_path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf);
            let here = w.stack.path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf);
            match home.or(here) {
                Some(d) => d,
                None => return Ok(()),
            }
        } else {
            self.script_path(folder)
        };
        let mut w = self.w();
        // The background whose fields these are.
        let Some(bg) = w
            .stack
            .backgrounds
            .iter()
            .find(|b| b.parts.iter().any(|p| p.name.eq_ignore_ascii_case("Name")) && b.parts.iter().any(|p| p.name.eq_ignore_ascii_case("Creator")))
            .map(|b| (b.id, b.parts.iter().map(|p| (p.name.to_lowercase(), p.id)).collect::<Vec<_>>()))
        else {
            return Ok(());
        };
        let (bg_id, fields) = bg;
        let field = |n: &str| fields.iter().find(|(f, _)| f == &n.to_lowercase()).map(|(_, id)| *id);
        let name_field = field("Name");
        let have: Vec<String> = w
            .stack
            .cards
            .iter()
            .filter(|c| c.background == bg_id)
            .filter_map(|c| name_field.and_then(|id| c.bg_text.get(&id).cloned()))
            .collect();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            let Ok(meta) = std::fs::metadata(&path) else { continue };
            let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if meta.is_dir() || file_name.starts_with('.') || file_name.ends_with(".rsrc") {
                continue;
            }
            let shown = super::paths::shown(&path);
            if have.contains(&shown) {
                continue;
            }
            let (kind, creator) = mac_kind(&path, &meta);
            let date = |t: std::io::Result<std::time::SystemTime>| {
                t.ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%-m/%-d/%y").to_string()).unwrap_or_default()
            };
            let values = [
                shown.clone(),
                super::paths::folder(&dir),
                kind.to_string(),
                creator.to_string(),
                date(meta.created()),
                date(meta.modified()),
                meta.len().to_string(),
            ];
            let id = w.stack.alloc_id();
            let (width, height) = (w.stack.width, w.stack.height);
            let mut card = Card { id, background: bg_id, picture: Picture::new(width, height), show_pict: true, ..Card::default() };
            for (name, value) in FIELDS.iter().zip(values) {
                if let Some(f) = field(name) {
                    card.bg_text.insert(f, value);
                }
            }
            w.stack.cards.push(card);
            w.dirty = true;
        }
        Ok(())
    }
}

/// A file's Mac type and creator, as far as today's files show them: a
/// HyperCard stack is STAK by WILD, a program APPL, anything else unknown.
fn mac_kind(path: &Path, meta: &std::fs::Metadata) -> (&'static str, &'static str) {
    let mut head = [0u8; 8];
    let stack = path.extension().is_some_and(|e| e == "rcstack")
        || std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head[4..8] == b"STAK";
    if stack {
        return ("STAK", "WILD");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 != 0 {
            return ("APPL", "????");
        }
    }
    #[cfg(windows)]
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        return ("APPL", "????");
    }
    let _ = meta;
    ("????", "????")
}

/// GetCreator(code): the program a creator code belongs to. HyperCard is
/// WILD; for the rest, today's files open in the program the system picks,
/// which `open <document> with <program>` uses whatever it's called.
pub fn creator_name(code: &str) -> String {
    match code.trim() {
        "WILD" => "HyperCard".into(),
        "" => String::new(),
        _ => "Default Program".into(),
    }
}
