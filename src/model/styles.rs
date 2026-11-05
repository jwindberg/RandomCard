//! Styled text: runs of font, size and style within a field's text.
//!
//! A field's text has a list of runs, each starting at a character offset
//! and lasting until the next. Text before the first run, and any setting a
//! run leaves out, takes the field's own textFont, textSize and textStyle.

use serde::{Deserialize, Serialize};

/// Font, size and style for a run of text; `None` takes the field's own.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// Comma-separated, as HyperTalk writes it: "bold,italic", or "plain".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The text's color (`the foreColor` of a chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<super::color::Rgb>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleRun {
    /// Where the run starts, in characters.
    pub start: usize,
    #[serde(flatten)]
    pub style: TextStyle,
}

/// The fonts in the Font menu: HyperCard's usual Macintosh fonts.
pub const FONTS: &[&str] = &["Chicago", "Courier", "Geneva", "Helvetica", "Monaco", "New York", "Palatino", "Times"];

/// The text sizes in the Style menu.
pub const SIZES: &[u32] = &[9, 10, 12, 14, 18, 24];

/// HyperCard's text styles, in its menu order.
pub const STYLES: &[&str] = &["bold", "italic", "underline", "outline", "shadow", "condense", "extend", "group"];

/// Tidy a textStyle value: "Bold, Italic" → "bold,italic"; nothing → "plain".
pub fn normalize_style(s: &str) -> String {
    let words: Vec<String> = s
        .split(',')
        .map(|w| w.trim().to_ascii_lowercase())
        .filter(|w| !w.is_empty() && w != "plain")
        .collect();
    let ordered: Vec<&str> = STYLES.iter().copied().filter(|st| words.iter().any(|w| w == st)).collect();
    if ordered.is_empty() { "plain".into() } else { ordered.join(",") }
}

pub fn has_style(style: &str, which: &str) -> bool {
    style.split(',').any(|w| w.trim().eq_ignore_ascii_case(which))
}

/// Add or remove one style ("bold") from a textStyle value.
pub fn with_style(style: &str, which: &str, on: bool) -> String {
    let mut words: Vec<&str> = style.split(',').map(str::trim).filter(|w| !w.eq_ignore_ascii_case(which)).collect();
    if on {
        words.push(which);
    }
    normalize_style(&words.join(","))
}

/// The style in effect at character `at`.
pub fn style_at(runs: &[StyleRun], at: usize) -> TextStyle {
    runs.iter().take_while(|r| r.start <= at).last().map(|r| r.style.clone()).unwrap_or_default()
}

/// Merge runs that repeat the style before them and drop runs past `len`.
pub fn normalize(runs: &mut Vec<StyleRun>, len: usize) {
    runs.sort_by_key(|r| r.start);
    let mut out: Vec<StyleRun> = Vec::with_capacity(runs.len());
    for r in runs.drain(..) {
        if r.start >= len {
            break;
        }
        // A later run at the same place wins.
        if out.last().is_some_and(|l| l.start == r.start) {
            out.pop();
        }
        let before = out.last().map(|l| l.style.clone()).unwrap_or_default();
        if r.style != before {
            out.push(r);
        }
    }
    *runs = out;
}

/// Keep runs in step with an edit: `removed` characters at `at` replaced by
/// `inserted` ones. New text takes the style of the first character it
/// replaces, or, when nothing is replaced, of the character before it.
pub fn edit(runs: &mut Vec<StyleRun>, at: usize, removed: usize, inserted: usize, new_len: usize) {
    if runs.is_empty() {
        return;
    }
    let inherit = if removed > 0 || at == 0 { style_at(runs, at) } else { style_at(runs, at - 1) };
    let after = style_at(runs, at + removed);
    let mut out: Vec<StyleRun> = Vec::with_capacity(runs.len() + 2);
    for r in runs.iter() {
        if r.start < at {
            out.push(r.clone());
        } else if r.start > at + removed {
            out.push(StyleRun { start: r.start - removed + inserted, style: r.style.clone() });
        }
    }
    out.push(StyleRun { start: at, style: inherit });
    out.push(StyleRun { start: at + inserted, style: after });
    *runs = out;
    normalize(runs, new_len);
}

/// Keep runs in step with a change of the whole text, by finding the part
/// that changed.
pub fn retext(runs: &mut Vec<StyleRun>, old: &str, new: &str) {
    if runs.is_empty() || old == new {
        return;
    }
    let (o, n): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let prefix = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let max_suffix = o.len().min(n.len()) - prefix;
    let suffix = o.iter().rev().zip(n.iter().rev()).take(max_suffix).take_while(|(a, b)| a == b).count();
    edit(runs, prefix, o.len() - prefix - suffix, n.len() - prefix - suffix, n.len());
}

/// Change the style of characters `start..end`.
pub fn apply(runs: &mut Vec<StyleRun>, start: usize, end: usize, len: usize, change: impl Fn(&mut TextStyle)) {
    if start >= end {
        return;
    }
    let after = style_at(runs, end);
    let mut out: Vec<StyleRun> = Vec::with_capacity(runs.len() + 2);
    out.extend(runs.iter().filter(|r| r.start < start).cloned());
    let mut first = style_at(runs, start);
    change(&mut first);
    out.push(StyleRun { start, style: first });
    for r in runs.iter().filter(|r| r.start > start && r.start < end) {
        let mut s = r.style.clone();
        change(&mut s);
        out.push(StyleRun { start: r.start, style: s });
    }
    out.push(StyleRun { start: end, style: after });
    out.extend(runs.iter().filter(|r| r.start > end).cloned());
    *runs = out;
    normalize(runs, len);
}

