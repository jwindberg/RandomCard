//! JSON as HyperTalk reads it: everything is text. A string reads as its
//! plain text, a number or true/false as written, null as empty, and an
//! object or array as JSON text, ready to read again. Objects keep their
//! keys in the order they came.

use serde_json::Value;

/// One step into a JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// An object's key.
    Key(String),
    /// An array's element, from 1, or "last".
    Element(String),
}

/// The text a JSON value reads as.
pub fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// JSON from text; empty text is null. The error quotes the start of it.
pub fn parse(source: &str) -> Result<Value, String> {
    let t = source.trim();
    if t.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(t).map_err(|e| {
        let start: String = t.chars().take(30).collect();
        format!("Expected JSON but found \"{start}\" ({e})")
    })
}

fn element<'a>(v: &'a Value, which: &str) -> Option<&'a Value> {
    let items = v.as_array()?;
    let w = which.trim();
    if w.eq_ignore_ascii_case("last") {
        return items.last();
    }
    let n: usize = w.parse().ok()?;
    items.get(n.checked_sub(1)?)
}

/// Follow steps from the outside in. A key or element that isn't there,
/// or one asked of the wrong kind of value, is null.
#[cfg(test)]
pub fn follow(source: &str, steps: &[Step]) -> Result<Value, String> {
    Ok(walk(&parse(source)?, steps))
}

/// The same, from a value already parsed.
pub fn walk(v: &Value, steps: &[Step]) -> Value {
    let mut cur = v;
    for s in steps {
        let next = match s {
            Step::Key(k) => cur.get(k.as_str()),
            Step::Element(n) => element(cur, n),
        };
        match next {
            Some(n) => cur = n,
            None => return Value::Null,
        }
    }
    cur.clone()
}

/// What steps lead to, as text.
#[cfg(test)]
pub fn get(source: &str, steps: &[Step]) -> Result<String, String> {
    follow(source, steps).map(|v| text(&v))
}

/// An object's keys, or an array's elements as text, one a line.
pub fn keys(v: &Value) -> String {
    match v {
        Value::Object(m) => m.keys().cloned().collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

pub fn elements(v: &Value) -> String {
    match v {
        Value::Array(a) => a.iter().map(text).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// How many elements an array has, or keys an object; 0 for anything else.
pub fn count(v: &Value) -> usize {
    match v {
        Value::Array(a) => a.len(),
        Value::Object(m) => m.len(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(s: &[&str]) -> Vec<Step> {
        // "#2" is element 2, anything else a key.
        s.iter()
            .map(|s| match s.strip_prefix('#') {
                Some(n) => Step::Element(n.into()),
                None => Step::Key(s.to_string()),
            })
            .collect()
    }

    #[test]
    fn reading() {
        let joke = r#"{"id":"abc","value":"Chuck counted to infinity. Twice.","categories":["dev"],"n":3,"ok":true,"none":null,"owner":{"name":"Ada"}}"#;
        let get = |s: &[&str]| get(joke, &steps(s)).unwrap();
        assert_eq!(get(&["value"]), "Chuck counted to infinity. Twice.");
        assert_eq!(get(&["n"]), "3");
        assert_eq!(get(&["ok"]), "true");
        assert_eq!(get(&["none"]), "");
        assert_eq!(get(&["missing"]), "");
        assert_eq!(get(&["categories"]), r#"["dev"]"#);
        assert_eq!(get(&["owner", "name"]), "Ada");
        assert_eq!(get(&["value", "deeper"]), "");
        // No steps: the whole thing, as JSON.
        assert_eq!(get(&[]), text(&parse(joke).unwrap()));
        assert!(super::get("not json", &[]).unwrap_err().starts_with("Expected JSON"));
        assert_eq!(super::get("", &steps(&["x"])).unwrap(), "");
    }

    #[test]
    fn arrays() {
        let found = r#"{"total":2,"result":[{"id":"a","value":"one"},{"id":"b","value":"two"}],"tags":["x","y","z"]}"#;
        let get = |s: &[&str]| get(found, &steps(s)).unwrap();
        assert_eq!(get(&["result", "#2", "value"]), "two");
        assert_eq!(get(&["tags", "#1"]), "x");
        assert_eq!(get(&["tags", "#last"]), "z");
        assert_eq!(get(&["tags", "#0"]), "");
        assert_eq!(get(&["tags", "#9"]), "");
        assert_eq!(get(&["total", "#1"]), "");
        let v = parse(found).unwrap();
        // Keys in the order they came, not sorted.
        assert_eq!(keys(&v), "total\nresult\ntags");
        assert_eq!(elements(&follow(found, &steps(&["tags"])).unwrap()), "x\ny\nz");
        assert_eq!(count(&follow(found, &steps(&["result"])).unwrap()), 2);
        assert_eq!(count(&v), 3);
        assert_eq!(count(&follow(found, &steps(&["total"])).unwrap()), 0);
    }
}
