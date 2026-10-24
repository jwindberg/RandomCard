//! Printing: cards and text made into PDF documents, which open in the
//! system's viewer to print or save. HyperCard printed to the printer
//! directly; a PDF does the same job on any system, with no printer setup.

mod card;
mod layout;
pub mod pdf;

use std::path::PathBuf;

use crate::model::{Id, Rect, Stack, TextAlign};
use card::Canvas;
use pdf::{Document, PAGE_HEIGHT, PAGE_WIDTH};

const MARGIN: f32 = 36.0;
const GAP: f32 = 18.0;

/// A document of cards at full size (smaller only if a card won't fit),
/// as many to a page as fit; `alone` puts each on a page of its own. With
/// `area`, only that part of each card is printed.
pub fn cards_pdf(stack: &Stack, cards: &[Id], alone: bool, area: Option<Rect>) -> Vec<u8> {
    let area = area.unwrap_or(Rect::new(0, 0, stack.width as i32, stack.height as i32));
    let (w, h) = ((area.right - area.left).max(1) as f32, (area.bottom - area.top).max(1) as f32);
    let scale = (1.0f32).min((PAGE_WIDTH - 2.0 * MARGIN) / w).min((PAGE_HEIGHT - 2.0 * MARGIN) / h);
    let per_page = if alone { 1 } else { (((PAGE_HEIGHT - 2.0 * MARGIN + GAP) / (h * scale + GAP)) as usize).max(1) };
    let mut doc = Document::new();
    for page in cards.chunks(per_page) {
        let mut c = Canvas { doc: &mut doc, ops: String::new() };
        for (i, &id) in page.iter().enumerate() {
            let x = ((PAGE_WIDTH - w * scale) / 2.0).round();
            let top = MARGIN + i as f32 * (h * scale + GAP);
            // Card coordinates: y down from the card's top left.
            c.ops.push_str(&format!("q {scale} 0 0 -{scale} {x} {} cm\n", PAGE_HEIGHT - top));
            c.ops.push_str(&format!("0 0 {w} {h} re W n 1 0 0 1 {} {} cm\n", -area.left, -area.top));
            card::draw_card(&mut c, stack, id);
            c.ops.push_str(&format!("1 0 0 1 {} {} cm\n", area.left, area.top));
            // A hairline round the card's edge.
            c.ops.push_str(&format!("0.6 G 0.5 w -0.25 -0.25 {} {} re S\nQ\n", w + 0.5, h + 0.5));
        }
        let ops = std::mem::take(&mut c.ops);
        doc.add_page(&ops);
    }
    doc.finish()
}

/// A document of plain text (`print "..."`, `print field 1`), wrapped and
/// paginated.
pub fn text_pdf(text: &str) -> Vec<u8> {
    let setting =
        layout::Setting { font: "Geneva", size: 10, style: "plain", align: TextAlign::Left, wrap: true, line_height: None };
    let lines = layout::layout(text, &[], &setting, PAGE_WIDTH - 2.0 * MARGIN);
    let mut doc = Document::new();
    let page_height = PAGE_HEIGHT - 2.0 * MARGIN;
    let mut start = 0;
    while start < lines.len() || doc.page_count() == 0 {
        let first_top = lines.get(start).map_or(0.0, |l| l.top);
        let end = (start..lines.len()).find(|&i| lines[i].top + lines[i].height - first_top > page_height).unwrap_or(lines.len());
        let end = end.max(start + 1).min(lines.len());
        let page: Vec<layout::Line> = lines[start..end]
            .iter()
            .map(|l| layout::Line { top: l.top - first_top, baseline: l.baseline - first_top, ..l.clone() })
            .collect();
        let mut c = Canvas { doc: &mut doc, ops: format!("q 1 0 0 -1 {MARGIN} {} cm\n", PAGE_HEIGHT - MARGIN) };
        c.text_lines(&page);
        c.ops.push_str("Q\n");
        let ops = std::mem::take(&mut c.ops);
        doc.add_page(&ops);
        start = end;
    }
    doc.finish()
}

/// Write a document where it can be found again, named after what it holds.
pub fn save(name: &str, pdf: &[u8]) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("RandomCard");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Can't print: {e}"))?;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let path = dir.join(format!("{}.pdf", safe.trim()));
    std::fs::write(&path, pdf).map_err(|e| format!("Can't print: {e}"))?;
    Ok(path)
}

/// Open a document in the system's viewer, to print it from there.
pub fn open_in_viewer(path: &std::path::Path) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(path).spawn().map(|_| ()).map_err(|e| format!("Can't open {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_cards_and_text() {
        let s = Stack::demo();
        let ids: Vec<Id> = s.cards.iter().map(|c| c.id).collect();
        let pdf = String::from_utf8_lossy(&cards_pdf(&s, &ids, false, None)).to_string();
        // Two 512×342 cards fit on a Letter page.
        assert!(pdf.contains(&format!("/Count {}", ids.len().div_ceil(2))), "{}", &pdf[pdf.len() - 300..]);
        assert!(pdf.contains("/BaseFont /Helvetica"));
        let one = String::from_utf8_lossy(&cards_pdf(&s, &ids[..1], true, None)).to_string();
        assert!(one.contains("/Count 1"));
        let long = "word ".repeat(5000);
        let text = String::from_utf8_lossy(&text_pdf(&long)).to_string();
        assert!(!text.contains("/Count 1 ") && text.contains("/Count "));
    }
}
