//! `print card`, `print all cards`, `print "text"`, `open printing` and
//! `close printing`. Documents are made as PDFs and opened in the system's
//! viewer, which prints them.

use crate::hypertalk::ast::{Expr, ObjSpec, PrintWhat};
use crate::hypertalk::value::parse_point;
use crate::model::{Id, PartKind, Rect};
use crate::print;

use super::interp::{Interp, R, err};
use super::world::ObjRef;

impl Interp {
    pub(super) fn print(&mut self, what: &PrintWhat) -> R<()> {
        let ids: Vec<Id> = match what {
            PrintWhat::Card { card, area } => {
                let id = match card {
                    Some(spec) => self.resolve_card(spec)?,
                    None => self.w().card_id,
                };
                if let Some((from, to)) = area {
                    let (a, b) = (self.eval(from)?, self.eval(to)?);
                    let (Some(a), Some(b)) = (parse_point(&a), parse_point(&b)) else {
                        return err("Expected points after print card from … to …");
                    };
                    let rect = Rect::new(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
                    let (name, pdf) = {
                        let w = self.w();
                        (format!("{} card", w.stack.name), print::cards_pdf(&w.stack, &[id], true, Some(rect)))
                    };
                    return self.emit(&name, pdf);
                }
                vec![id]
            }
            PrintWhat::AllCards => self.w().stack.cards.iter().map(|c| c.id).collect(),
            PrintWhat::MarkedCards => self.w().stack.cards.iter().filter(|c| c.marked).map(|c| c.id).collect(),
            PrintWhat::Cards(n) => {
                let Ok(n) = self.eval(n)?.trim().parse::<usize>() else { return err("Expected a number of cards") };
                let w = self.w();
                w.stack.cards.iter().cycle().skip(w.card_index()).take(n.min(w.stack.cards.len())).map(|c| c.id).collect()
            }
            PrintWhat::Text(e) => {
                // `print field 1` prints the field's text in its own font.
                if matches!(e, Expr::Object(ObjSpec::Part { kind: Some(PartKind::Field), .. }))
                    && let Ok(obj @ ObjRef::Part { .. }) = self.resolve_dynamic(e)
                {
                    return self.print_field(obj);
                }
                let text = self.eval(e)?;
                let name = format!("{} text", self.w().stack.name);
                return self.emit(&name, print::text_pdf(&text));
            }
        };
        let alone = ids.len() == 1;
        self.print_cards(ids, alone)
    }

    /// Print cards, or add them to the document `open printing` started.
    /// Print a field's text in its font, size and style (Print Field…).
    pub(super) fn print_field(&mut self, obj: ObjRef) -> R<()> {
        let ObjRef::Part { layer, id, card } = obj else { return err("Can only print a field") };
        let (text, font, size, style, name) = {
            let w = self.w();
            let Some(p) = w.stack.part(card, layer, id) else { return err("No such field") };
            (w.stack.field_text(card, layer, id), p.text_font.clone(), p.text_size, p.text_style.clone(), p.name.clone())
        };
        let doc = format!("{} {}", self.w().stack.name, if name.is_empty() { "field".to_string() } else { name });
        self.emit(&doc, print::text_pdf_in(&text, &font, size.max(1), &style))
    }

    pub(super) fn print_cards(&mut self, ids: Vec<Id>, alone: bool) -> R<()> {
        if let Some(job) = &mut self.print_job {
            job.extend(ids);
            return Ok(());
        }
        if ids.is_empty() {
            return Ok(());
        }
        let (name, pdf) = {
            let w = self.w();
            let what = if ids.len() == 1 { "card" } else { "cards" };
            (format!("{} {what}", w.stack.name), print::cards_pdf(&w.stack, &ids, alone, None))
        };
        self.emit(&name, pdf)
    }

    pub(super) fn close_printing(&mut self) -> R<()> {
        match self.print_job.take() {
            Some(ids) if !ids.is_empty() => self.print_cards(ids, false),
            _ => Ok(()),
        }
    }

    pub(super) fn emit(&mut self, name: &str, pdf: Vec<u8>) -> R<()> {
        let path = print::save(name, &pdf).or_else(err)?;
        if self.open_documents {
            print::open_in_viewer(&path).or_else(err)?;
        }
        self.printed.push(path);
        Ok(())
    }
}
