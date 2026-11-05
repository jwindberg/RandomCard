//! JSON as HyperTalk reads it: everything is text. A string reads as its
//! plain text, a number or true/false as written, null as empty, and an
//! object or array as JSON text, ready to read again.

use serde_json::Value;

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

/// Follow keys from the outside in, as text. A key that isn't there, or a
/// key of something that isn't an object, reads as empty.
pub fn get(source: &str, keys: &[String]) -> Result<String, String> {
    let v = parse(source)?;
    let mut cur = &v;
    for k in keys {
        match cur.get(k.as_str()) {
            Some(next) => cur = next,
            None => return Ok(String::new()),
        }
    }
    Ok(text(cur))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading() {
        let joke = r#"{"id":"abc","value":"Chuck counted to infinity. Twice.","categories":["dev"],"n":3,"ok":true,"none":null,"owner":{"name":"Ada"}}"#;
        let get = |keys: &[&str]| get(joke, &keys.iter().map(|k| k.to_string()).collect::<Vec<_>>()).unwrap();
        assert_eq!(get(&["value"]), "Chuck counted to infinity. Twice.");
        assert_eq!(get(&["n"]), "3");
        assert_eq!(get(&["ok"]), "true");
        assert_eq!(get(&["none"]), "");
        assert_eq!(get(&["missing"]), "");
        assert_eq!(get(&["categories"]), r#"["dev"]"#);
        assert_eq!(get(&["owner", "name"]), "Ada");
        assert_eq!(get(&["value", "deeper"]), "");
        // No keys: the whole thing, as JSON.
        assert_eq!(get(&[]), text(&parse(joke).unwrap()));
        assert!(super::get("not json", &[]).unwrap_err().starts_with("Expected JSON"));
        assert_eq!(super::get("", &["x".into()]).unwrap(), "");
    }
}
