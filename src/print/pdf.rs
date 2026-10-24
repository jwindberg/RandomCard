//! Just enough of PDF to print cards: pages of vector drawing, the
//! standard fonts (which viewers supply, so nothing is embedded) and 1-bit
//! images. Units are points, 72 to the inch: one HyperCard pixel each.

use std::collections::BTreeMap;
use std::fmt::Write;

/// US Letter, in points.
pub const PAGE_WIDTH: f32 = 612.0;
pub const PAGE_HEIGHT: f32 = 792.0;

pub struct Document {
    /// Object bodies; object n is at index n - 1. `None` until written.
    objects: Vec<Option<Vec<u8>>>,
    pages: Vec<usize>,
    pages_id: usize,
    resources_id: usize,
    /// Font resource names by PDF base font, e.g. "Helvetica-Bold" → "F2".
    fonts: BTreeMap<&'static str, String>,
    /// Image resource names and their objects.
    images: Vec<(String, usize)>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Document {
        let mut d = Document {
            objects: Vec::new(),
            pages: Vec::new(),
            pages_id: 0,
            resources_id: 0,
            fonts: BTreeMap::new(),
            images: Vec::new(),
        };
        d.pages_id = d.reserve();
        d.resources_id = d.reserve();
        d
    }

    fn reserve(&mut self) -> usize {
        self.objects.push(None);
        self.objects.len()
    }

    fn add(&mut self, body: Vec<u8>) -> usize {
        self.objects.push(Some(body));
        self.objects.len()
    }

    /// A compressed stream object.
    fn add_stream(&mut self, dict: &str, data: &[u8]) -> usize {
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
        let mut body = format!("<< {dict} /Filter /FlateDecode /Length {} >>\nstream\n", packed.len()).into_bytes();
        body.extend_from_slice(&packed);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    /// The resource name of a standard font.
    pub fn font(&mut self, base: &'static str) -> String {
        let n = self.fonts.len() + 1;
        self.fonts.entry(base).or_insert_with(|| format!("F{n}")).clone()
    }

    /// A 1-bit image mask: where a bit is 1, drawing paints the fill color.
    /// Rows are packed most significant bit first, padded to whole bytes.
    pub fn image_mask(&mut self, width: u32, height: u32, bits: &[u8]) -> String {
        let dict = format!(
            "/Type /XObject /Subtype /Image /Width {width} /Height {height} /ImageMask true /BitsPerComponent 1 /Decode [1 0]"
        );
        let id = self.add_stream(&dict, bits);
        let name = format!("Im{}", self.images.len() + 1);
        self.images.push((name.clone(), id));
        name
    }

    pub fn add_page(&mut self, content: &str) {
        let content_id = self.add_stream("", content.as_bytes());
        let page = format!(
            "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] /Resources {} 0 R /Contents {content_id} 0 R >>",
            self.pages_id, self.resources_id
        );
        let id = self.add(page.into_bytes());
        self.pages.push(id);
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn finish(mut self) -> Vec<u8> {
        let mut fonts = String::new();
        let font_list: Vec<(&'static str, String)> = self.fonts.iter().map(|(b, n)| (*b, n.clone())).collect();
        for (base, name) in font_list {
            let id = self.add(
                format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>").into_bytes(),
            );
            let _ = write!(fonts, "/{name} {id} 0 R ");
        }
        let images: String = self.images.iter().map(|(n, id)| format!("/{n} {id} 0 R ")).collect();
        self.objects[self.resources_id - 1] = Some(format!("<< /Font << {fonts}>> /XObject << {images}>> >>").into_bytes());
        let kids: String = self.pages.iter().map(|p| format!("{p} 0 R ")).collect();
        self.objects[self.pages_id - 1] =
            Some(format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", self.pages.len()).into_bytes());
        let catalog = self.add(format!("<< /Type /Catalog /Pages {} 0 R >>", self.pages_id).into_bytes());

        let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (i, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body.as_deref().unwrap_or(b"null"));
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", self.objects.len() + 1);
        for o in offsets {
            let _ = writeln!(table, "{o:010} 00000 n ");
        }
        let _ = write!(
            table,
            "trailer\n<< /Size {} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.objects.len() + 1
        );
        out.extend_from_slice(table.as_bytes());
        out
    }
}

/// A PDF string literal in WinAnsi encoding, the standard fonts' encoding.
pub fn text_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('(');
    for c in s.chars() {
        let b = win_ansi(c);
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out.push(')');
    out
}

fn win_ansi(c: char) -> u8 {
    match c as u32 {
        0x20..=0x7e | 0xa0..=0xff => c as u8,
        _ => match c {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Œ' => 0x8c,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'œ' => 0x9c,
            'Ÿ' => 0x9f,
            // Near enough, where the standard fonts have nothing better.
            '◀' | '◁' | '←' => b'<',
            '▶' | '▷' | '→' => b'>',
            '▲' | '↑' => b'^',
            '▼' | '↓' => b'v',
            _ => b'?',
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_document() {
        let mut d = Document::new();
        let f = d.font("Helvetica");
        d.add_page(&format!("BT /{f} 12 Tf 72 720 Td {} Tj ET", text_literal("Hello (world) café")));
        let pdf = d.finish();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Count 1") && text.contains("/BaseFont /Helvetica"));
        assert!(text.trim_end().ends_with("%%EOF"));
        assert_eq!(text_literal("a(b)é"), "(a\\(b\\)\\351)");
    }
}
