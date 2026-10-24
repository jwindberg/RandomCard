//! Laying out field text by HyperCard's metrics: where lines break, how
//! tall they are, and where each word goes, as HyperCard placed them.

use crate::model::metrics::{self, Metrics};
use crate::model::styles::{self, StyleRun};
use crate::model::TextAlign;

/// A run of text in one style, placed on a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub x: f32,
    pub text: String,
    pub font: String,
    pub size: u32,
    pub style: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub top: f32,
    pub baseline: f32,
    pub height: f32,
    pub pieces: Vec<Piece>,
}

/// How text is set: its default font, size and style, alignment and lines.
pub struct Setting<'a> {
    pub font: &'a str,
    pub size: u32,
    pub style: &'a str,
    pub align: TextAlign,
    pub wrap: bool,
    /// Every line this high (fixedLineHeight), or by its fonts if `None`.
    pub line_height: Option<u32>,
}

struct Char {
    c: char,
    font: String,
    size: u32,
    style: String,
    m: Metrics,
}

/// Lay out `text` with its style runs in a column `width` wide.
pub fn layout(text: &str, runs: &[StyleRun], s: &Setting, width: f32) -> Vec<Line> {
    let chars: Vec<Char> = text
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let run = styles::style_at(runs, i);
            let font = run.font.unwrap_or_else(|| s.font.to_string());
            let size = run.size.unwrap_or(s.size);
            let style = run.style.unwrap_or_else(|| s.style.to_string());
            let m = metrics::metrics(&font, size, &style);
            Char { c, font, size, style, m }
        })
        .collect();
    let base = metrics::metrics(s.font, s.size, s.style);
    let width_of = |range: std::ops::Range<usize>| -> f32 { chars[range].iter().map(|c| c.m.width(c.c)).sum() };

    // Break into lines: ranges of characters, without their newline.
    let mut breaks: Vec<std::ops::Range<usize>> = Vec::new();
    let mut start = 0;
    while start <= chars.len() {
        let end = chars[start..].iter().position(|c| c.c == '\n').map_or(chars.len(), |p| start + p);
        let mut line_start = start;
        if s.wrap {
            let mut x = 0.0;
            let mut i = start;
            while i < end {
                // A word and the spaces after it.
                let word_end = (i..end).find(|&k| chars[k].c == ' ').unwrap_or(end);
                let next = (word_end..end).find(|&k| chars[k].c != ' ').unwrap_or(end);
                let w = width_of(i..word_end);
                if x + w > width && i > line_start {
                    breaks.push(line_start..i);
                    line_start = i;
                    x = 0.0;
                    continue;
                }
                if w > width && i == line_start {
                    // A word wider than the column breaks where it must.
                    let mut k = i;
                    let mut wx = 0.0;
                    while k < word_end && (k == i || wx + chars[k].m.width(chars[k].c) <= width) {
                        wx += chars[k].m.width(chars[k].c);
                        k += 1;
                    }
                    if k < word_end {
                        breaks.push(line_start..k);
                        line_start = k;
                        i = k;
                        x = 0.0;
                        continue;
                    }
                }
                x += width_of(i..next);
                i = next;
            }
        }
        breaks.push(line_start..end);
        start = end + 1;
    }

    let mut lines = Vec::with_capacity(breaks.len());
    let mut top = 0.0;
    for range in breaks {
        let line_chars = &chars[range.clone()];
        let (ascent, height) = match s.line_height {
            Some(h) => {
                let h = h as f32;
                (h - (h / 4.0).floor(), h)
            }
            None if line_chars.is_empty() => {
                // An empty line takes the style of where it is.
                let m = chars.get(range.start).map_or(base, |c| c.m);
                (m.ascent as f32, m.line_height() as f32)
            }
            None => {
                let ascent = line_chars.iter().map(|c| c.m.ascent).max().unwrap_or(0) as f32;
                let descent = line_chars.iter().map(|c| c.m.descent).max().unwrap_or(0) as f32;
                let leading = line_chars.iter().map(|c| c.m.leading).min().unwrap_or(0) as f32;
                (ascent, ascent + descent + leading)
            }
        };
        let trimmed = line_chars.iter().rposition(|c| c.c != ' ').map_or(0, |p| p + 1);
        let line_width: f32 = line_chars[..trimmed].iter().map(|c| c.m.width(c.c)).sum();
        let mut x = match s.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((width - line_width) / 2.0).floor(),
            TextAlign::Right => width - line_width,
        };
        let mut pieces: Vec<Piece> = Vec::new();
        for ch in line_chars {
            let same = pieces.last().is_some_and(|p| p.font == ch.font && p.size == ch.size && p.style == ch.style);
            // Each word is placed where HyperCard put it.
            if same && ch.c != ' ' && !pieces.last().unwrap().text.ends_with(' ') {
                pieces.last_mut().unwrap().text.push(ch.c);
            } else if ch.c == ' ' && same {
                pieces.last_mut().unwrap().text.push(' ');
            } else {
                pieces.push(Piece { x, text: ch.c.to_string(), font: ch.font.clone(), size: ch.size, style: ch.style.clone() });
            }
            x += ch.m.width(ch.c);
        }
        pieces.retain(|p| !p.text.trim().is_empty());
        lines.push(Line { top, baseline: top + ascent, height, pieces });
        top += height;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(width: f32, text: &str) -> Vec<Line> {
        let s = Setting { font: "Geneva", size: 12, style: "plain", align: TextAlign::Left, wrap: true, line_height: None };
        layout(text, &[], &s, width)
    }

    #[test]
    fn wraps_by_hypercard_widths() {
        let m = metrics::metrics("Geneva", 12, "plain");
        let w = m.text_width("hello");
        let lines = plain(w + m.width(' ') + 2.0, "hello there world");
        assert_eq!(lines.len(), 3);
        assert_eq!((lines[0].height, lines[0].baseline, lines[1].top), (16.0, 12.0, 16.0));
        let words: Vec<&str> = lines.iter().flat_map(|l| l.pieces.iter().map(|p| p.text.trim())).collect();
        assert_eq!(words, vec!["hello", "there", "world"]);
        // Newlines make lines, and an empty one still has height.
        assert_eq!(plain(500.0, "a\n\nb").len(), 3);
        // A word wider than the column breaks inside.
        assert!(plain(20.0, "abcdefghij").len() > 1);
        // Words are placed by HyperCard's widths.
        let line = &plain(500.0, "ab cd")[0];
        assert_eq!(line.pieces[1].x, m.text_width("ab "));
    }
}