/// The run of grouped text around character `at`, if it's in one. Clicking
/// grouped text makes `the clickText` the whole group.
pub fn group_at(runs: &[StyleRun], field_style: &str, len: usize, at: usize) -> Option<(usize, usize)> {
    let grouped = |i: usize| has_style(style_at(runs, i).style.as_deref().unwrap_or(field_style), "group");
    if at >= len || !grouped(at) {
        return None;
    }
    let start = (0..at).rev().take_while(|&i| grouped(i)).last().unwrap_or(at);
    let end = (at..len).find(|&i| !grouped(i)).unwrap_or(len);
    Some((start, end))
}

/// The distinct styles over `start..end` (one style if the range is empty).
pub fn styles_in(runs: &[StyleRun], start: usize, end: usize) -> Vec<TextStyle> {
    let mut out = vec![style_at(runs, start)];
    for r in runs.iter().filter(|r| r.start > start && r.start < end) {
        if !out.contains(&r.style) {
            out.push(r.style.clone());
        }
    }
    out
}

/// The runs of characters `start..end`, for copying: they start from 0,
/// and each setting a run leaves to the field is filled in from `own`, so
/// the text keeps its look in another field.
pub fn copy_runs(runs: &[StyleRun], start: usize, end: usize, own: &TextStyle) -> Vec<StyleRun> {
    let filled = |s: &TextStyle| TextStyle {
        font: s.font.clone().or_else(|| own.font.clone()),
        size: s.size.or(own.size),
        style: s.style.clone().or_else(|| own.style.clone()),
        color: s.color.or(own.color),
    };
    let mut out = vec![StyleRun { start: 0, style: filled(&style_at(runs, start)) }];
    out.extend(runs.iter().filter(|r| r.start > start && r.start < end).map(|r| StyleRun {
        start: r.start - start,
        style: filled(&r.style),
    }));
    out
}

/// Give characters `at..at + clip_len` the copied runs.
pub fn paste_runs(runs: &mut Vec<StyleRun>, at: usize, clip: &[StyleRun], clip_len: usize, len: usize) {
    for (i, r) in clip.iter().enumerate() {
        let end = clip.get(i + 1).map_or(clip_len, |n| n.start);
        apply(runs, at + r.start, at + end, len, |s| *s = r.style.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copying_runs() {
        // "Hello world": "world" is bold, in a Geneva 12 field.
        let runs = vec![StyleRun { start: 6, style: bold() }];
        let own = TextStyle { font: Some("Geneva".into()), size: Some(12), style: Some("plain".into()), color: None };
        let clip = copy_runs(&runs, 4, 9, &own);
        let geneva = |style: &str| TextStyle { style: Some(style.into()), ..own.clone() };
        assert_eq!(
            clip,
            vec![StyleRun { start: 0, style: geneva("plain") }, StyleRun { start: 2, style: geneva("bold") }]
        );
        // Pasted into a plain field: "o wo" keeps its look there.
        let mut into = Vec::new();
        paste_runs(&mut into, 3, &clip, 5, 10);
        assert_eq!(style_at(&into, 2), TextStyle::default());
        assert_eq!(style_at(&into, 3), geneva("plain"));
        assert_eq!(style_at(&into, 5), geneva("bold"));
        assert_eq!(style_at(&into, 8), TextStyle::default());
    }

    fn bold() -> TextStyle {
        TextStyle { style: Some("bold".into()), ..Default::default() }
    }

    fn run(start: usize, style: TextStyle) -> StyleRun {
        StyleRun { start, style }
    }

    #[test]
    fn styles_follow_edits() {
        // "hello world" with "world" bold.
        let mut runs = vec![run(6, bold())];
        // Typing at the end of "world" stays bold; typing before it doesn't.
        retext(&mut runs, "hello world", "hello world!");
        assert_eq!(runs, vec![run(6, bold())]);
        retext(&mut runs, "hello world!", "hello, world!");
        assert_eq!(runs, vec![run(7, bold())]);
        // Replacing bold text keeps it bold.
        retext(&mut runs, "hello, world!", "hello, there!");
        assert_eq!(runs, vec![run(7, bold())]);
        // Deleting the bold text entirely leaves nothing styled after it.
        retext(&mut runs, "hello, there!", "hello, ");
        assert_eq!(runs, vec![]);
    }

    #[test]
    fn applying_styles() {
        let mut runs = Vec::new();
        apply(&mut runs, 2, 5, 10, |s| s.style = Some("bold".into()));
        assert_eq!(runs, vec![run(2, bold()), run(5, TextStyle::default())]);
        assert_eq!(styles_in(&runs, 2, 5), vec![bold()]);
        assert_eq!(styles_in(&runs, 0, 5).len(), 2);
        // Unbolding the same range removes the runs.
        apply(&mut runs, 2, 5, 10, |s| s.style = None);
        assert_eq!(runs, vec![]);
    }

    #[test]
    fn style_values() {
        assert_eq!(normalize_style("Italic, Bold"), "bold,italic");
        assert_eq!(normalize_style(""), "plain");
        assert_eq!(with_style("bold,italic", "bold", false), "italic");
        assert_eq!(with_style("plain", "underline", true), "underline");
        assert!(has_style("bold,italic", "italic"));
    }
}
