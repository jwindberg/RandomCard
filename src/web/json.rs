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

/// A value from HyperTalk text: JSON if it already is JSON (`36`, `true`,
/// `{"a":1}`, `"42"` in quotes), and a string otherwise.
pub fn value_of(text: &str) -> Value {
    serde_json::from_str(text.trim()).unwrap_or_else(|_| Value::String(text.to_string()))
}

/// Set what steps lead to, making objects and arrays on the way, and give
/// back the whole as JSON text. An element just past the end appends one.
pub fn set(source: &str, steps: &[Step], value: Value) -> Result<String, String> {
    let mut root = parse(source)?;
    let mut cur = &mut root;
    for s in steps {
        cur = match s {
            Step::Key(k) => {
                if !cur.is_object() {
                    *cur = Value::Object(Default::default());
                }
                cur.as_object_mut().unwrap().entry(k.clone()).or_insert(Value::Null)
            }
            Step::Element(n) => {
                if !cur.is_array() {
                    *cur = Value::Array(Vec::new());
                }
                let items = cur.as_array_mut().unwrap();
                let at = if n.trim().eq_ignore_ascii_case("last") {
                    items.len().max(1)
                } else {
                    n.trim().parse::<usize>().ok().filter(|i| *i >= 1).ok_or_else(|| format!("Expected an element number, not \"{n}\""))?
                };
                if at > items.len() + 1 {
                    return Err(format!("There's no element {} to set: the array has {}", at, items.len()));
                }
                if at == items.len() + 1 {
                    items.push(Value::Null);
                }
                &mut items[at - 1]
            }
        };
    }
    *cur = value;
    Ok(root.to_string())
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
        // Numbers keep their exact text (RFC 8259 §6), however long.
        let n = r#"{"big":12345678901234567890123,"price":1.10,"e":1e400}"#;
        assert_eq!(super::get(n, &steps(&["big"])).unwrap(), "12345678901234567890123");
        assert_eq!(super::get(n, &steps(&["price"])).unwrap(), "1.10");
        // Beyond any float: kept exactly, its exponent spelled with a sign.
        assert_eq!(super::get(n, &steps(&["e"])).unwrap(), "1e+400");
    }

    #[test]
    fn writing() {
        let k = |s: &str| Step::Key(s.into());
        let e = |s: &str| Step::Element(s.into());
        let body = set("{}", &[k("name")], value_of("Ada")).unwrap();
        let body = set(&body, &[k("age")], value_of("36")).unwrap();
        let body = set(&body, &[k("tags"), e("1")], value_of("math")).unwrap();
        let body = set(&body, &[k("tags"), e("2")], value_of("poetry")).unwrap();
        let body = set(&body, &[k("id")], value_of("\"42\"")).unwrap();
        let body = set(&body, &[k("owner"), k("ok")], value_of("true")).unwrap();
        assert_eq!(body, r#"{"name":"Ada","age":36,"tags":["math","poetry"],"id":"42","owner":{"ok":true}}"#);
        // Changing keeps the order; an empty start becomes what's set.
        assert_eq!(set(&body, &[k("name")], value_of("Grace")).unwrap().find("Grace"), Some(9));
        assert_eq!(set("", &[e("1")], value_of("x")).unwrap(), r#"["x"]"#);
        assert_eq!(set("[1,2]", &[e("last")], value_of("9")).unwrap(), "[1,9]");
        assert!(set("[1]", &[e("5")], value_of("x")).unwrap_err().contains("no element 5"));
        assert_eq!(value_of("{\"a\":1}"), serde_json::json!({"a": 1}));
        assert_eq!(value_of(""), Value::String(String::new()));
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
