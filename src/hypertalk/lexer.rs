//! Tokenizer for HyperTalk. HyperTalk is case-insensitive and line-oriented:
//! newlines end statements, `--` starts a comment, and `¬` continues a line.

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// An identifier or keyword, lowercased.
    Word(String),
    Number(String),
    Str(String),
    /// Operators and punctuation.
    Sym(&'static str),
    Newline,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    /// The word as written, before lowercasing.
    pub text: String,
}

const SYMBOLS: &[&str] = &[
    "&&", "<=", ">=", "<>", "≤", "≥", "≠", "+", "-", "*", "/", "^", "&", "=", "<", ">", "(", ")", ",", ":", ";",
];

pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    let mut line = 1;
    while i < chars.len() {
        let c = chars[i];
        if c == '\r' || c == '\n' {
            // Collapse \r\n into one newline.
            if c == '\r' && chars.get(i + 1) == Some(&'\n') {
                i += 1;
            }
            toks.push(Token { tok: Tok::Newline, line, text: String::new() });
            line += 1;
            i += 1;
        } else if c == ' ' || c == '\t' {
            i += 1;
        } else if c == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' && chars[i] != '\r' {
                i += 1;
            }
        } else if c == '¬' || c == '\\' {
            // Line continuation: skip to and including the next newline.
            i += 1;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
            if chars.get(i) == Some(&'-') && chars.get(i + 1) == Some(&'-') {
                while i < chars.len() && chars[i] != '\n' && chars[i] != '\r' {
                    i += 1;
                }
            }
            if chars.get(i) == Some(&'\r') {
                i += 1;
            }
            if chars.get(i) == Some(&'\n') {
                i += 1;
            }
            line += 1;
        } else if c == '"' || c == '“' || c == '”' {
            let start = i + 1;
            i += 1;
            while i < chars.len() && !matches!(chars[i], '"' | '“' | '”' | '\n' | '\r') {
                i += 1;
            }
            if i >= chars.len() || chars[i] == '\n' || chars[i] == '\r' {
                return Err(format!("line {line}: missing closing quote"));
            }
            toks.push(Token { tok: Tok::Str(chars[start..i].iter().collect()), line, text: String::new() });
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            // Exponent notation, e.g. 1.5e10
            if i + 1 < chars.len()
                && (chars[i] == 'e' || chars[i] == 'E')
                && (chars[i + 1].is_ascii_digit()
                    || ((chars[i + 1] == '-' || chars[i + 1] == '+')
                        && chars.get(i + 2).is_some_and(|d| d.is_ascii_digit())))
            {
                i += 2;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            // A number immediately followed by letters is an identifier-ish word like "3d"; treat as word.
            if i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '_') {
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let w: String = chars[start..i].iter().collect();
                toks.push(Token { tok: Tok::Word(w.to_lowercase()), line, text: w });
            } else {
                toks.push(Token { tok: Tok::Number(chars[start..i].iter().collect()), line, text: String::new() });
            }
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let w: String = chars[start..i].iter().collect();
            toks.push(Token { tok: Tok::Word(w.to_lowercase()), line, text: w });
        } else {
            let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let Some(sym) = SYMBOLS.iter().find(|s| rest.starts_with(**s)) else {
                return Err(format!("line {line}: unexpected character '{c}'"));
            };
            toks.push(Token { tok: Tok::Sym(sym), line, text: String::new() });
            i += sym.chars().count();
        }
    }
    toks.push(Token { tok: Tok::Newline, line, text: String::new() });
    toks.push(Token { tok: Tok::Eof, line, text: String::new() });
    Ok(toks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let t = lex("put \"Hi\" into Fld 1 -- comment\nadd 1.5 to x ¬\n  & y").unwrap();
        let kinds: Vec<Tok> = t.into_iter().map(|t| t.tok).collect();
        assert_eq!(kinds[0], Tok::Word("put".into()));
        assert_eq!(kinds[1], Tok::Str("Hi".into()));
        assert_eq!(kinds[3], Tok::Word("fld".into()));
        assert_eq!(kinds[5], Tok::Newline);
        assert_eq!(kinds[7], Tok::Number("1.5".into()));
        assert_eq!(kinds[10], Tok::Sym("&"));
    }
}
