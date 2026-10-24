//! `print card`, `print all cards`, `print "text"`, `open printing` and
//! `close printing`. Documents are made as PDFs and opened in the system's
//! viewer, which prints them.

use crate::hypertalk::ast::PrintWhat;
use crate::hypertalk::value::parse_point;
use crate::model::{Id, Rect};
use crate::print;

use super::interp::{Interp, R, err};

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
                let text = self.eval(e)?;
                let name = format!("{} text", self.w().stack.name);
                return self.emit(&name, print::text_pdf(&text));
            }
        };
        let alone = ids.len() == 1;
        self.print_cards(ids, alone)
    }

    /// Print cards, or add them to the document `open printing` started.
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

    fn emit(&mut self, name: &str, pdf: Vec<u8>) -> R<()> {
        let path = print::save(name, &pdf).or_else(err)?;
        if self.open_documents {
            print::open_in_viewer(&path).or_else(err)?;
        }
        self.printed.push(path);
        Ok(())
    }
}
