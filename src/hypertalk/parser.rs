//! Recursive-descent parser for HyperTalk, modelled on HyperCard 2.x syntax.

use super::ast::*;
use super::lexer::{Tok, Token, lex};
use crate::model::{Layer, PartKind};

type PResult<T> = Result<T, String>;

/// Words that can never begin a term; they end the expression before them.
const STOP_WORDS: &[&str] = &[
    "into", "before", "after", "to", "of", "in", "from", "then", "else", "with", "without", "and", "or", "is",
    "contains", "mod", "div", "times", "by", "at", "for", "until", "while", "end", "tool", "picture", "pict",
    "window", "box", "ascending", "descending", "numeric", "international", "datetime", "ticks", "tick",
    "seconds", "secs", "sec", "using", "where", "tempo", "cards", "cds", "marked",
];

const ZERO_ARG_FUNCS: &[&str] = &[
    "date", "time", "seconds", "secs", "sec", "ticks", "mouse", "mouseloc", "mouseh", "mousev", "mouseclick",
    "clickloc", "clickh", "clickv", "clickchunk", "clickline", "clicktext", "shiftkey", "optionkey",
    "commandkey", "cmdkey", "controlkey", "ctrlkey", "result", "tool", "paramcount", "params", "version",
    "selectedtext", "selectedchunk", "selectedline", "selectedfield", "selectedloc", "foundtext",
    "foundchunk", "foundline", "foundfield", "menus", "stacks", "windows", "sound", "speech", "voices",
    "screenrect", "systemversion", "diskspace", "heapspace", "osname", "osversion", "dateitems",
];

const ONE_ARG_FUNCS: &[&str] = &[
    "average", "min", "max", "sum", "random", "round", "sqrt", "trunc", "sin", "cos", "tan", "atan", "exp",
    "exp1", "exp2", "ln", "ln1", "log2", "abs", "chartonum", "numtochar", "value", "length", "param",
];

/// Properties that may be written without `the`, e.g. `visible of field 1`.
const PROPERTIES: &[&str] = &[
    "name", "id", "number", "rect", "rectangle", "loc", "location", "left", "top", "right", "bottom", "width",
    "height", "topleft", "botright", "bottomright", "visible", "style", "textfont", "textsize", "textstyle",
    "textalign", "textheight", "hilite", "highlight", "hilight", "highlite", "autohilite", "autohighlight",
    "showname", "enabled", "family", "sharedhilite", "icon", "locktext", "dontwrap", "autotab", "widemargins",
    "showlines", "sharedtext", "dontsearch", "autoselect", "scroll", "script", "contents", "text", "marked",
    "showpict", "cantdelete", "cantmodify", "owner", "partnumber", "checkmark", "commandchar", "menumessage",
];

const TYPE_NAMES: &[&str] =
    &["number", "integer", "point", "rect", "rectangle", "date", "logical", "boolean", "bool", "empty"];

/// Words that start a built-in command statement.
const COMMANDS: &[&str] = &[
    "put", "get", "set", "add", "subtract", "multiply", "divide", "answer", "ask", "beep", "go", "wait", "send",
    "do", "hide", "show", "lock", "unlock", "visual", "push", "pop", "find", "sort", "delete", "choose",
    "domenu", "mark", "unmark", "open", "close", "read", "write", "enable", "disable", "edit", "select",
    "click", "type", "convert", "play", "speak", "dial", "save", "arrowkey", "create", "reset", "start",
    "stop", "debug", "flash", "print", "drag", "help", "tabkey", "enterkey", "returnkey", "enterinfield",
    "returninfield",
];

/// Words that begin an expression rather than a message, when at the start of a statement.
const EXPR_START_WORDS: &[&str] = &[
    "the", "not", "there", "field", "fld", "fields", "flds", "card", "cd", "button", "btn", "buttons", "btns",
    "background", "bkgnd", "bg", "stack", "me", "target", "msg", "message", "this", "empty", "quote", "space",
    "tab", "comma", "colon", "pi", "true", "false", "char", "character", "chars", "characters", "word", "words",
    "item", "items", "line", "lines", "first", "second", "third", "fourth", "fifth", "sixth", "seventh",
    "eighth", "ninth", "tenth", "last", "middle", "mid", "any", "part", "marked", "recent", "prev",
    "previous", "number",
];

pub fn parse_script(src: &str) -> PResult<Script> {
    let mut p = Parser::new(src)?;
    p.script()
}

pub fn parse_statements(src: &str) -> PResult<Vec<Stmt>> {
    let mut p = Parser::new(src)?;
    let mut out = Vec::new();
    loop {
        p.skip_newlines();
        if p.at_eof() {
            break;
        }
        out.push(p.statement()?);
        if !p.at_newline() && !p.at_eof() {
            return Err(p.err_here("Expected end of line"));
        }
    }
    Ok(out)
}

pub fn parse_expression(src: &str) -> PResult<Expr> {
    let mut p = Parser::new(src)?;
    p.skip_newlines();
    let e = p.expr()?;
    p.skip_newlines();
    if !p.at_eof() {
        return Err(p.err_here("Expected end of expression"));
    }
    Ok(e)
}

/// Parse text such as `card 3` or `bkgnd button "OK"` as an object reference.
pub fn parse_object(src: &str) -> Option<ObjSpec> {
    let mut p = Parser::new(src).ok()?;
    let spec = p.object().ok()??;
    p.skip_newlines();
    p.at_eof().then_some(spec)
}

fn bool_lit(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

fn is_ordinal(w: &str) -> bool {
    matches!(
        w,
        "first"
            | "second"
            | "third"
            | "fourth"
            | "fifth"
            | "sixth"
            | "seventh"
            | "eighth"
            | "ninth"
            | "tenth"
            | "middle"
            | "mid"
            | "last"
            | "any"
    )
}

fn ordinal_of(w: &str) -> Option<Ordinal> {
    Some(match w {
        "first" => Ordinal::Nth(1),
        "second" => Ordinal::Nth(2),
        "third" => Ordinal::Nth(3),
        "fourth" => Ordinal::Nth(4),
        "fifth" => Ordinal::Nth(5),
        "sixth" => Ordinal::Nth(6),
        "seventh" => Ordinal::Nth(7),
        "eighth" => Ordinal::Nth(8),
        "ninth" => Ordinal::Nth(9),
        "tenth" => Ordinal::Nth(10),
        "middle" | "mid" => Ordinal::Middle,
        "last" => Ordinal::Last,
        "any" => Ordinal::Any,
        _ => return None,
    })
}

fn chunk_kind(w: &str) -> Option<ChunkKind> {
    Some(match w {
        "char" | "chars" | "character" | "characters" => ChunkKind::Char,
        "word" | "words" => ChunkKind::Word,
        "item" | "items" => ChunkKind::Item,
        "line" | "lines" => ChunkKind::Line,
        _ => return None,
    })
}

fn is_card_word(w: &str) -> bool {
    matches!(w, "card" | "cd")
}

fn is_cards_word(w: &str) -> bool {
    matches!(w, "cards" | "cds")
}

fn is_bg_word(w: &str) -> bool {
    matches!(w, "background" | "backgrounds" | "bkgnd" | "bkgnds" | "bg" | "bgs")
}

fn part_kind(w: &str) -> Option<PartKind> {
    match w {
        "button" | "buttons" | "btn" | "btns" => Some(PartKind::Button),
        "field" | "fields" | "fld" | "flds" => Some(PartKind::Field),
        _ => None,
    }
}

fn is_part_word(w: &str) -> bool {
    part_kind(w).is_some() || w == "part" || w == "parts"
}

fn constant(w: &str) -> Option<&'static str> {
    Some(match w {
        "empty" => "",
        "quote" => "\"",
        "return" | "linefeed" => "\n",
        "space" => " ",
        "tab" => "\t",
        "comma" => ",",
        "colon" => ":",
        "formfeed" => "\x0c",
        "pi" => "3.14159265358979323846",
        "true" => "true",
        "false" => "false",
        "up" => "up",
        "down" => "down",
        "zero" => "0",
        "one" => "1",
        "two" => "2",
        "three" => "3",
        "four" => "4",
        "five" => "5",
        "six" => "6",
        "seven" => "7",
        "eight" => "8",
        "nine" => "9",
        "ten" => "10",
        _ => return None,
    })
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    /// How many multi-line `if` blocks enclose the current position.
    if_depth: usize,
}

