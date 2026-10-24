//! Built-in HyperTalk functions: `the date`, `random(6)`, `the mouseLoc`, ...

use crate::hypertalk::ast::NameLength;
use crate::hypertalk::value::*;

use super::dates;
use super::world::{FieldRange, World, line_number};
use super::interp::{Interp, R, err, to_int, to_number};

/// Arguments to list functions may be given separately or as one comma list.
fn number_list(args: &[String]) -> R<Vec<f64>> {
    let mut out = Vec::new();
    for a in args {
        for piece in a.split(',') {
            if piece.trim().is_empty() {
                continue;
            }
            out.push(to_number(piece)?);
        }
    }
    Ok(out)
}

fn up_down(b: bool) -> String {
    if b { "down".into() } else { "up".into() }
}

impl Interp {
    pub fn builtin(&mut self, name: &str, len: NameLength, args: Vec<String>) -> R<String> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        let num = |i: usize| to_number(&args.get(i).cloned().unwrap_or_default());
        let need = |n: usize| -> R<()> {
            if args.len() < n {
                return err(format!("The {name} function needs {n} argument(s)"));
            }
            Ok(())
        };
        Ok(match name {
            // Math
            "abs" => self.fmt(num(0)?.abs()),
            "round" => self.fmt(round_half_even(num(0)?)),
            "trunc" => self.fmt(num(0)?.trunc()),
            "sqrt" => self.fmt(num(0)?.sqrt()),
            "sin" => self.fmt(num(0)?.sin()),
            "cos" => self.fmt(num(0)?.cos()),
            "tan" => self.fmt(num(0)?.tan()),
            "atan" => self.fmt(num(0)?.atan()),
            "exp" => self.fmt(num(0)?.exp()),
            "exp1" => self.fmt(num(0)?.exp_m1()),
            "exp2" => self.fmt(num(0)?.exp2()),
            "ln" => self.fmt(num(0)?.ln()),
            "ln1" => self.fmt(num(0)?.ln_1p()),
            "log2" => self.fmt(num(0)?.log2()),
            "random" => {
                let n = to_int(&arg(0))?;
                if n < 1 {
                    return err("random needs a positive number");
                }
                rand::random_range(1..=n).to_string()
            }
            "min" | "max" | "sum" | "average" => {
                let v = number_list(&args)?;
                if v.is_empty() {
                    return Ok(if name == "sum" { "0".into() } else { String::new() });
                }
                let r = match name {
                    "min" => v.iter().copied().fold(f64::INFINITY, f64::min),
                    "max" => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    "sum" => v.iter().sum(),
                    _ => v.iter().sum::<f64>() / v.len() as f64,
                };
                self.fmt(r)
            }
            "annuity" => {
                need(2)?;
                let (r, n) = (num(0)?, num(1)?);
                self.fmt(if r == 0.0 { n } else { (1.0 - (1.0 + r).powf(-n)) / r })
            }
            "compound" => {
                need(2)?;
                self.fmt((1.0 + num(0)?).powf(num(1)?))
            }

            // Text
            "length" => arg(0).chars().count().to_string(),
            "chartonum" => arg(0).chars().next().map(|c| c as u32).unwrap_or(0).to_string(),
            "numtochar" => {
                let n = to_int(&arg(0))?;
                char::from_u32(n as u32).map(String::from).unwrap_or_default()
            }
            "offset" => {
                need(2)?;
                let (needle, hay) = (arg(0).to_lowercase(), arg(1).to_lowercase());
                match hay.find(&needle) {
                    Some(i) if !needle.is_empty() => (hay[..i].chars().count() + 1).to_string(),
                    _ => "0".into(),
                }
            }
            "value" => return self.value_of(&arg(0)),

            // Dates and times
            "date" => dates::format_date(dates::now(), len),
            "time" => dates::format_time(dates::now(), len),
            "seconds" | "secs" | "sec" => dates::to_seconds(dates::now()).to_string(),
            "ticks" => ((self.started.elapsed().as_secs_f64() * 60.0) as u64).to_string(),
            "dateitems" => dates::date_items(dates::now()),

            // Mouse and keyboard
            "mouse" => up_down(self.w().mouse.down),
            "mouseloc" => {
                let w = self.w();
                format!("{},{}", w.mouse.x, w.mouse.y)
            }
            "mouseh" => self.w().mouse.x.to_string(),
            "mousev" => self.w().mouse.y.to_string(),
            "clickloc" => {
                let w = self.w();
                format!("{},{}", w.mouse.click_x, w.mouse.click_y)
            }
            "clickh" => self.w().mouse.click_x.to_string(),
            "clickv" => self.w().mouse.click_y.to_string(),
            "mouseclick" => {
                let mut w = self.w();
                let c = w.mouse.clicked;
                w.mouse.clicked = false;
                bool_str(c)
            }
            "shiftkey" => up_down(self.w().mouse.shift),
            "optionkey" => up_down(self.w().mouse.option),
            "commandkey" | "cmdkey" | "controlkey" | "ctrlkey" => up_down(self.w().mouse.command),

            // Handlers
            "result" => self.result.clone(),
            "paramcount" => self.frame().params.len().to_string(),
            "params" => {
                let f = self.frame();
                let args: Vec<String> = f.params.iter().map(|p| format!("\"{p}\"")).collect();
                if args.is_empty() { f.message.clone() } else { format!("{} {}", f.message, args.join(",")) }
            }
            "param" => {
                let n = to_int(&arg(0))?;
                let f = self.frame();
                if n == 0 { f.message.clone() } else { f.params.get((n - 1) as usize).cloned().unwrap_or_default() }
            }
            "target" => {
                let t = self.frame().target;
                self.name_of(t, NameLength::Default)
            }

            // Environment
            "tool" => format!("{} tool", self.w().tool.name()),
            "version" => match len {
                NameLength::Long => "02418000".into(),
                _ => "2.41".into(),
            },
            "foundtext" | "foundchunk" | "foundline" | "foundfield" => {
                let w = self.w();
                let found = w.found.as_ref().filter(|f| f.card == w.card_id);
                found.map(|f| range_info(&w, f, &name[5..])).unwrap_or_default()
            }
            "selectedtext" | "selectedchunk" | "selectedline" | "selectedfield" => {
                let w = self.w();
                w.current_selection().map(|r| range_info(&w, r, &name[8..])).unwrap_or_default()
            }
            "selectedloc" => {
                let w = self.w();
                match (w.current_selection(), w.selection_loc) {
                    (Some(_), Some((x, y))) => format!("{x},{y}"),
                    _ => String::new(),
                }
            }
            "clicktext" | "clickchunk" | "clickline" => {
                let w = self.w();
                let click = w.click.as_ref().filter(|c| c.card == w.card_id);
                match click {
                    // Clicking a space or past the text has no word, but still a line.
                    Some(c) if c.start < c.end || name == "clickline" => range_info(&w, c, &name[5..]),
                    _ => String::new(),
                }
            }
            "menus" => self.w().menus.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join("\n"),
            "stacks" => {
                let w = self.w();
                w.stack.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| w.stack.name.clone())
            }
            "windows" => "card window\nmessage box".into(),
            "sound" => self.audio.current(),
            "speech" => "done".into(),
            "voices" => String::new(),
            "screenrect" => "0,0,1920,1080".into(),
            "systemversion" | "osversion" => "1.0".into(),
            "osname" => std::env::consts::OS.into(),
            "diskspace" | "heapspace" => "1000000000".into(),
            "__fileexists" => bool_str(std::path::Path::new(&arg(0)).exists()),
            _ => return err(format!("No such function \"{name}\"")),
        })
    }
}

/// `the foundText`, `selectedChunk`, `clickLine` and friends: the text of a
/// range, or where it is ("char 3 to 7 of card field 1", "line 2 of …").
fn range_info(w: &World, r: &FieldRange, which: &str) -> String {
    let field = w.range_field_name(r);
    match which {
        "text" => w.range_text(r),
        "field" => field,
        "chunk" => format!("char {} to {} of {field}", r.start + 1, r.end),
        _ => {
            let text = w.stack.field_text(r.card, r.layer, r.field);
            let first = line_number(&text, r.start);
            let last = line_number(&text, r.end.max(r.start + 1) - 1).max(first);
            if last > first { format!("line {first} to {last} of {field}") } else { format!("line {first} of {field}") }
        }
    }
}
