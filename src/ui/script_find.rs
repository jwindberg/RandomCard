//! The script editor's Find and Replace, as in HyperCard 2.4's Script menu:
//! Find (Command-F), Find Again (G), Find Selection (H, or E on a Mac,
//! which keeps ⌘H), Replace (R) and Replace Again (T), with HyperCard's
//! Whole Word, Case Sensitive and Wraparound Search. Go to Line (J) is
//! RandomCard's own.
//!
//! Positions are character indexes, as the text editor's cursor counts them.

use eframe::egui::Key;

/// What's being looked for, shared by every script editor, so Find Again
/// works in the next script too.
#[derive(Clone, Debug)]
pub struct FindOptions {
    pub find: String,
    pub replace: String,
    pub whole_word: bool,
    pub case_sensitive: bool,
    pub wraparound: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        FindOptions { find: String::new(), replace: String::new(), whole_word: false, case_sensitive: false, wraparound: true }
    }
}

/// A script editor command, from its Command key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorCommand {
    Find,
    FindAgain,
    FindSelection,
    Replace,
    ReplaceAgain,
    GoToLine,
    Save,
    Checkpoint,
}

/// The Command keys a script editor takes for itself while it has the
/// keyboard, ahead of the menus.
pub const EDITOR_KEYS: [(Key, &str, EditorCommand); 8] = [
    (Key::F, "f", EditorCommand::Find),
    (Key::G, "g", EditorCommand::FindAgain),
    (FIND_SELECTION_KEY, FIND_SELECTION_CHAR, EditorCommand::FindSelection),
    (Key::R, "r", EditorCommand::Replace),
    (Key::T, "t", EditorCommand::ReplaceAgain),
    (Key::J, "j", EditorCommand::GoToLine),
    (Key::S, "s", EditorCommand::Save),
    (Key::D, "d", EditorCommand::Checkpoint),
];

const FIND_SELECTION_KEY: Key = if cfg!(target_os = "macos") { Key::E } else { Key::H };
pub const FIND_SELECTION_CHAR: &str = if cfg!(target_os = "macos") { "e" } else { "h" };

