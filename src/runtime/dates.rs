//! Dates and times in HyperTalk formats. Like HyperCard, `the seconds` counts
//! from midnight, January 1, 1904 (local time).

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

use crate::hypertalk::ast::NameLength;

fn epoch() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(1904, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()
}

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

pub fn to_seconds(dt: NaiveDateTime) -> i64 {
    (dt - epoch()).num_seconds()
}

pub fn from_seconds(s: i64) -> NaiveDateTime {
    epoch() + chrono::Duration::seconds(s)
}

pub fn format_date(dt: NaiveDateTime, len: NameLength) -> String {
    match len {
        NameLength::Long => dt.format("%A, %B %-d, %Y").to_string(),
        NameLength::Abbrev => dt.format("%a, %b %-d, %Y").to_string(),
        _ => dt.format("%-m/%-d/%y").to_string(),
    }
}

pub fn format_time(dt: NaiveDateTime, len: NameLength) -> String {
    match len {
        NameLength::Long => dt.format("%-I:%M:%S %p").to_string(),
        _ => dt.format("%-I:%M %p").to_string(),
    }
}

pub fn date_items(dt: NaiveDateTime) -> String {
    format!(
        "{},{},{},{},{},{},{}",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.weekday().number_from_sunday()
    )
}

fn parse_time_part(s: &str) -> Option<NaiveTime> {
    let s = s.trim().to_uppercase();
    for f in ["%I:%M:%S %p", "%I:%M %p", "%H:%M:%S", "%H:%M"] {
        if let Ok(t) = NaiveTime::parse_from_str(&s, f) {
            return Some(t);
        }
    }
    None
}

fn parse_date_part(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    for f in ["%m/%d/%y", "%m/%d/%Y", "%A, %B %d, %Y", "%a, %b %d, %Y", "%B %d, %Y", "%b %d, %Y", "%Y-%m-%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return Some(d);
        }
    }
    None
}

/// Parse any date/time representation HyperTalk scripts commonly use.
pub fn parse_date(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<i64>() {
        return Some(from_seconds(n));
    }
    let items: Vec<&str> = s.split(',').map(str::trim).collect();
    if items.len() == 7 && items.iter().all(|i| i.parse::<i64>().is_ok()) {
        let n: Vec<i64> = items.iter().map(|i| i.parse().unwrap()).collect();
        let date = NaiveDate::from_ymd_opt(n[0] as i32, 1, 1)?;
        // dateItems may overflow (e.g. month 13); normalize like HyperCard.
        let dt = date.and_hms_opt(0, 0, 0)?;
        let months = n[1] - 1;
        let dt = dt.checked_add_months(chrono::Months::new(months.max(0) as u32))?;
        let dt = dt + chrono::Duration::days(n[2] - 1)
            + chrono::Duration::hours(n[3])
            + chrono::Duration::minutes(n[4])
            + chrono::Duration::seconds(n[5]);
        return Some(dt);
    }
    if let Some(d) = parse_date_part(s) {
        return d.and_hms_opt(0, 0, 0);
    }
    if let Some(t) = parse_time_part(s) {
        return Some(now().date().and_time(t));
    }
    // "date time" combinations: try every split point.
    let words: Vec<&str> = s.split(' ').collect();
    for i in 1..words.len() {
        let (a, b) = (words[..i].join(" "), words[i..].join(" "));
        if let (Some(d), Some(t)) = (parse_date_part(&a), parse_time_part(&b)) {
            return Some(d.and_time(t));
        }
        if let (Some(t), Some(d)) = (parse_time_part(&a), parse_date_part(&b)) {
            return Some(d.and_time(t));
        }
    }
    None
}

/// Format for `convert x to <format>`, e.g. "seconds", "long date", "short date and long time".
pub fn convert(dt: NaiveDateTime, format: &str) -> Option<String> {
    let mut out = Vec::new();
    for part in format.split(" and ") {
        let words: Vec<&str> = part.split_whitespace().collect();
        let (len, kind) = match words.as_slice() {
            [k] => (NameLength::Default, *k),
            [l, k] => (
                match *l {
                    "long" | "english" => NameLength::Long,
                    "abbreviated" | "abbrev" | "abbr" => NameLength::Abbrev,
                    _ => NameLength::Short,
                },
                *k,
            ),
            _ => return None,
        };
        out.push(match kind {
            "seconds" | "secs" | "second" | "sec" => to_seconds(dt).to_string(),
            "dateitems" => date_items(dt),
            "date" => format_date(dt, len),
            "time" => format_time(dt, len),
            _ => return None,
        });
    }
    Some(out.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let dt = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap().and_hms_opt(13, 5, 9).unwrap();
        assert_eq!(format_date(dt, NameLength::Default), "10/23/26");
        assert_eq!(format_date(dt, NameLength::Long), "Friday, October 23, 2026");
        assert_eq!(format_time(dt, NameLength::Default), "1:05 PM");
        assert_eq!(parse_date(&to_seconds(dt).to_string()), Some(dt));
        assert_eq!(parse_date(&date_items(dt)), Some(dt));
        assert_eq!(parse_date("10/23/26").unwrap().date(), dt.date());
        assert_eq!(parse_date("Friday, October 23, 2026").unwrap().date(), dt.date());
        assert_eq!(convert(dt, "short date and long time").unwrap(), "10/23/26 1:05:09 PM");
    }
}