impl Parser {
    fn new(src: &str) -> PResult<Parser> {
        Ok(Parser { toks: lex(src)?, pos: 0, if_depth: 0 })
    }

    // ----- token helpers -----

    fn tok(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }

    fn tok_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }

    fn line(&self) -> usize {
        self.toks[self.pos.min(self.toks.len() - 1)].line
    }

    fn word(&self) -> Option<&str> {
        self.word_at(0)
    }

    fn word_at(&self, n: usize) -> Option<&str> {
        match self.tok_at(n) {
            Tok::Word(w) => Some(w.as_str()),
            _ => None,
        }
    }

    fn is(&self, w: &str) -> bool {
        self.word() == Some(w)
    }

    fn is_at(&self, n: usize, w: &str) -> bool {
        self.word_at(n) == Some(w)
    }

    fn is_sym(&self, s: &str) -> bool {
        matches!(self.tok(), Tok::Sym(x) if *x == s)
    }

    fn advance(&mut self) {
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
    }

    fn accept(&mut self, w: &str) -> bool {
        if self.is(w) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn accept_any(&mut self, ws: &[&str]) -> Option<String> {
        let w = self.word()?;
        if ws.contains(&w) {
            let w = w.to_string();
            self.advance();
            Some(w)
        } else {
            None
        }
    }

    fn accept_sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, w: &str) -> PResult<()> {
        if self.accept(w) { Ok(()) } else { Err(self.err_here(&format!("Expected \"{w}\""))) }
    }

    fn describe(&self) -> String {
        match self.tok() {
            Tok::Word(w) => format!("\"{w}\""),
            Tok::Number(n) => n.clone(),
            Tok::Str(s) => format!("\"{s}\""),
            Tok::Sym(s) => format!("\"{s}\""),
            Tok::Other(c) => format!("\"{c}\""),
            Tok::Newline => "end of line".into(),
            Tok::Eof => "end of script".into(),
        }
    }

    fn err_here(&self, msg: &str) -> String {
        format!("line {}: {msg}, found {}", self.line(), self.describe())
    }

    fn at_newline(&self) -> bool {
        matches!(self.tok(), Tok::Newline)
    }

    fn at_eof(&self) -> bool {
        matches!(self.tok(), Tok::Eof)
    }

    fn at_stmt_end(&self) -> bool {
        self.at_newline() || self.at_eof() || self.is("else")
    }

    fn skip_newlines(&mut self) {
        while self.at_newline() {
            self.advance();
        }
    }

    fn symbol(&mut self) -> PResult<String> {
        match self.tok().clone() {
            Tok::Word(w) => {
                self.advance();
                Ok(w)
            }
            _ => Err(self.err_here("Expected a name")),
        }
    }

    /// Consume tokens to the end of the line (for commands we accept but ignore).
    fn rest_of_line(&mut self) {
        while !self.at_newline() && !self.at_eof() {
            self.advance();
        }
    }

    /// Reconstruct the source text of tokens `start..end`.
    fn source_text(&self, start: usize, end: usize) -> String {
        let mut out = String::new();
        for t in self.toks.get(start..end).unwrap_or(&[]) {
            let piece = match &t.tok {
                Tok::Word(_) => t.text.clone(),
                Tok::Number(n) => n.clone(),
                Tok::Str(s) => format!("\"{s}\""),
                Tok::Sym(s) => s.to_string(),
                Tok::Other(c) => c.to_string(),
                Tok::Newline | Tok::Eof => break,
            };
            let tight = matches!(t.tok, Tok::Sym("," | ")")) || out.ends_with('(');
            if !out.is_empty() && !tight {
                out.push(' ');
            }
            out.push_str(&piece);
        }
        out
    }

    fn starts_term_at(&self, n: usize) -> bool {
        match self.tok_at(n) {
            Tok::Number(_) | Tok::Str(_) => true,
            Tok::Sym(s) => *s == "(",
            Tok::Word(w) => !STOP_WORDS.contains(&w.as_str()),
            _ => false,
        }
    }

    fn starts_term(&self) -> bool {
        self.starts_term_at(0)
    }

    // ----- scripts and statements -----

    fn script(&mut self) -> PResult<Script> {
        let mut script = Script::default();
        loop {
            self.skip_newlines();
            if self.at_eof() {
                break;
            }
            let is_func = if self.accept("on") {
                false
            } else if self.accept("function") {
                true
            } else {
                // HyperCard ignores anything outside handlers, such as banner text.
                self.rest_of_line();
                continue;
            };
            let name = self.symbol()?;
            let mut params = Vec::new();
            if !self.at_newline() {
                params.push(self.symbol()?);
                while self.accept_sym(",") {
                    params.push(self.symbol()?);
                }
            }
            let body = self.block(&["end"])?;
            self.expect("end")?;
            let end_name = self.symbol()?;
            if end_name != name {
                return Err(format!("line {}: Expected \"end {name}\" but found \"end {end_name}\"", self.line()));
            }
            let handler = Handler { params, body };
            if is_func {
                script.functions.insert(name, handler);
            } else {
                script.handlers.insert(name, handler);
            }
        }
        Ok(script)
    }

    /// Parse statements, one per line, until one of `terminators` starts a line.
    fn block(&mut self, terminators: &[&str]) -> PResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_eof() {
                return Err(self.err_here(&format!("Expected \"{}\"", terminators.join("\" or \""))));
            }
            if self.if_depth == 0 && self.is("end") && self.is_at(1, "if") {
                // A stray `end if` after a one-line else; HyperCard tolerates it.
                self.pos += 2;
                continue;
            }
            if terminators.iter().any(|t| self.is(t)) {
                return Ok(stmts);
            }
            stmts.push(self.statement()?);
            if !self.at_newline() && !self.at_eof() {
                return Err(self.err_here("Expected end of line"));
            }
        }
    }

    fn statement(&mut self) -> PResult<Stmt> {
        let line = self.line();
        let kind = self.statement_kind()?;
        Ok(Stmt { line, kind })
    }

    fn statement_kind(&mut self) -> PResult<StmtKind> {
        let Some(w) = self.word().map(str::to_string) else {
            return Ok(StmtKind::Expr(self.expr()?));
        };
        match w.as_str() {
            "if" => return self.if_stmt(),
            "repeat" => return self.repeat_stmt(),
            "global" => {
                self.advance();
                let mut names = vec![self.symbol()?];
                while self.accept_sym(",") {
                    names.push(self.symbol()?);
                }
                return Ok(StmtKind::Global(names));
            }
            "return" => {
                self.advance();
                if self.at_stmt_end() {
                    return Ok(StmtKind::Return(None));
                }
                return Ok(StmtKind::Return(Some(self.expr()?)));
            }
            "exit" => {
                self.advance();
                if self.accept("repeat") {
                    return Ok(StmtKind::ExitRepeat);
                }
                if self.accept("to") {
                    self.accept_any(&["hypercard", "randomcard"]);
                    return Ok(StmtKind::ExitToHyperCard);
                }
                self.symbol()?;
                return Ok(StmtKind::ExitHandler);
            }
            "pass" => {
                self.advance();
                self.symbol()?;
                return Ok(StmtKind::Pass);
            }
            "next" if self.is_at(1, "repeat") => {
                self.advance();
                self.advance();
                return Ok(StmtKind::NextRepeat);
            }
            _ => {}
        }
        if COMMANDS.contains(&w.as_str()) {
            let start = self.pos + 1;
            let cmd = self.command(&w)?;
            let args = self.source_text(start, self.pos);
            return Ok(StmtKind::Cmd { name: w, cmd, args });
        }
        if EXPR_START_WORDS.contains(&w.as_str()) || constant(&w).is_some() {
            return Ok(StmtKind::Expr(self.expr()?));
        }
        // A message: a name followed by optional comma-separated arguments.
        self.advance();
        let mut args = Vec::new();
        if !self.at_stmt_end() {
            args.push(self.expr()?);
            while self.accept_sym(",") {
                args.push(self.expr()?);
            }
        }
        Ok(StmtKind::Message { name: w, args })
    }

    fn if_stmt(&mut self) -> PResult<StmtKind> {
        self.expect("if")?;
        let cond = self.expr()?;
        self.skip_newlines();
        self.expect("then")?;
        if self.at_newline() {
            self.if_depth += 1;
            let then = self.block(&["else", "end"]);
            self.if_depth -= 1;
            let then = then?;
            if self.accept("end") {
                self.expect("if")?;
                return Ok(StmtKind::If { cond, then, els: None });
            }
            let els = self.else_part()?;
            return Ok(StmtKind::If { cond, then, els: Some(els) });
        }
        let then = vec![self.statement()?];
        if self.is("else") {
            let els = self.else_part()?;
            return Ok(StmtKind::If { cond, then, els: Some(els) });
        }
        // An `else` may begin the following line.
        let mut n = 0;
        while matches!(self.tok_at(n), Tok::Newline) {
            n += 1;
        }
        if n > 0 && self.is_at(n, "else") {
            self.skip_newlines();
            let els = self.else_part()?;
            return Ok(StmtKind::If { cond, then, els: Some(els) });
        }
        Ok(StmtKind::If { cond, then, els: None })
    }

    fn else_part(&mut self) -> PResult<Vec<Stmt>> {
        self.expect("else")?;
        if self.at_newline() {
            self.if_depth += 1;
            let els = self.block(&["end"]);
            self.if_depth -= 1;
            let els = els?;
            self.expect("end")?;
            self.expect("if")?;
            return Ok(els);
        }
        // A one-line `else` finishes the if, even after a multi-line `then`;
        // an `end if` that follows belongs to an enclosing if.
        Ok(vec![self.statement()?])
    }

    fn repeat_stmt(&mut self) -> PResult<StmtKind> {
        self.expect("repeat")?;
        let kind = if self.at_newline() || self.accept("forever") {
            RepeatKind::Forever
        } else if self.accept("while") {
            RepeatKind::While(self.expr()?)
        } else if self.accept("until") {
            RepeatKind::Until(self.expr()?)
        } else if self.accept("with") {
            let var = self.symbol()?;
            if !self.accept_sym("=") {
                return Err(self.err_here("Expected \"=\""));
            }
            let from = self.expr()?;
            let down = self.accept("down");
            self.expect("to")?;
            let to = self.expr()?;
            RepeatKind::With { var, from, to, down }
        } else {
            self.accept("for");
            let count = self.expr()?;
            self.accept("times");
            RepeatKind::Count(count)
        };
        let body = self.block(&["end"])?;
        self.expect("end")?;
        self.expect("repeat")?;
        Ok(StmtKind::Repeat { kind, body })
    }

    /// A comma-separated list of expressions joined into one comma-delimited value.
    fn list_expr(&mut self) -> PResult<Expr> {
        let mut e = self.expr()?;
        while self.accept_sym(",") {
            let rhs = self.expr()?;
            e = Expr::Binary(
                BinOp::Concat,
                Box::new(Expr::Binary(BinOp::Concat, Box::new(e), Box::new(Expr::Lit(",".into())))),
                Box::new(rhs),
            );
        }
        Ok(e)
    }

    fn prep(&mut self) -> Option<Prep> {
        if self.accept("into") {
            Some(Prep::Into)
        } else if self.accept("before") {
            Some(Prep::Before)
        } else if self.accept("after") {
            Some(Prep::After)
        } else {
            None
        }
    }

    // ----- commands -----

    fn command(&mut self, name: &str) -> PResult<Cmd> {
        self.advance();
        Ok(match name {
            "put" => {
                let value = self.list_expr()?;
                let cmd = match self.prep() {
                    Some(prep) => Cmd::Put { value, prep, dest: Some(self.expr()?) },
                    None => Cmd::Put { value, prep: Prep::Into, dest: None },
                };
                // `put items into menu "X" with menuMessages msgs`
                if self.accept("with") {
                    self.accept_any(&["menumessages", "menumessage", "menumsgs", "menumsg"]);
                    self.list_expr()?;
                }
                cmd
            }
            "get" => Cmd::Get(self.list_expr()?),
            "set" => {
                self.accept("the");
                let mut prop = self.symbol()?;
                if matches!(prop.as_str(), "long" | "short" | "abbr" | "abbrev" | "abbreviated") {
                    prop = self.symbol()?;
                }
                let obj = if self.accept("of") || self.accept("in") { Some(self.term()?) } else { None };
                self.expect("to")?;
                Cmd::Set { prop, obj, value: self.list_expr()? }
            }
            "add" => {
                let value = self.expr()?;
                self.expect("to")?;
                Cmd::Add { value, dest: self.expr()? }
            }
            "subtract" => {
                let value = self.expr()?;
                self.expect("from")?;
                Cmd::Subtract { value, dest: self.expr()? }
            }
            "multiply" => {
                let dest = self.expr()?;
                self.expect("by")?;
                Cmd::Multiply { dest, value: self.expr()? }
            }
            "divide" => {
                let dest = self.expr()?;
                self.expect("by")?;
                Cmd::Divide { dest, value: self.expr()? }
            }
            "answer" => {
                if self.accept("file") {
                    let prompt = self.expr()?;
                    if self.accept("of") {
                        self.accept("type");
                        self.expr()?;
                    }
                    return Ok(Cmd::AnswerFile(prompt));
                }
                let prompt = self.expr()?;
                let mut buttons = Vec::new();
                if self.accept("with") {
                    buttons.push(self.term()?);
                    while self.accept("or") {
                        buttons.push(self.term()?);
                    }
                }
                Cmd::Answer { prompt, buttons }
            }
            "ask" => {
                if self.accept("file") {
                    let prompt = self.expr()?;
                    let default = if self.accept("with") { Some(self.expr()?) } else { None };
                    return Ok(Cmd::AskFile { prompt, default });
                }
                let password = self.accept("password");
                if password {
                    self.accept("clear");
                }
                let prompt = self.expr()?;
                let default = if self.accept("with") { Some(self.expr()?) } else { None };
                Cmd::Ask { prompt, default, password }
            }
            "beep" => Cmd::Beep(if self.at_stmt_end() { None } else { Some(self.expr()?) }),
            "go" => {
                self.accept("to");
                let dest = self.go_dest()?;
                // Options that only matter with multiple windows.
                while let Some(_w) = self.accept_any(&["in", "a", "new", "window", "without", "dialog"]) {}
                Cmd::Go(dest)
            }
            "wait" => {
                if self.accept("until") {
                    return Ok(Cmd::WaitUntil(self.expr()?));
                }
                if self.accept("while") {
                    return Ok(Cmd::WaitWhile(self.expr()?));
                }
                self.accept("for");
                let n = self.expr()?;
                let unit = if self.accept_any(&["seconds", "second", "secs", "sec"]).is_some() {
                    TimeUnit::Seconds
                } else {
                    self.accept_any(&["ticks", "tick"]);
                    TimeUnit::Ticks
                };
                Cmd::Wait(n, unit)
            }
            "send" => {
                let msg = self.list_expr()?;
                let target = if self.accept("to") { Some(self.expr()?) } else { None };
                Cmd::Send { msg, target }
            }
            "do" => Cmd::Do(self.expr()?),
            "hide" => Cmd::Hide(self.show_hide()?),
            "show" => {
                if self.accept("all") {
                    self.accept_any(&["cards", "cds"]);
                    return Ok(Cmd::ShowCards(None));
                }
                if self.accept("marked") || self.word().is_some_and(is_cards_word) {
                    self.accept_any(&["cards", "cds"]);
                    return Ok(Cmd::ShowCards(None));
                }
                if matches!(self.tok(), Tok::Number(_)) && self.word_at(1).is_some_and(is_cards_word) {
                    let n = self.term()?;
                    self.advance();
                    return Ok(Cmd::ShowCards(Some(n)));
                }
                let what = self.show_hide()?;
                let at = if self.accept("at") { Some(self.list_expr()?) } else { None };
                Cmd::Show { what, at }
            }
            "lock" => {
                if self.accept("messages") {
                    Cmd::LockMessages
                } else {
                    self.expect("screen")?;
                    Cmd::LockScreen
                }
            }
            "unlock" => {
                if self.accept("messages") {
                    Cmd::UnlockMessages
                } else {
                    self.expect("screen")?;
                    if self.accept("with") {
                        self.accept("visual");
                        self.accept("effect");
                        Cmd::UnlockScreen(Some(self.effect()?))
                    } else {
                        Cmd::UnlockScreen(None)
                    }
                }
            }
            "visual" => {
                self.accept("effect");
                Cmd::VisualEffect(self.effect()?)
            }
            "push" => {
                if self.word().is_some_and(is_card_word) && self.at_stmt_end_at(1) {
                    self.advance();
                    Cmd::PushCard(None)
                } else {
                    Cmd::PushCard(Some(self.expr()?))
                }
            }
            "pop" => {
                if !self.word().is_some_and(is_card_word) {
                    return Err(self.err_here("Expected \"card\""));
                }
                self.advance();
                match self.prep() {
                    Some(prep) => Cmd::PopCard(Some((prep, self.expr()?))),
                    None => Cmd::PopCard(None),
                }
            }
            "find" => {
                let mode = self.find_mode();
                self.accept("international");
                let what = self.expr()?;
                let field = if self.accept("in") || self.accept("of") {
                    if self.is("marked") {
                        None
                    } else {
                        Some(self.expr()?)
                    }
                } else {
                    None
                };
                if self.accept("of") || self.accept("in") || self.is("marked") {
                    self.accept("marked");
                    self.accept_any(&["cards", "cds"]);
                }
                Cmd::Find { mode, what, field }
            }
            "sort" => self.sort_cmd()?,
            "delete" => Cmd::Delete(self.expr()?),
            "choose" => {
                if self.accept("tool") {
                    return Ok(Cmd::ChooseTool(self.expr()?));
                }
                if matches!(self.tok(), Tok::Str(_) | Tok::Number(_) | Tok::Sym("(")) {
                    let e = self.expr()?;
                    self.accept("tool");
                    return Ok(Cmd::ChooseTool(e));
                }
                let mut words = Vec::new();
                while let Some(w) = self.word() {
                    if w == "tool" {
                        break;
                    }
                    words.push(w.to_string());
                    self.advance();
                }
                self.accept("tool");
                Cmd::ChooseTool(Expr::Lit(words.join(" ")))
            }
            "domenu" => {
                let e = self.expr()?;
                if self.accept_sym(",") {
                    self.expr()?;
                }
                while self.accept_any(&["without", "dialog"]).is_some() {}
                Cmd::DoMenu(e)
            }
            "mark" => Cmd::Mark(self.mark_what()?),
            "unmark" => Cmd::Unmark(self.mark_what()?),
            "open" => {
                if self.accept("file") {
                    Cmd::OpenFile(self.expr()?)
                } else {
                    self.rest_of_line();
                    Cmd::NoOp
                }
            }
            "close" => {
                if self.accept("file") {
                    Cmd::CloseFile(self.expr()?)
                } else {
                    self.rest_of_line();
                    Cmd::NoOp
                }
            }
            "read" => {
                self.expect("from")?;
                self.expect("file")?;
                let file = self.expr()?;
                let at = if self.accept("at") { Some(self.expr()?) } else { None };
                let (mut until, mut count) = (None, None);
                if self.accept("until") {
                    until = Some(self.expr()?);
                } else if self.accept("for") {
                    count = Some(self.expr()?);
                }
                Cmd::ReadFile { file, until, count, at }
            }
            "write" => {
                let data = self.expr()?;
                self.expect("to")?;
                self.expect("file")?;
                let file = self.expr()?;
                let (mut at, mut at_end) = (None, false);
                if self.accept("at") {
                    if self.accept_any(&["eof", "end"]).is_some() {
                        at_end = true;
                    } else {
                        at = Some(self.expr()?);
                    }
                }
                Cmd::WriteFile { data, file, at, at_end }
            }
            "enable" => Cmd::Enable(self.expr()?),
            "disable" => Cmd::Disable(self.expr()?),
            "edit" => {
                self.accept("the");
                self.expect("script")?;
                self.expect("of")?;
                Cmd::EditScript(self.expr()?)
            }
            "select" => {
                if self.accept("empty") {
                    return Ok(Cmd::Select(None));
                }
                self.accept_any(&["before", "after"]);
                if self.accept("text") {
                    self.expect("of")?;
                }
                Cmd::Select(Some(self.expr()?))
            }
            "click" => {
                self.expect("at")?;
                let e = self.list_expr()?;
                if self.accept("with") {
                    self.list_expr()?;
                }
                Cmd::ClickAt(e)
            }
            "drag" => {
                self.expect("from")?;
                let from = self.list_expr()?;
                self.expect("to")?;
                let to = self.list_expr()?;
                if self.accept("with") {
                    self.list_expr()?;
                }
                Cmd::Drag { from, to }
            }
            "type" => {
                let e = self.expr()?;
                if self.accept("with") {
                    self.list_expr()?;
                }
                Cmd::Type(e)
            }
            "convert" => {
                let value = self.expr()?;
                if self.accept("from") {
                    while !self.is("to") && !self.at_stmt_end() {
                        self.advance();
                    }
                }
                self.expect("to")?;
                let mut words = Vec::new();
                while let Some(w) = self.word() {
                    words.push(w.to_string());
                    self.advance();
                }
                Cmd::Convert { value, to: words.join(" ") }
            }
            "play" => {
                if self.accept("stop") {
                    return Ok(Cmd::PlayStop);
                }
                let sound = self.expr()?;
                let mut tempo = None;
                let mut notes = Vec::new();
                while !self.at_stmt_end() {
                    if self.accept("tempo") {
                        tempo = Some(self.term()?);
                    } else {
                        notes.push(self.expr()?);
                    }
                }
                Cmd::Play { sound, tempo, notes }
            }
            "speak" => {
                let e = self.expr()?;
                if self.accept("with") {
                    self.rest_of_line();
                }
                Cmd::Speak(e)
            }
            "dial" => {
                let number = self.expr()?;
                // `with modem` and the like don't apply.
                self.rest_of_line();
                Cmd::Dial(number)
            }
            "save" => {
                self.accept("this");
                self.accept("stack");
                if self.accept("as") {
                    self.accept("stack");
                    Cmd::SaveStack(Some(self.expr()?))
                } else {
                    if !self.at_stmt_end() {
                        // `save stack "x" as "y"` — saving other stacks isn't supported; save as y.
                        self.expr()?;
                        self.expect("as")?;
                        self.accept("stack");
                        return Ok(Cmd::SaveStack(Some(self.expr()?)));
                    }
                    Cmd::SaveStack(None)
                }
            }
            "arrowkey" => Cmd::ArrowKey(self.expr()?),
            "tabkey" | "enterkey" | "returnkey" | "enterinfield" | "returninfield" => Cmd::NoOp,
            _ => {
                // create menu, reset menubar, start using, debug, flash, print, drag, help ...
                self.rest_of_line();
                Cmd::NoOp
            }
        })
    }

    fn at_stmt_end_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Newline | Tok::Eof) || self.is_at(n, "else")
    }

    fn find_mode(&mut self) -> FindMode {
        if self.accept("whole") {
            FindMode::Whole
        } else if self.accept("string") {
            FindMode::String
        } else if self.accept("chars") {
            FindMode::Chars
        } else if self.accept("word") {
            FindMode::Word
        } else {
            self.accept("normal");
            FindMode::Normal
        }
    }

    fn mark_what(&mut self) -> PResult<MarkWhat> {
        if self.accept("all") {
            self.accept_any(&["cards", "cds"]);
            return Ok(MarkWhat::All);
        }
        if self.word().is_some_and(is_cards_word) {
            self.advance();
            if self.accept("where") {
                return Ok(MarkWhat::Where(self.expr()?));
            }
            self.expect("by")?;
            self.expect("finding")?;
            let mode = self.find_mode();
            self.accept("international");
            let what = self.expr()?;
            let field = if self.accept("in") || self.accept("of") { Some(self.expr()?) } else { None };
            return Ok(MarkWhat::Finding(mode, what, field));
        }
        Ok(MarkWhat::Card(self.expr()?))
    }

    fn sort_cmd(&mut self) -> PResult<Cmd> {
        self.accept("the");
        let marked = self.accept("marked");
        let mut cards = marked;
        let mut of = None;
        if self.word().is_some_and(is_cards_word) {
            self.advance();
            cards = true;
            if self.accept("of") {
                if self.is("this") && self.is_at(1, "stack") || self.is("stack") {
                    self.accept("this");
                    self.advance();
                } else {
                    // `sort cards of this background by ...`
                    of = Some(self.term()?);
                }
            }
        } else if self.is("this") && self.is_at(1, "stack") || self.is("stack") {
            self.accept("this");
            self.advance();
            cards = true;
        } else if self.word().is_some_and(|w| {
            matches!(w, "ascending" | "descending" | "text" | "numeric" | "international" | "datetime" | "by")
        }) {
            cards = true;
        }
        let mut kind = ChunkKind::Line;
        let mut container = None;
        if !cards {
            if let Some(k) = self.word().and_then(chunk_kind)
                && (self.is_at(1, "of") || self.is_at(1, "in"))
            {
                kind = k;
                self.advance();
                self.advance();
            }
            container = Some(self.expr()?);
        }
        let ascending = !self.accept("descending");
        self.accept("ascending");
        let style = if self.accept("numeric") {
            SortStyle::Numeric
        } else if self.accept("datetime") {
            SortStyle::DateTime
        } else {
            self.accept_any(&["text", "international"]);
            SortStyle::Text
        };
        let by = if self.accept("by") { Some(self.expr()?) } else { None };
        match container {
            Some(container) => Ok(Cmd::SortChunks { kind, container, ascending, style, by }),
            None => {
                let by = by.ok_or_else(|| self.err_here("Expected \"by\""))?;
                Ok(Cmd::SortCards { ascending, style, by, marked, of })
            }
        }
    }

    fn show_hide(&mut self) -> PResult<ShowHide> {
        self.accept("the");
        if self.accept("menubar") {
            return Ok(ShowHide::MenuBar);
        }
        if self.accept("titlebar") {
            return Ok(ShowHide::TitleBar);
        }
        if self.word().is_some_and(is_card_word) && matches!(self.word_at(1), Some("picture" | "pict")) {
            self.advance();
            self.advance();
            return Ok(ShowHide::CardPicture);
        }
        if self.word().is_some_and(is_bg_word) && matches!(self.word_at(1), Some("picture" | "pict")) {
            self.advance();
            self.advance();
            return Ok(ShowHide::BgPicture);
        }
        if self.accept_any(&["picture", "pict"]).is_some() {
            self.expect("of")?;
            return Ok(ShowHide::PictureOf(self.expr()?));
        }
        let e = self.expr()?;
        if matches!(e, Expr::Object(ObjSpec::MsgBox)) {
            return Ok(ShowHide::MsgBox);
        }
        Ok(ShowHide::Object(e))
    }

    fn effect(&mut self) -> PResult<Effect> {
        let name = if matches!(self.tok(), Tok::Word(_)) {
            let mut words = Vec::new();
            while let Some(w) = self.word() {
                if matches!(w, "fast" | "slow" | "slowly" | "very" | "to") {
                    break;
                }
                words.push(w.to_string());
                self.advance();
            }
            Expr::Lit(words.join(" "))
        } else {
            self.term()?
        };
        let mut speed = None;
        if self.accept("very") {
            if self.accept("fast") {
                speed = Some("very fast".into());
            } else {
                self.accept_any(&["slow", "slowly"]);
                speed = Some("very slow".into());
            }
        } else if self.accept("fast") {
            speed = Some("fast".into());
        } else if self.accept_any(&["slow", "slowly"]).is_some() {
            speed = Some("slow".into());
        }
        if self.accept("to") {
            self.term()?;
        }
        Ok(Effect { name, speed })
    }

    fn go_dest(&mut self) -> PResult<GoDest> {
        if self.is("the")
            && self.word_at(1).is_some_and(|w| is_ordinal(w) || matches!(w, "next" | "prev" | "previous"))
        {
            self.advance();
        }
        let card_at = |n: usize| self.word_at(n).is_some_and(is_card_word);
        let simple_position = self.at_stmt_end_at(1)
            || card_at(1) && self.at_stmt_end_at(2)
            || self.is_at(1, "marked") && card_at(2) && self.at_stmt_end_at(3);
        if let Some(pos) = self.position_word().filter(|_| simple_position) {
            // `go next`, `go prev card`, `go next marked card`
            self.advance();
            let marked = self.accept("marked");
            self.accept_any(&["card", "cd"]);
            return Ok(GoDest::Card(if marked { CardSpec::MarkedPosition(pos) } else { CardSpec::Position(pos) }));
        }
        if self.accept("back") {
            return Ok(GoDest::Card(CardSpec::Back));
        }
        if self.accept("forth") {
            return Ok(GoDest::Card(CardSpec::Forth));
        }
        if self.accept("home") {
            return Ok(GoDest::Card(CardSpec::Which(Which::Ordinal(Ordinal::Nth(1)))));
        }
        if let Some(ord) = self.word().and_then(ordinal_of)
            && self.at_stmt_end_at(1)
        {
            // `go first`, `go last`
            self.advance();
            return Ok(GoDest::Card(CardSpec::Which(Which::Ordinal(ord))));
        }
        let e = self.expr()?;
        if let Expr::Object(ObjSpec::Card(spec)) = e {
            return Ok(GoDest::Card(spec));
        }
        Ok(GoDest::Expr(e))
    }

    fn position_word(&self) -> Option<Position> {
        match self.word()? {
            "next" => Some(Position::Next),
            "prev" | "previous" => Some(Position::Prev),
            _ => None,
        }
    }

    // ----- expressions -----

    pub fn expr(&mut self) -> PResult<Expr> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> PResult<Expr> {
        let mut e = self.and_expr()?;
        while self.accept("or") {
            e = Expr::Binary(BinOp::Or, Box::new(e), Box::new(self.and_expr()?));
        }
        Ok(e)
    }

    fn and_expr(&mut self) -> PResult<Expr> {
        let mut e = self.eq_expr()?;
        while self.accept("and") {
            e = Expr::Binary(BinOp::And, Box::new(e), Box::new(self.eq_expr()?));
        }
        Ok(e)
    }

    fn eq_expr(&mut self) -> PResult<Expr> {
        let mut e = self.cmp_expr()?;
        loop {
            let op = if self.accept_sym("=") {
                BinOp::Eq
            } else if self.accept_sym("<>") || self.accept_sym("≠") {
                BinOp::Ne
            } else if self.is("is") {
                self.advance();
                if self.accept("not") { BinOp::Ne } else { BinOp::Eq }
            } else {
                break;
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.cmp_expr()?));
        }
        Ok(e)
    }

    fn is_type_at(&self, n: usize) -> bool {
        matches!(self.word_at(n), Some("a" | "an")) && self.word_at(n + 1).is_some_and(|w| TYPE_NAMES.contains(&w))
    }

    fn cmp_expr(&mut self) -> PResult<Expr> {
        let mut e = self.concat_expr()?;
        loop {
            let op = if self.accept_sym("<") {
                BinOp::Lt
            } else if self.accept_sym(">") {
                BinOp::Gt
            } else if self.accept_sym("<=") || self.accept_sym("≤") {
                BinOp::Le
            } else if self.accept_sym(">=") || self.accept_sym("≥") {
                BinOp::Ge
            } else if self.accept("contains") {
                BinOp::Contains
            } else if self.is("is") {
                // Only the comparison forms of "is"; plain equality is handled one level up.
                if self.is_at(1, "in") {
                    self.advance();
                    self.advance();
                    BinOp::IsIn
                } else if self.is_at(1, "within") {
                    self.advance();
                    self.advance();
                    BinOp::IsWithin
                } else if self.is_at(1, "not") && self.is_at(2, "in") {
                    self.pos += 3;
                    BinOp::IsNotIn
                } else if self.is_at(1, "not") && self.is_at(2, "within") {
                    self.pos += 3;
                    BinOp::IsNotWithin
                } else if self.is_type_at(1) {
                    self.pos += 2;
                    let t = self.symbol()?;
                    e = Expr::IsType(true, Box::new(e), t);
                    continue;
                } else if self.is_at(1, "not") && self.is_type_at(2) {
                    self.pos += 3;
                    let t = self.symbol()?;
                    e = Expr::IsType(false, Box::new(e), t);
                    continue;
                } else {
                    break;
                }
            } else {
                break;
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.concat_expr()?));
        }
        Ok(e)
    }

    fn concat_expr(&mut self) -> PResult<Expr> {
        let mut e = self.add_expr()?;
        loop {
            let op = if self.accept_sym("&&") {
                BinOp::ConcatSpace
            } else if self.accept_sym("&") {
                BinOp::Concat
            } else {
                break;
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.add_expr()?));
        }
        Ok(e)
    }

    fn add_expr(&mut self) -> PResult<Expr> {
        let mut e = self.mul_expr()?;
        loop {
            let op = if self.accept_sym("+") {
                BinOp::Add
            } else if self.accept_sym("-") {
                BinOp::Sub
            } else {
                break;
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.mul_expr()?));
        }
        Ok(e)
    }

    fn mul_expr(&mut self) -> PResult<Expr> {
        let mut e = self.pow_expr()?;
        loop {
            let op = if self.accept_sym("*") {
                BinOp::Mul
            } else if self.accept_sym("/") {
                BinOp::Div
            } else if self.accept("div") {
                BinOp::IntDiv
            } else if self.accept("mod") {
                BinOp::Mod
            } else {
                break;
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.pow_expr()?));
        }
        Ok(e)
    }

    fn pow_expr(&mut self) -> PResult<Expr> {
        let mut e = self.unary_expr()?;
        while self.accept_sym("^") {
            e = Expr::Binary(BinOp::Pow, Box::new(e), Box::new(self.unary_expr()?));
        }
        Ok(e)
    }

    fn unary_expr(&mut self) -> PResult<Expr> {
        if self.accept_sym("-") {
            return Ok(Expr::Unary(UnOp::Neg, Box::new(self.unary_expr()?)));
        }
        if self.accept("not") {
            return Ok(Expr::Unary(UnOp::Not, Box::new(self.unary_expr()?)));
        }
        if self.is("there") && self.is_at(1, "is") {
            let positive = if matches!(self.word_at(2), Some("a" | "an")) {
                self.pos += 3;
                true
            } else if self.is_at(2, "no") {
                self.pos += 3;
                false
            } else if self.is_at(2, "not") && matches!(self.word_at(3), Some("a" | "an")) {
                self.pos += 4;
                false
            } else {
                return Err(self.err_here("Expected \"there is a\" or \"there is no\""));
            };
            if self.accept_any(&["menu", "menuitem"]).is_some() {
                // Menus aren't scriptable here: no menu or menu item exists.
                self.term()?;
                if self.accept("of") {
                    self.expect("menu")?;
                    self.term()?;
                }
                return Ok(Expr::Lit(bool_lit(!positive).into()));
            }
            if self.accept("file") {
                let path = self.term()?;
                let test = Expr::Call("__fileexists".into(), vec![path]);
                return Ok(if positive { test } else { Expr::Unary(UnOp::Not, Box::new(test)) });
            }
            let Some(obj) = self.object()? else {
                return Err(self.err_here("Expected an object after \"there is a\""));
            };
            return Ok(Expr::ThereIs(positive, obj));
        }
        self.term()
    }

    pub fn term(&mut self) -> PResult<Expr> {
        match self.tok().clone() {
            Tok::Number(n) => {
                self.advance();
                Ok(Expr::Lit(n))
            }
            Tok::Str(s) => {
                self.advance();
                Ok(Expr::Lit(s))
            }
            Tok::Sym("(") => {
                self.advance();
                let e = self.expr()?;
                if !self.accept_sym(")") {
                    return Err(self.err_here("Expected \")\""));
                }
                Ok(e)
            }
            Tok::Sym("-") => {
                self.advance();
                Ok(Expr::Unary(UnOp::Neg, Box::new(self.term()?)))
            }
            Tok::Word(w) => self.word_term(&w),
            _ => Err(self.err_here("Expected an expression")),
        }
    }

    fn word_term(&mut self, w: &str) -> PResult<Expr> {
        // Function call with parentheses.
        let builtin = ZERO_ARG_FUNCS.contains(&w) || ONE_ARG_FUNCS.contains(&w);
        if matches!(self.tok_at(1), Tok::Sym("("))
            && (builtin || !EXPR_START_WORDS.contains(&w) && constant(w).is_none())
        {
            let name = w.to_string();
            self.advance();
            self.advance();
            let mut args = Vec::new();
            if !self.accept_sym(")") {
                args.push(self.expr()?);
                while self.accept_sym(",") {
                    args.push(self.expr()?);
                }
                if !self.accept_sym(")") {
                    return Err(self.err_here("Expected \")\""));
                }
            }
            return Ok(Expr::Call(name, args));
        }
        if w == "the" {
            return self.the_term();
        }
        if let Some(e) = self.chunk_term()? {
            return Ok(e);
        }
        if self.implicit_the() {
            return self.the_rest();
        }
        if matches!(w, "message" | "msg" | "variable") && self.is_at(1, "watcher") {
            // Debugger windows aren't supported.
            self.pos += 2;
            return Ok(Expr::Call("__menu".into(), vec![]));
        }
        if let Some(obj) = self.object()? {
            return Ok(Expr::Object(obj));
        }
        if w == "selection" {
            self.advance();
            return Ok(Expr::Selection);
        }
        if is_ordinal(w) && matches!(self.word_at(1), Some("menu" | "menuitem")) {
            // `last menuItem of menu "File"`
            self.advance();
            let w = self.word().unwrap_or_default().to_string();
            self.advance();
            let mut args = Vec::new();
            if w == "menuitem" && self.accept("of") {
                self.expect("menu")?;
                args.push(self.term()?);
            }
            return Ok(Expr::Call("__menu".into(), args));
        }
        if w == "window" && self.starts_term_at(1) {
            // Palettes and other windows aren't supported either.
            self.advance();
            return Ok(Expr::Call("__menu".into(), vec![self.term()?]));
        }
        if matches!(w, "menu" | "menuitem") && self.starts_term_at(1) {
            // Menus can't be scripted here; references to them do nothing.
            self.advance();
            let mut args = vec![self.term()?];
            if w == "menuitem" && self.accept("of") {
                self.expect("menu")?;
                args.push(self.term()?);
            }
            return Ok(Expr::Call("__menu".into(), args));
        }
        if let Some(c) = constant(w) {
            self.advance();
            return Ok(Expr::Lit(c.into()));
        }
        if STOP_WORDS.contains(&w) {
            return Err(self.err_here("Expected an expression"));
        }
        let text = self.toks[self.pos.min(self.toks.len() - 1)].text.clone();
        self.advance();
        Ok(Expr::Var(text))
    }

    /// Whether the current word starts a property or function without `the`.
    fn implicit_the(&self) -> bool {
        let n = if self.length_word(0).is_some() && self.word_at(1).is_some() { 1 } else { 0 };
        let Some(w) = self.word_at(n) else { return false };
        let known = PROPERTIES.contains(&w) || ONE_ARG_FUNCS.contains(&w);
        known && (self.is_at(n + 1, "of") || w == "number" && self.is_at(n + 1, "in"))
    }

    fn length_word(&self, n: usize) -> Option<NameLength> {
        Some(match self.word_at(n)? {
            "long" | "english" => NameLength::Long,
            "short" => NameLength::Short,
            "abbr" | "abbrev" | "abbreviated" => NameLength::Abbrev,
            _ => return None,
        })
    }

    fn the_term(&mut self) -> PResult<Expr> {
        // `the first word of x`, `the next card`, `the message box` ...
        if self.word_at(1).is_some_and(|w| {
            is_ordinal(w) || matches!(w, "next" | "prev" | "previous" | "msg" | "message" | "selection")
        }) {
            self.advance();
            if self.accept("selection") {
                return Ok(Expr::Selection);
            }
            let w = self.word().unwrap_or_default().to_string();
            return self.word_term(&w);
        }
        self.advance(); // "the"
        self.the_rest()
    }

    /// A property or function after `the`; HyperTalk also allows `the` to be
    /// left out, as in `visible of field 1` or `number of lines in x`.
    fn the_rest(&mut self) -> PResult<Expr> {
        if self.accept("target") {
            return Ok(Expr::Property {
                name: "name".into(),
                length: NameLength::Default,
                obj: Some(Box::new(ObjSpec::Target)),
            });
        }
        if self.is("number") && (self.is_at(1, "of") || self.is_at(1, "in")) {
            let save = self.pos;
            self.pos += 2;
            if let Some(c) = self.countable()? {
                return Ok(Expr::Number(c));
            }
            self.pos = save;
        }
        let mut length = NameLength::Default;
        if let Some(l) = self.length_word(0)
            && self.word_at(1).is_some()
        {
            length = l;
            self.advance();
        }
        let name = self.symbol()?;
        if self.is("of") || self.is("in") {
            if ONE_ARG_FUNCS.contains(&name.as_str()) {
                self.advance();
                return Ok(Expr::Builtin(name, length, vec![self.term()?]));
            }
            self.advance();
            if self.is("the") && self.is_at(1, "target") {
                self.pos += 2;
                return Ok(Expr::Property { name, length, obj: Some(Box::new(ObjSpec::Target)) });
            }
            let obj = self.term()?;
            let obj = match obj {
                Expr::Object(o) => o,
                other => {
                    // A property of a chunk or a dynamic object reference.
                    let len = match length {
                        NameLength::Default => "",
                        NameLength::Short => "short",
                        NameLength::Long => "long",
                        NameLength::Abbrev => "abbrev",
                    };
                    return Ok(Expr::Call("__propof".into(), vec![Expr::Lit(name), Expr::Lit(len.into()), other]));
                }
            };
            return Ok(Expr::Property { name, length, obj: Some(Box::new(obj)) });
        }
        if ZERO_ARG_FUNCS.contains(&name.as_str()) {
            return Ok(Expr::Builtin(name, length, vec![]));
        }
        Ok(Expr::Property { name, length, obj: None })
    }

    fn countable(&mut self) -> PResult<Option<Countable>> {
        if let Some(k) = self.word().and_then(chunk_kind) {
            self.advance();
            if !(self.accept("of") || self.accept("in")) {
                return Err(self.err_here("Expected \"of\""));
            }
            return Ok(Some(Countable::Chunks(k, Box::new(self.term()?))));
        }
        if self.accept("marked") {
            self.accept_any(&["cards", "cds", "card", "cd"]);
            return Ok(Some(Countable::MarkedCards));
        }
        let w = self.word().unwrap_or_default().to_string();
        let explicit_layer = if is_card_word(&w) || is_cards_word(&w) {
            Some(Layer::Card)
        } else if is_bg_word(&w) {
            Some(Layer::Background)
        } else {
            None
        };
        let next = self.word_at(1).unwrap_or_default().to_string();
        if explicit_layer.is_some() && is_part_word(&next) || explicit_layer.is_none() && is_part_word(&w) {
            if explicit_layer.is_some() {
                self.advance();
            }
            let kw = self.symbol()?;
            let kind = part_kind(&kw);
            let layer = explicit_layer.unwrap_or(match kind {
                Some(PartKind::Field) => Layer::Background,
                _ => Layer::Card,
            });
            let of = if self.accept("of") || self.accept("in") {
                match self.object()? {
                    Some(o) => Some(Box::new(o)),
                    None => return Err(self.err_here("Expected a card or background")),
                }
            } else {
                None
            };
            return Ok(Some(Countable::Parts { kind, layer, of }));
        }
        if is_cards_word(&w) || (is_card_word(&w) && !self.starts_term_at(1)) {
            self.advance();
            if self.is("of") && self.is_at(1, "this") && self.is_at(2, "stack") {
                self.pos += 3;
                return Ok(Some(Countable::Cards(None)));
            }
            if self.accept("of") || self.accept("in") {
                return Ok(Some(Countable::Cards(Some(Box::new(self.term()?)))));
            }
            return Ok(Some(Countable::Cards(None)));
        }
        if matches!(w.as_str(), "backgrounds" | "bkgnds" | "bgs")
            || (is_bg_word(&w) && !self.starts_term_at(1) && !self.is_at(1, "id"))
        {
            self.advance();
            if self.is("of") && self.is_at(1, "this") && self.is_at(2, "stack") {
                self.pos += 3;
            }
            return Ok(Some(Countable::Backgrounds));
        }
        Ok(None)
    }

    fn chunk_term(&mut self) -> PResult<Option<Expr>> {
        let Some(w) = self.word() else { return Ok(None) };
        // `the` was already consumed if present.
        let (kind, start) = if let Some(ord) = ordinal_of(w) {
            let Some(kind) = self.word_at(1).and_then(chunk_kind) else { return Ok(None) };
            self.advance();
            self.advance();
            (kind, ChunkIndex::Ordinal(ord))
        } else if let Some(kind) = chunk_kind(w) {
            if !self.starts_term_at(1) && !matches!(self.tok_at(1), Tok::Sym("-")) {
                return Ok(None);
            }
            self.advance();
            (kind, ChunkIndex::Expr(Box::new(self.expr()?)))
        } else {
            return Ok(None);
        };
        let end = if matches!(start, ChunkIndex::Expr(_)) && self.accept("to") {
            Some(Box::new(self.expr()?))
        } else {
            None
        };
        if !(self.accept("of") || self.accept("in")) {
            return Err(self.err_here("Expected \"of\""));
        }
        let target = self.term()?;
        Ok(Some(Expr::Chunk(Box::new(Chunk { kind, start, end }), Box::new(target))))
    }

    // ----- object references -----

    fn which(&mut self) -> PResult<Which> {
        if self.accept("id") {
            return Ok(Which::Id(Box::new(self.term()?)));
        }
        Ok(Which::Expr(Box::new(self.term()?)))
    }

    /// Try to parse an object reference at the current position.
    fn object(&mut self) -> PResult<Option<ObjSpec>> {
        let save = self.pos;
        let result = self.object_inner();
        if let Ok(None) = result {
            self.pos = save;
        }
        result
    }

    fn object_inner(&mut self) -> PResult<Option<ObjSpec>> {
        let n = if self.is("the") { 1 } else { 0 };
        let Some(w) = self.word_at(n).map(str::to_string) else { return Ok(None) };
        let next = self.word_at(n + 1).unwrap_or_default().to_string();
        match w.as_str() {
            "me" if n == 0 => {
                self.advance();
                return Ok(Some(ObjSpec::Me));
            }
            "target" if n == 0 => {
                self.advance();
                return Ok(Some(ObjSpec::Target));
            }
            "msg" | "message" => {
                self.pos += n + 1;
                self.accept_any(&["box", "window"]);
                return Ok(Some(ObjSpec::MsgBox));
            }
            "hypercard" | "randomcard" if n == 0 => {
                self.advance();
                return Ok(Some(ObjSpec::HyperCard));
            }
            "stack" if n == 0 => {
                self.advance();
                if self.starts_term() {
                    return Ok(Some(ObjSpec::Stack(Some(Box::new(self.term()?)))));
                }
                return Ok(Some(ObjSpec::Stack(None)));
            }
            "this" if n == 0 => {
                if next == "stack" {
                    self.pos += 2;
                    return Ok(Some(ObjSpec::Stack(None)));
                }
                if is_card_word(&next) {
                    self.pos += 2;
                    return Ok(Some(ObjSpec::Card(self.card_suffix(CardSpec::Position(Position::This))?)));
                }
                if is_bg_word(&next) {
                    self.pos += 2;
                    return Ok(Some(ObjSpec::Background(BgSpec::Position(Position::This))));
                }
                return Ok(None);
            }
            "next" | "prev" | "previous" => {
                let pos = if w == "next" { Position::Next } else { Position::Prev };
                if is_card_word(&next) {
                    self.pos += n + 2;
                    return Ok(Some(ObjSpec::Card(self.card_suffix(CardSpec::Position(pos))?)));
                }
                if next == "marked" && self.word_at(n + 2).is_some_and(is_card_word) {
                    self.pos += n + 3;
                    return Ok(Some(ObjSpec::Card(CardSpec::MarkedPosition(pos))));
                }
                if is_bg_word(&next) {
                    self.pos += n + 2;
                    return Ok(Some(ObjSpec::Background(BgSpec::Position(pos))));
                }
                return Ok(None);
            }
            "recent" if is_card_word(&next) => {
                self.pos += n + 2;
                return Ok(Some(ObjSpec::Card(CardSpec::Recent)));
            }
            "marked" if is_card_word(&next) => {
                self.pos += n + 2;
                return Ok(Some(ObjSpec::Card(CardSpec::Marked(self.which()?))));
            }
            _ => {}
        }
        if let Some(ord) = ordinal_of(&w) {
            self.pos += n + 1;
            let w2 = self.word().unwrap_or_default().to_string();
            let w3 = self.word_at(1).unwrap_or_default().to_string();
            if w2 == "marked" && is_card_word(&w3) {
                self.pos += 2;
                return Ok(Some(ObjSpec::Card(CardSpec::Marked(Which::Ordinal(ord)))));
            }
            if (is_card_word(&w2) || is_bg_word(&w2)) && is_part_word(&w3) {
                let layer = if is_card_word(&w2) { Layer::Card } else { Layer::Background };
                self.pos += 2;
                let kind = part_kind(&w3);
                return Ok(Some(self.part_suffix(kind, Some(layer), Which::Ordinal(ord))?));
            }
            if is_card_word(&w2) {
                self.advance();
                return Ok(Some(ObjSpec::Card(self.card_suffix(CardSpec::Which(Which::Ordinal(ord)))?)));
            }
            if is_bg_word(&w2) {
                self.advance();
                return Ok(Some(ObjSpec::Background(BgSpec::Which(Which::Ordinal(ord)))));
            }
            if is_part_word(&w2) {
                self.advance();
                return Ok(Some(self.part_suffix(part_kind(&w2), None, Which::Ordinal(ord))?));
            }
            return Ok(None);
        }
        if n == 1 {
            return Ok(None);
        }
        if is_card_word(&w) || is_bg_word(&w) {
            let layer = if is_card_word(&w) { Layer::Card } else { Layer::Background };
            if is_part_word(&next) {
                self.pos += 2;
                let which = self.which()?;
                return Ok(Some(self.part_suffix(part_kind(&next), Some(layer), which)?));
            }
            if layer == Layer::Card && next == "window" {
                // The card window has the card's size; treat it as the stack.
                self.pos += 2;
                return Ok(Some(ObjSpec::Stack(None)));
            }
            if matches!(next.as_str(), "picture" | "pict" | "window") {
                return Ok(None);
            }
            self.advance();
            let which = if self.is("id") || self.starts_term() {
                Some(self.which()?)
            } else {
                None
            };
            if layer == Layer::Card {
                let spec = match which {
                    Some(w) => CardSpec::Which(w),
                    None => CardSpec::Position(Position::This),
                };
                return Ok(Some(ObjSpec::Card(self.card_suffix(spec)?)));
            }
            self.skip_of_this_stack();
            return Ok(Some(ObjSpec::Background(match which {
                Some(w) => BgSpec::Which(w),
                None => BgSpec::Position(Position::This),
            })));
        }
        if let Some(kind) = part_kind(&w) {
            self.advance();
            if !self.is("id") && !self.starts_term() {
                return Err(self.err_here(&format!("Expected a {} number or name", kind.name())));
            }
            let which = self.which()?;
            return Ok(Some(self.part_suffix(Some(kind), None, which)?));
        }
        if w == "part" && self.starts_term_at(1) {
            self.advance();
            let which = self.which()?;
            return Ok(Some(self.part_suffix(None, None, which)?));
        }
        Ok(None)
    }

    /// Skip `of this stack` or `of stack "Name"`; only the open stack can be addressed.
    fn skip_of_this_stack(&mut self) {
        if self.is("of") && self.is_at(1, "this") && self.is_at(2, "stack") {
            self.pos += 3;
        } else if self.is("of") && self.is_at(1, "stack") && self.starts_term_at(2) {
            self.pos += 2;
            let _ = self.term();
        }
    }

    fn card_spec_follows_of(&self) -> bool {
        if !(self.is("of") || self.is("in")) {
            return false;
        }
        let w1 = self.word_at(1).unwrap_or_default();
        let w2 = self.word_at(2).unwrap_or_default();
        is_card_word(w1)
            && !is_part_word(w2)
            && w2 != "picture"
            || (matches!(w1, "this" | "next" | "prev" | "previous" | "recent" | "marked") || is_ordinal(w1))
                && is_card_word(w2)
            || w1 == "the" && (is_ordinal(w2) || matches!(w2, "next" | "prev" | "previous"))
    }

    fn card_suffix(&mut self, spec: CardSpec) -> PResult<CardSpec> {
        let bg_at = |n: usize| self.word_at(n).is_some_and(is_bg_word);
        let qualifier = |n: usize| {
            self.word_at(n).is_some_and(|w| is_ordinal(w) || matches!(w, "this" | "next" | "prev" | "previous"))
        };
        if (self.is("of") || self.is("in"))
            && (bg_at(1) || qualifier(1) && bg_at(2) || self.is_at(1, "the") && qualifier(2) && bg_at(3))
        {
            self.advance();
            if let Some(ObjSpec::Background(bg)) = self.object()? {
                return Ok(CardSpec::OfBackground(Box::new(spec), Box::new(bg)));
            }
            return Err(self.err_here("Expected a background"));
        }
        self.skip_of_this_stack();
        Ok(spec)
    }

    fn part_suffix(&mut self, kind: Option<PartKind>, layer: Option<Layer>, which: Which) -> PResult<ObjSpec> {
        let card = if self.card_spec_follows_of() {
            self.advance();
            match self.object()? {
                Some(ObjSpec::Card(c)) => Some(Box::new(c)),
                _ => return Err(self.err_here("Expected a card")),
            }
        } else {
            None
        };
        Ok(ObjSpec::Part { kind, layer, which, card })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stmt(src: &str) -> StmtKind {
        let mut v = parse_statements(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        assert_eq!(v.len(), 1, "{src}");
        v.remove(0).kind
    }

    #[test]
    fn parses_handlers() {
        let s = parse_script(
            "on mouseUp\n  put \"hi\" into field 1\n  if x > 2 then\n    beep\n  else\n    answer \"no\"\n  end if\nend mouseUp\n\nfunction sq n\n  return n * n\nend sq",
        )
        .unwrap();
        assert!(s.handlers.contains_key("mouseup"));
        assert_eq!(s.functions["sq"].params, vec!["n"]);
    }

    #[test]
    fn parses_commands() {
        for src in [
            "put the number of cards into n",
            "put word 2 of line 3 of field \"Data\" after it",
            "set the hilite of button \"Check\" to true",
            "set the rect of card button 1 to 10,10,100,40",
            "go to next card",
            "go to card \"Index\"",
            "go back",
            "go to the last card",
            "answer \"Continue?\" with \"No\" or \"Yes\"",
            "ask \"Your name?\" with \"Nobody\"",
            "repeat with i = 1 to 10\n  add i to total\nend repeat",
            "wait 2 seconds",
            "visual effect dissolve slow",
            "send \"mouseUp\" to button 3 of card 2",
            "sort lines of field 1 descending numeric",
            "sort cards by field \"Name\"",
            "find \"apple\" in field \"Fruit\"",
            "delete char 1 to 3 of x",
            "choose browse tool",
            "if there is a card \"Foo\" then go to card \"Foo\" else beep",
            "put \"a\" & return & \"b\" into msg",
            "get the long name of me",
            "put the mouseLoc into loc",
            "put random(6) + the abs of -3 into n",
            "if x is not in y and y is a number then beep",
            "put the number of words in field 2",
            "set the itemDelimiter to tab",
            "lock screen",
            "unlock screen with visual effect wipe left fast",
            "doMenu \"New Card\"",
            "myHandler 1, \"two\", the date",
            "put the first char of the last word of x",
            "hide card field \"Secret\"",
            "show msg",
            "push card",
            "pop card",
            "put the name of the target",
            "put the short name of bg field 2 of card 3",
            "get the value of \"1+2\"",
        ] {
            stmt(src);
        }
    }

    #[test]
    fn else_if_chain() {
        let src = "on t\nif a then\n  b\nelse if c then\n  d\nelse\n  e\nend if\nend t";
        parse_script(src).unwrap();
        let src = "on t\nif a then b\nelse c\nput 1\nend t";
        let s = parse_script(src).unwrap();
        assert_eq!(s.handlers["t"].body.len(), 2);
    }

    #[test]
    fn precedence() {
        let StmtKind::Cmd { cmd: Cmd::Put { value, .. }, .. } = stmt("put 1 + 2 * 3 & \"x\"") else { panic!() };
        let Expr::Binary(BinOp::Concat, lhs, _) = value else { panic!("{value:?}") };
        assert!(matches!(*lhs, Expr::Binary(BinOp::Add, _, _)));
    }
}