/// The editor command for a Command key typed as `ch`.
#[cfg(any(target_os = "macos", test))]
pub fn command_for(ch: &str) -> Option<EditorCommand> {
    EDITOR_KEYS.iter().find(|(_, c, _)| c.eq_ignore_ascii_case(ch)).map(|(_, _, cmd)| *cmd)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn matches_at(text: &[char], at: usize, pattern: &[char], o: &FindOptions) -> bool {
    let end = at + pattern.len();
    if pattern.is_empty() || end > text.len() {
        return false;
    }
    let same = text[at..end].iter().zip(pattern).all(|(a, b)| {
        if o.case_sensitive { a == b } else { a.to_lowercase().eq(b.to_lowercase()) }
    });
    same && (!o.whole_word || ((at == 0 || !is_word(text[at - 1])) && (end == text.len() || !is_word(text[end]))))
}

/// The next match at or after `from`, wrapping to the top if Wraparound
/// Search is on.
pub fn find(text: &str, from: usize, o: &FindOptions) -> Option<(usize, usize)> {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = o.find.chars().collect();
    let found = |range: std::ops::Range<usize>| range.into_iter().find(|&i| matches_at(&text, i, &pattern, o));
    let from = from.min(text.len());
    found(from..text.len())
        .or_else(|| if o.wraparound { found(0..from) } else { None })
        .map(|i| (i, i + pattern.len()))
}

/// `text` with the characters from `start` to `end` replaced by `with`.
pub fn replace_range(text: &str, (start, end): (usize, usize), with: &str) -> String {
    let mut out: String = text.chars().take(start).collect();
    out.push_str(with);
    out.extend(text.chars().skip(end));
    out
}

/// Replace: the next match at or after `from` is replaced. Returns the new
/// text and where the replacement is, or None if there's no match.
pub fn replace_next(text: &str, from: usize, o: &FindOptions) -> Option<(String, (usize, usize))> {
    let (start, end) = find(text, from, o)?;
    let new = replace_range(text, (start, end), &o.replace);
    Some((new, (start, start + o.replace.chars().count())))
}

/// Replace All: every match replaced, and how many there were.
pub fn replace_all(text: &str, o: &FindOptions) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = o.find.chars().collect();
    let (mut out, mut count, mut i) = (String::new(), 0, 0);
    while i < chars.len() {
        if matches_at(&chars, i, &pattern, o) {
            out.push_str(&o.replace);
            i += pattern.len();
            count += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    (out, count)
}

/// Where line `line` (from 1) starts and ends, past the last line's end
/// if there aren't that many.
pub fn line_range(text: &str, line: usize) -> (usize, usize) {
    let start: usize = text.split('\n').take(line.saturating_sub(1)).map(|l| l.chars().count() + 1).sum();
    let len = text.split('\n').nth(line.saturating_sub(1)).map(|l| l.chars().count()).unwrap_or(0);
    let start = start.min(text.chars().count());
    (start, start + len)
}

/// The line (from 1) that character `at` is on.
pub fn line_of(text: &str, at: usize) -> usize {
    text.chars().take(at).filter(|c| *c == '\n').count() + 1
}

/// The characters from `start` to `end`.
pub fn slice(text: &str, (start, end): (usize, usize)) -> String {
    text.chars().skip(start).take(end.saturating_sub(start)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(find: &str) -> FindOptions {
        FindOptions { find: find.into(), replace: "x".into(), ..Default::default() }
    }

    #[test]
    fn finding() {
        let text = "on mouseUp\n  put Mouse into m\n  -- mouseUp again\nend mouseUp";
        // Case doesn't matter unless asked; the search goes on from `from`.
        assert_eq!(find(text, 0, &opts("mouse")), Some((3, 8)));
        assert_eq!(find(text, 4, &opts("mouse")), Some((17, 22)));
        let case = FindOptions { case_sensitive: true, ..opts("Mouse") };
        assert_eq!(find(text, 0, &case), Some((17, 22)));
        // Whole words: "mouse" alone isn't in "mouseUp".
        let whole = FindOptions { whole_word: true, ..opts("mouse") };
        assert_eq!(find(text, 0, &whole), Some((17, 22)));
        // Wraparound starts again at the top; without it, the end is the end.
        let last = text.rfind("mouseUp").unwrap();
        assert_eq!(find(text, last + 1, &opts("mouseup")), Some((3, 10)));
        let no_wrap = FindOptions { wraparound: false, ..opts("mouseup") };
        assert_eq!(find(text, last + 1, &no_wrap), None);
        assert_eq!(find(text, 0, &opts("")), None);
        // Character indexes, not bytes.
        assert_eq!(find("¬ é put", 0, &opts("put")), Some((4, 7)));
    }

    #[test]
    fn replacing() {
        let o = FindOptions { find: "x".into(), replace: "count".into(), whole_word: true, ..Default::default() };
        let text = "put x into x\nadd 1 to x2";
        assert_eq!(replace_all(text, &o), ("put count into count\nadd 1 to x2".to_string(), 2));
        let (once, at) = replace_next(text, 5, &o).unwrap();
        assert_eq!((once.as_str(), at), ("put x into count\nadd 1 to x2", (11, 16)));
        assert_eq!(replace_next("nothing", 0, &o), None);
    }

    #[test]
    fn lines() {
        let text = "on a\n  b\nend a";
        assert_eq!(line_range(text, 2), (5, 8));
        assert_eq!(line_range(text, 3), (9, 14));
        assert_eq!(line_range(text, 9), (14, 14));
        assert_eq!(line_of(text, 6), 2);
        assert_eq!(slice(text, (5, 8)), "  b");
        assert_eq!(command_for("G"), Some(EditorCommand::FindAgain));
        assert_eq!(command_for("q"), None);
    }
}
