//! In HyperTalk every value is text; numbers and booleans are just text that
//! happens to look like a number or "true"/"false".

pub fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    // Reject things Rust accepts but HyperTalk doesn't, like "inf" or "nan".
    if !t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')) {
        return None;
    }
    t.parse::<f64>().ok()
}

pub fn parse_int(s: &str) -> Option<i64> {
    let n = parse_number(s)?;
    if n.fract() == 0.0 { Some(n as i64) } else { None }
}

pub fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn bool_str(b: bool) -> String {
    if b { "true".into() } else { "false".into() }
}

/// Format a number using a HyperTalk numberFormat such as "0.######".
pub fn format_number(n: f64, number_format: &str) -> String {
    if n.is_nan() {
        return "NAN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "INF".into() } else { "-INF".into() };
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    let (int_part, frac_part) = match number_format.split_once('.') {
        Some((i, f)) => (i, f),
        None => (number_format, ""),
    };
    let min_int = int_part.chars().filter(|c| *c == '0').count();
    let min_frac = frac_part.chars().filter(|c| *c == '0').count();
    let max_frac = frac_part.chars().filter(|c| *c == '0' || *c == '#').count();
    let mut s = format!("{:.*}", max_frac, n);
    if s.contains('.') {
        // Trim optional trailing digits.
        let dot = s.find('.').unwrap();
        while s.len() > dot + 1 + min_frac && s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    // Pad integer digits.
    let neg = s.starts_with('-');
    let body = s.trim_start_matches('-');
    let int_len = body.find('.').unwrap_or(body.len());
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    for _ in int_len..min_int {
        out.push('0');
    }
    if min_int == 0 && body.starts_with("0.") {
        out.push_str(&body[1..]);
    } else {
        out.push_str(body);
    }
    if out == "-0" {
        out = "0".into();
    }
    out
}

/// Split into (start, end) byte ranges for each word: runs of non-whitespace.
pub fn word_ranges(s: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            if let Some(st) = start.take() {
                out.push((st, i));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        out.push((st, s.len()));
    }
    out
}

/// Ranges of delimited pieces (items or lines). An empty string has no pieces,
/// and a trailing delimiter does not start a new piece.
pub fn delimited_ranges(s: &str, delim: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if s.is_empty() || delim.is_empty() {
        if !s.is_empty() {
            out.push((0, s.len()));
        }
        return out;
    }
    let mut start = 0;
    while let Some(off) = s[start..].find(delim) {
        out.push((start, start + off));
        start += off + delim.len();
    }
    if start < s.len() {
        out.push((start, s.len()));
    }
    out
}

pub fn char_ranges(s: &str) -> Vec<(usize, usize)> {
    s.char_indices().map(|(i, c)| (i, i + c.len_utf8())).collect()
}

pub fn count_chunks(s: &str, kind: super::ast::ChunkKind, item_delim: &str) -> usize {
    use super::ast::ChunkKind::*;
    match kind {
        Char => s.chars().count(),
        Word => word_ranges(s).len(),
        Item => delimited_ranges(s, item_delim).len(),
        Line => delimited_ranges(s, "\n").len(),
    }
}

/// Round half to even, as HyperCard (via SANE) does.
pub fn round_half_even(n: f64) -> f64 {
    let r = n.round();
    if (n - n.trunc()).abs() == 0.5 && r % 2.0 != 0.0 { r - n.signum() } else { r }
}

/// Compare two values the way HyperTalk does: numerically if both are numbers,
/// otherwise as case-insensitive text.
pub fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_number(a), parse_number(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

pub fn equals(a: &str, b: &str) -> bool {
    match (parse_number(a), parse_number(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a.trim_end().eq_ignore_ascii_case(b.trim_end()) || a.to_lowercase() == b.to_lowercase(),
    }
}

pub fn parse_point(s: &str) -> Option<(i32, i32)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parse_int(parts[0])? as i32, parse_int(parts[1])? as i32))
}

pub fn parse_rect(s: &str) -> Option<(i32, i32, i32, i32)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    Some((
        parse_int(parts[0])? as i32,
        parse_int(parts[1])? as i32,
        parse_int(parts[2])? as i32,
        parse_int(parts[3])? as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_number(3.0, "0.######"), "3");
        assert_eq!(format_number(0.5, "0.######"), "0.5");
        assert_eq!(format_number(1.0 / 3.0, "0.######"), "0.333333");
        assert_eq!(format_number(2.5, "0.00"), "2.50");
        assert_eq!(format_number(-0.25, "#.##"), "-.25");
        assert_eq!(format_number(7.1, "00.0"), "07.1");
    }

    #[test]
    fn ranges() {
        assert_eq!(delimited_ranges("a,b,,c", ",").len(), 4);
        assert_eq!(delimited_ranges("a\nb\n", "\n").len(), 2);
        assert_eq!(word_ranges("  hello   big world ").len(), 3);
        assert_eq!(round_half_even(2.5), 2.0);
        assert_eq!(round_half_even(3.5), 4.0);
        assert_eq!(round_half_even(-2.5), -2.0);
        assert_eq!(round_half_even(2.6), 3.0);
    }
}
