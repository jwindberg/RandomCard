//! Chunk expressions: `char 3 to 5 of word 2 of line 1 of x`.

use super::ast::{ChunkKind, Prep};
use super::value::{char_ranges, word_ranges};

#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub kind: ChunkKind,
    /// 1-based, inclusive.
    pub start: i64,
    pub end: i64,
}

/// Every delimited segment, including a trailing empty one, as byte ranges.
fn split_ranges(s: &str, delim: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    if !delim.is_empty() {
        while let Some(off) = s[start..].find(delim) {
            out.push((start, start + off));
            start += off + delim.len();
        }
    }
    out.push((start, s.len()));
    out
}

fn delim_for(kind: ChunkKind, item_delim: &str) -> &str {
    match kind {
        ChunkKind::Line => "\n",
        _ => item_delim,
    }
}

/// The byte range of a chunk within `s[base.0..base.1]`. When `pad` is set,
/// missing items and lines are created by inserting delimiters into `s`.
fn range_in(s: &mut String, base: (usize, usize), c: &Resolved, item_delim: &str, pad: bool) -> (usize, usize) {
    let sub = &s[base.0..base.1];
    let ranges: Vec<(usize, usize)> = match c.kind {
        ChunkKind::Char => char_ranges(sub),
        ChunkKind::Word => word_ranges(sub),
        ChunkKind::Item | ChunkKind::Line => {
            let delim = delim_for(c.kind, item_delim).to_string();
            let mut r = split_ranges(sub, &delim);
            let want = c.end.max(c.start).max(1) as usize;
            if pad && r.len() < want {
                let insert = delim.repeat(want - r.len());
                s.insert_str(base.1, &insert);
                let sub = &s[base.0..base.1 + insert.len()];
                r = split_ranges(sub, &delim);
            }
            r
        }
    };
    let sub_len = match c.kind {
        ChunkKind::Char | ChunkKind::Word => base.1 - base.0,
        // Item and line ranges always end at the (possibly padded) end of the base.
        _ => ranges.last().map(|r| r.1).unwrap_or(0),
    };
    if ranges.is_empty() {
        return (base.0, base.0);
    }
    let count = ranges.len() as i64;
    let start = c.start.max(1);
    let end = c.end.max(start).min(count);
    if start > count {
        // Past the end: an insertion point at the end of the base range.
        return (base.0 + sub_len, base.0 + sub_len);
    }
    (base.0 + ranges[(start - 1) as usize].0, base.0 + ranges[(end - 1) as usize].1)
}

/// Locate nested chunks, listed outermost first (as written left to right).
pub fn locate(s: &mut String, chunks: &[Resolved], item_delim: &str, pad: bool) -> (usize, usize) {
    let mut range = (0, s.len());
    for c in chunks.iter().rev() {
        if c.start < 1 && c.end < 1 {
            return (range.0, range.0);
        }
        range = range_in(s, range, c, item_delim, pad);
    }
    range
}

pub fn get(s: &str, chunks: &[Resolved], item_delim: &str) -> String {
    let mut s = s.to_string();
    let (a, b) = locate(&mut s, chunks, item_delim, false);
    // A chunk past the end is empty.
    if a == b {
        return String::new();
    }
    s[a..b].to_string()
}

pub fn put(s: &mut String, chunks: &[Resolved], item_delim: &str, prep: Prep, value: &str) {
    let (a, b) = locate(s, chunks, item_delim, true);
    match prep {
        Prep::Into => s.replace_range(a..b, value),
        Prep::Before => s.insert_str(a, value),
        Prep::After => s.insert_str(b, value),
    }
}

pub fn delete(s: &mut String, chunks: &[Resolved], item_delim: &str) {
    let (a, mut b) = locate(s, chunks, item_delim, false);
    if a == b && chunks.first().is_some_and(|c| c.kind != ChunkKind::Char) {
        return;
    }
    let Some(c) = chunks.first() else {
        s.clear();
        return;
    };
    let mut a = a;
    match c.kind {
        ChunkKind::Char => {}
        ChunkKind::Word => {
            // Remove following whitespace too.
            while let Some(ch) = s[b..].chars().next() {
                if ch == ' ' || ch == '\t' {
                    b += ch.len_utf8();
                } else {
                    break;
                }
            }
        }
        ChunkKind::Item | ChunkKind::Line => {
            let delim = delim_for(c.kind, item_delim);
            if s[b..].starts_with(delim) {
                b += delim.len();
            } else if a >= delim.len() && s[..a].ends_with(delim) {
                a -= delim.len();
            }
        }
    }
    s.replace_range(a..b, "");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(kind: ChunkKind, start: i64, end: i64) -> Resolved {
        Resolved { kind, start, end }
    }

    #[test]
    fn get_chunks() {
        let s = "one two three\nfour,five,six\nseven";
        assert_eq!(get(s, &[r(ChunkKind::Word, 2, 2)], ","), "two");
        assert_eq!(get(s, &[r(ChunkKind::Item, 2, 2), r(ChunkKind::Line, 2, 2)], ","), "five");
        assert_eq!(get(s, &[r(ChunkKind::Char, 1, 3), r(ChunkKind::Line, 3, 3)], ","), "sev");
        assert_eq!(get(s, &[r(ChunkKind::Line, 5, 5)], ","), "");
        assert_eq!(get(s, &[r(ChunkKind::Word, 2, 3)], ","), "two three");
    }

    #[test]
    fn put_chunks() {
        let mut s = String::from("a,b,c");
        put(&mut s, &[r(ChunkKind::Item, 2, 2)], ",", Prep::Into, "X");
        assert_eq!(s, "a,X,c");
        put(&mut s, &[r(ChunkKind::Item, 5, 5)], ",", Prep::Into, "E");
        assert_eq!(s, "a,X,c,,E");
        let mut s = String::new();
        put(&mut s, &[r(ChunkKind::Line, 3, 3)], ",", Prep::Into, "third");
        assert_eq!(s, "\n\nthird");
        let mut s = String::from("hello world");
        put(&mut s, &[r(ChunkKind::Char, 1, 1), r(ChunkKind::Word, 2, 2)], ",", Prep::Into, "W");
        assert_eq!(s, "hello World");
        put(&mut s, &[r(ChunkKind::Word, 1, 1)], ",", Prep::After, ",");
        assert_eq!(s, "hello, World");
    }

    #[test]
    fn delete_chunks() {
        let mut s = String::from("a,b,c");
        delete(&mut s, &[r(ChunkKind::Item, 2, 2)], ",");
        assert_eq!(s, "a,c");
        delete(&mut s, &[r(ChunkKind::Item, 2, 2)], ",");
        assert_eq!(s, "a");
        let mut s = String::from("one two three");
        delete(&mut s, &[r(ChunkKind::Word, 1, 1)], ",");
        assert_eq!(s, "two three");
        let mut s = String::from("abc");
        delete(&mut s, &[r(ChunkKind::Char, 2, 2)], ",");
        assert_eq!(s, "ac");
    }
}
