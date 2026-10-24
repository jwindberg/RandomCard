//! Importing original HyperCard 2.x stack files.
//!
//! A stack's data fork is a sequence of big-endian blocks, each with a 16-byte
//! header (size, type, id, filler): STAK (the stack), LIST and PAGE (card
//! order), BKGD and CARD (layers with their parts and contents), BMAP
//! (WOBA-compressed pictures), FTBL (font names) and so on. The layouts here
//! follow WyldCard's stack reader and Pierre Lorenzi's HyperCardPreview.
//! Button icons and sounds come from the stack's resource fork when one can
//! be found; other resources (XCMDs, color) aren't used.

mod resources;
mod woba;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::model::{
    Background, Card, Id, Part, PartKind, PartStyle, Picture, Rect, Stack, TextAlign, DEFAULT_HEIGHT, DEFAULT_WIDTH,
};
use woba::QdRect;

const HEADER: usize = 16;
const MACBINARY_HEADER: usize = 128;

/// Reported when a private-access HyperCard stack needs its password, and
/// when the password given is wrong.
pub const NEEDS_PASSWORD: &str = "This stack is password protected.";
pub const WRONG_PASSWORD: &str = "That isn't the stack's password.";

/// Open a RandomCard stack or an original HyperCard stack, whichever `path`
/// is. Private-access HyperCard stacks need their password to import.
pub fn load_any(path: &Path, password: Option<&str>) -> Result<Stack, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
    if is_hypercard_stack(&bytes) {
        import_bytes(&bytes, path, password)
    } else {
        Stack::load(path)
    }
}

/// Whether the bytes look like a HyperCard stack, optionally MacBinary-wrapped.
pub fn is_hypercard_stack(bytes: &[u8]) -> bool {
    data_fork(bytes).is_some()
}

/// MacBinary: a 128-byte header (type 'STAK' at 65), the data fork, then the resource fork.
fn is_macbinary(bytes: &[u8]) -> bool {
    bytes.len() > MACBINARY_HEADER
        && bytes[0] == 0
        && bytes.get(65..69) == Some(b"STAK")
        && bytes.get(MACBINARY_HEADER + 4..MACBINARY_HEADER + 8) == Some(b"STAK")
}

fn data_fork(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.get(4..8) == Some(b"STAK") {
        return Some(bytes);
    }
    if let Some(data) = resources::apple_entry(bytes, 1).filter(|d| d.get(4..8) == Some(b"STAK")) {
        return Some(data);
    }
    if is_macbinary(bytes) {
        let len = u32::from_be_bytes(bytes[83..87].try_into().ok()?) as usize;
        return bytes.get(MACBINARY_HEADER..MACBINARY_HEADER + len);
    }
    None
}

pub fn import_bytes(bytes: &[u8], path: &Path, password: Option<&str>) -> Result<Stack, String> {
    let data = data_fork(bytes).ok_or("This is not a HyperCard stack.")?;
    let blocks = read_blocks(data)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Imported".into());
    let mut stack = convert(&blocks, &name, password)?;
    if let Some(fork) = resources::find_resource_fork(path, bytes) {
        stack.icons = resources::icons(&fork);
        stack.sounds = resources::sounds(&fork);
    }
    add_chain_resources(&mut stack, path);
    stack.imported_from = Some(path.to_path_buf());
    Ok(stack)
}

/// Fetch icons the stack's buttons use, and sounds its scripts name, from
/// further along the resource chain when the stack's own fork lacks them.
fn add_chain_resources(stack: &mut Stack, path: &Path) {
    let all_parts = stack.cards.iter().flat_map(|c| &c.parts).chain(stack.backgrounds.iter().flat_map(|b| &b.parts));
    let mut missing_icons: HashSet<i32> = all_parts.map(|p| p.icon).filter(|i| *i != 0).collect();
    missing_icons.retain(|i| !stack.icons.contains_key(i));
    let scripts = all_scripts(stack).to_lowercase();
    let mut have_sounds: HashSet<String> = stack.sounds.values().map(|s| s.name.to_lowercase()).collect();
    for fork in resources::chain_forks(path) {
        for (id, icon) in resources::icons(&fork) {
            if missing_icons.remove(&id) {
                stack.icons.insert(id, icon);
            }
        }
        for (id, sound) in resources::sounds(&fork) {
            let name = sound.name.to_lowercase();
            // Scripts name sounds in quotes: play "boing".
            if !name.is_empty() && !have_sounds.contains(&name) && scripts.contains(&format!("\"{name}\"")) {
                have_sounds.insert(name);
                let id = if stack.sounds.contains_key(&id) { stack.sounds.keys().max().unwrap_or(&0) + 1 } else { id };
                stack.sounds.insert(id, sound);
            }
        }
    }
}

fn all_scripts(stack: &Stack) -> String {
    let mut text = stack.script.clone();
    for b in &stack.backgrounds {
        text.push_str(&b.script);
        b.parts.iter().for_each(|p| text.push_str(&p.script));
    }
    for c in &stack.cards {
        text.push_str(&c.script);
        c.parts.iter().for_each(|p| text.push_str(&p.script));
    }
    text
}

// ----- binary reading -----

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

type R<T> = Result<T, String>;

fn corrupt<T>(what: &str) -> R<T> {
    Err(format!("The stack is damaged ({what})."))
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    fn at(data: &'a [u8], pos: usize) -> Reader<'a> {
        Reader { data, pos }
    }

    fn bytes(&mut self, n: usize) -> R<&'a [u8]> {
        match self.data.get(self.pos..self.pos + n) {
            Some(b) => {
                self.pos += n;
                Ok(b)
            }
            None => corrupt("unexpected end of block"),
        }
    }

    fn skip(&mut self, n: usize) -> R<()> {
        self.bytes(n).map(|_| ())
    }

    fn u8(&mut self) -> R<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn i16(&mut self) -> R<i16> {
        Ok(i16::from_be_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    fn u16(&mut self) -> R<u16> {
        Ok(u16::from_be_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> R<i32> {
        Ok(i32::from_be_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> R<u32> {
        Ok(u32::from_be_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn rect(&mut self) -> R<QdRect> {
        Ok(QdRect {
            top: self.i16()? as i32,
            left: self.i16()? as i32,
            bottom: self.i16()? as i32,
            right: self.i16()? as i32,
        })
    }

    /// A null-terminated MacRoman string; tolerates a missing terminator at the end.
    fn cstring(&mut self) -> String {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let len = rest.iter().position(|b| *b == 0).unwrap_or(rest.len());
        self.pos += (len + 1).min(rest.len());
        mac_roman(&rest[..len])
    }
}

const MAC_ROMAN_HIGH: [char; 128] = [
    'Ä', 'Å', 'Ç', 'É', 'Ñ', 'Ö', 'Ü', 'á', 'à', 'â', 'ä', 'ã', 'å', 'ç', 'é', 'è', 'ê', 'ë', 'í', 'ì', 'î', 'ï', 'ñ',
    'ó', 'ò', 'ô', 'ö', 'õ', 'ú', 'ù', 'û', 'ü', '†', '°', '¢', '£', '§', '•', '¶', 'ß', '®', '©', '™', '´', '¨', '≠',
    'Æ', 'Ø', '∞', '±', '≤', '≥', '¥', 'µ', '∂', '∑', '∏', 'π', '∫', 'ª', 'º', 'Ω', 'æ', 'ø', '¿', '¡', '¬', '√', 'ƒ',
    '≈', '∆', '«', '»', '…', '\u{a0}', 'À', 'Ã', 'Õ', 'Œ', 'œ', '–', '—', '“', '”', '‘', '’', '÷', '◊', 'ÿ', 'Ÿ', '⁄',
    '€', '‹', '›', 'ﬁ', 'ﬂ', '‡', '·', '‚', '„', '‰', 'Â', 'Ê', 'Á', 'Ë', 'È', 'Í', 'Î', 'Ï', 'Ì', 'Ó', 'Ô', '\u{f8ff}',
    'Ò', 'Ú', 'Û', 'Ù', 'ı', 'ˆ', '˜', '¯', '˘', '˙', '˚', '¸', '˝', '˛', 'ˇ',
];

/// Decode MacRoman text, turning classic Mac carriage returns into newlines.
pub fn mac_roman(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| match *b {
            b'\r' => '\n',
            b if b < 0x80 => b as char,
            b => MAC_ROMAN_HIGH[(b - 0x80) as usize],
        })
        .collect()
}

// ----- blocks -----

struct Block<'a> {
    kind: [u8; 4],
    id: i32,
    data: &'a [u8],
}

fn read_blocks(data: &[u8]) -> R<Vec<Block<'_>>> {
    let mut blocks = Vec::new();
    let mut off = 0;
    while off + HEADER <= data.len() {
        let mut r = Reader::at(data, off);
        // The top byte of the size can hold flags.
        let size = (r.u32()? & 0x00ff_ffff) as usize;
        let kind: [u8; 4] = r.bytes(4)?.try_into().unwrap();
        let id = r.i32()?;
        if size < HEADER || off + size > data.len() {
            if blocks.is_empty() {
                return corrupt("bad first block");
            }
            break;
        }
        blocks.push(Block { kind, id, data: &data[off + HEADER..off + size] });
        if &kind == b"TAIL" {
            break;
        }
        off += size;
    }
    match blocks.first() {
        Some(b) if &b.kind == b"STAK" => Ok(blocks),
        _ => Err("This is not a HyperCard stack.".into()),
    }
}

struct StackInfo {
    protection: crate::model::Protection,
    patterns: Option<Vec<[u8; 8]>>,
    width: u32,
    height: u32,
    first_bg: i32,
    script: String,
    cant_modify: bool,
}

/// The part of the STAK block that private access encrypts.
const ENCRYPTED: std::ops::Range<usize> = 0x08..0x08 + 0x32;

/// Decrypt a private-access stack's header with its password, as HyperCard
/// did (via HyperCardPreview's StackBlockDecrypter).
fn decrypt_header(data: &[u8], password: &str) -> R<Vec<u8>> {
    use crate::model::password::{first_hash, hash_number, stored_hash};
    let mut out = data.to_vec();
    let Some(slice) = out.get_mut(ENCRYPTED) else { return corrupt("short stack block") };
    let mut x = first_hash(password);
    for _ in 0..10 {
        x = hash_number(x);
    }
    // Overlapping 4-byte windows, two bytes apart.
    for i in (0..=slice.len() - 4).step_by(2) {
        x = hash_number(x);
        for (k, b) in x.to_be_bytes().iter().enumerate() {
            slice[i + k] ^= b;
        }
    }
    if Reader::at(&out, 0x34).u32()? != stored_hash(password) {
        return Err(WRONG_PASSWORD.into());
    }
    Ok(out)
}

fn read_stack_block(data: &[u8], password: Option<&str>) -> R<StackInfo> {
    let mut r = Reader::new(data);
    let format = r.u32()?;
    match format {
        9 | 10 => {}
        1..=8 => {
            return Err("This stack was made with HyperCard 1.x. Open it in HyperCard 2.x and use \
                        \"Convert Stack…\" first, then import the converted stack."
                .into());
        }
        _ => return Err(format!("Unknown HyperCard stack format ({format}).")),
    }
    // Flags aren't encrypted; private access encrypts the header around them.
    let flags = Reader::at(data, 0x3c).u16()?;
    let private_access = flags & (1 << 13) != 0;
    let decrypted;
    let header = if private_access {
        decrypted = decrypt_header(data, password.ok_or(NEEDS_PASSWORD)?)?;
        &decrypted[..]
    } else {
        data
    };
    let first_bg = Reader::at(header, 0x18).i32()?;
    let password_hash = Some(Reader::at(header, 0x34).u32()?).filter(|h| *h != 0);
    let user_level_limit = match Reader::at(header, 0x38).u16()? {
        n @ 1..=5 => n as u8,
        _ => 5,
    };
    let protection = crate::model::Protection {
        user_level_limit,
        password_hash,
        cant_delete: flags & (1 << 14) != 0,
        cant_abort: flags & (1 << 11) != 0,
        cant_peek: flags & (1 << 10) != 0,
        private_access,
    };
    let mut size = Reader::at(data, 0x1a8);
    let (height, width) = (size.i16()?, size.i16()?);
    let script = if data.len() > 0x5f0 { Reader::at(data, 0x5f0).cstring() } else { String::new() };
    // The stack's 40 paint patterns, 8 bytes each.
    let patterns = data.get(0x2b0..0x2b0 + 320).and_then(|bytes| {
        let pats: Vec<[u8; 8]> = bytes.as_chunks::<8>().0.to_vec();
        pats.iter().any(|p| *p != [0; 8]).then_some(pats)
    });
    Ok(StackInfo {
        protection,
        patterns,
        width: if width <= 0 { DEFAULT_WIDTH } else { width as u32 },
        height: if height <= 0 { DEFAULT_HEIGHT } else { height as u32 },
        first_bg,
        script,
        cant_modify: flags & (1 << 15) != 0,
    })
}

fn read_fonts(data: &[u8]) -> R<HashMap<i16, String>> {
    let mut r = Reader::new(data);
    let count = r.u32()?;
    r.skip(4)?;
    let mut fonts = HashMap::new();
    for _ in 0..count {
        let id = r.i16()?;
        let name = r.cstring();
        // Records are word aligned.
        if !r.pos.is_multiple_of(2) {
            r.pos += 1;
        }
        fonts.insert(id, name);
    }
    Ok(fonts)
}

/// Card ids in stack order, with their marked flags, from the LIST and PAGE blocks.
fn read_card_order(blocks: &[Block]) -> R<Option<Vec<(i32, bool)>>> {
    let Some(list) = blocks.iter().find(|b| &b.kind == b"LIST") else { return Ok(None) };
    let mut r = Reader::new(list.data);
    let page_count = r.u32()?;
    r.skip(8)?; // page size, total entries
    let entry_size = r.u16()? as usize;
    r.skip(18)?;
    if entry_size < 5 {
        return corrupt("bad page entry size");
    }
    let mut order = Vec::new();
    for _ in 0..page_count {
        let page_id = r.i32()?;
        let entries = r.u16()? as usize;
        let Some(page) = blocks.iter().find(|b| &b.kind == b"PAGE" && b.id == page_id) else {
            return corrupt("missing page");
        };
        let mut p = Reader::at(page.data, 8);
        for _ in 0..entries {
            let card = p.i32()?;
            let flags = p.u8()?;
            p.skip(entry_size - 5)?;
            order.push((card, flags & 0x10 != 0));
        }
    }
    Ok(Some(order))
}

struct PartRec {
    id: i16,
    is_field: bool,
    flags: u8,
    rect: QdRect,
    ext: u8,
    style: u8,
    icon: i16,
    align: i16,
    font: i16,
    size: i16,
    text_style: u8,
    name: String,
    script: String,
}

struct ContentRec {
    /// Negative for card parts, positive for background parts.
    id: i16,
    text: String,
}

struct Layer {
    id: i32,
    bitmap: i32,
    flags: u16,
    /// For cards: the background; for backgrounds: the next background.
    link: i32,
    parts: Vec<PartRec>,
    contents: Vec<ContentRec>,
    name: String,
    script: String,
}

fn read_layer(b: &Block, is_card: bool) -> R<Layer> {
    let mut r = Reader::new(b.data);
    let bitmap = r.i32()?;
    let flags = r.u16()?;
    let link;
    if is_card {
        r.skip(10)?;
        r.skip(4)?; // page id
        link = r.i32()?;
    } else {
        r.skip(2)?;
        r.skip(4)?; // card count
        link = r.i32()?; // next background
        r.skip(4)?; // previous background
    }
    let part_count = r.u16()?;
    r.skip(2)?; // next part id
    r.skip(4)?; // part list size
    let content_count = r.u16()?;
    r.skip(4)?; // content size
    let mut parts = Vec::new();
    for _ in 0..part_count {
        let size = r.u16()? as usize;
        if size < 2 {
            return corrupt("bad part record");
        }
        let rec = r.bytes(size - 2)?;
        parts.push(read_part(rec)?);
    }
    let mut contents = Vec::new();
    for _ in 0..content_count {
        let id = r.i16()?;
        let len = r.u16()? as usize;
        let payload = r.bytes(len)?;
        if !len.is_multiple_of(2) {
            r.pos += 1;
        }
        contents.push(ContentRec { id, text: content_text(payload) });
    }
    let name = r.cstring();
    let script = r.cstring();
    Ok(Layer { id: b.id, bitmap, flags, link, parts, contents, name, script })
}

fn read_part(rec: &[u8]) -> R<PartRec> {
    let mut r = Reader::new(rec);
    let id = r.i16()?;
    let kind = r.u8()?;
    let flags = r.u8()?;
    let rect = r.rect()?;
    let ext = r.u8()?;
    let style = r.u8()?;
    r.skip(2)?; // title width / last selected line
    let icon = r.i16()?;
    let align = r.i16()?;
    let font = r.i16()?;
    let size = r.i16()?;
    let text_style = r.u8()?;
    r.skip(1)?;
    r.skip(2)?; // line height
    let name = r.cstring();
    r.pos += 1;
    let script = r.cstring();
    Ok(PartRec {
        id,
        is_field: kind == 2,
        flags,
        rect,
        ext,
        style,
        icon,
        align,
        font,
        size,
        text_style,
        name,
        script,
    })
}

/// Field text, skipping any table of style runs (rich text isn't supported).
fn content_text(payload: &[u8]) -> String {
    let text = match payload.first() {
        None => &[][..],
        Some(b) if b & 0x80 != 0 && payload.len() >= 2 => {
            let styles = (u16::from_be_bytes([payload[0], payload[1]]) & 0x7fff) as usize;
            payload.get(styles..).unwrap_or(&[])
        }
        Some(_) => &payload[1..],
    };
    let end = text.iter().rposition(|b| *b != 0).map(|i| i + 1).unwrap_or(0);
    mac_roman(&text[..end])
}

fn read_bitmap(data: &[u8], width: u32, height: u32) -> R<Picture> {
    let mut r = Reader::at(data, 8);
    let card = r.rect()?;
    let mask_rect = r.rect()?;
    let image_rect = r.rect()?;
    r.skip(8)?;
    let mask_len = r.u32()? as usize;
    let image_len = r.u32()? as usize;
    let mask = r.bytes(mask_len).unwrap_or(&[]);
    let image = r.bytes(image_len).unwrap_or(&[]);
    let decoded = woba::decode(card, mask_rect, mask, image_rect, image).ok_or("The stack is damaged (bad picture).")?;
    // Place the bitmap on a card-sized picture.
    let mut pic = Picture::new(width, height);
    for y in 0..decoded.height as i32 {
        for x in 0..decoded.width as i32 {
            pic.set(card.left + x, card.top + y, decoded.get(x, y));
        }
    }
    Ok(pic)
}

// ----- conversion to RandomCard's model -----

fn convert(blocks: &[Block], name: &str, password: Option<&str>) -> R<Stack> {
    let info = read_stack_block(blocks[0].data, password)?;
    let fonts = match blocks.iter().find(|b| &b.kind == b"FTBL") {
        Some(b) => read_fonts(b.data)?,
        None => HashMap::new(),
    };
    let bitmaps: HashMap<i32, &[u8]> =
        blocks.iter().filter(|b| &b.kind == b"BMAP").map(|b| (b.id, b.data)).collect();
    let picture = |id: i32| -> R<Picture> {
        match bitmaps.get(&id) {
            Some(data) if id != 0 => read_bitmap(data, info.width, info.height),
            _ => Ok(Picture::new(info.width, info.height)),
        }
    };

    // Backgrounds, in their linked-list order where possible.
    let mut bg_layers: Vec<Layer> =
        blocks.iter().filter(|b| &b.kind == b"BKGD").map(|b| read_layer(b, false)).collect::<R<_>>()?;
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    let mut next = info.first_bg;
    while let Some(i) = bg_layers.iter().position(|l| l.id == next) {
        if !seen.insert(next) {
            break;
        }
        next = bg_layers[i].link;
        ordered.push(bg_layers.remove(i));
    }
    ordered.append(&mut bg_layers);

    let card_layers: HashMap<i32, Layer> = blocks
        .iter()
        .filter(|b| &b.kind == b"CARD")
        .map(|b| read_layer(b, true).map(|l| (l.id, l)))
        .collect::<R<_>>()?;
    let order = match read_card_order(blocks)? {
        Some(o) => o,
        None => blocks.iter().filter(|b| &b.kind == b"CARD").map(|b| (b.id, false)).collect(),
    };

    let mut stack = Stack::new(name);
    stack.width = info.width;
    stack.height = info.height;
    stack.script = info.script;
    stack.cant_modify = info.cant_modify;
    stack.protection = info.protection;
    if let Some(p) = info.patterns {
        stack.patterns = p;
    }
    stack.backgrounds.clear();
    stack.cards.clear();
    let mut max_id = 0;

    for layer in &ordered {
        let mut parts: Vec<Part> = layer.parts.iter().map(|p| convert_part(p, &fonts)).collect();
        // Background contents: shared field text, button contents.
        for c in layer.contents.iter().filter(|c| c.id > 0) {
            if let Some(p) = parts.iter_mut().find(|p| p.id == c.id as Id) {
                p.contents = c.text.clone();
            }
        }
        max_id = max_id.max(layer.id).max(parts.iter().map(|p| p.id as i32).max().unwrap_or(0));
        stack.backgrounds.push(Background {
            id: layer.id as Id,
            name: layer.name.clone(),
            script: layer.script.clone(),
            parts,
            picture: picture(layer.bitmap)?,
            show_pict: layer.flags & (1 << 13) == 0,
            cant_delete: layer.flags & (1 << 14) != 0,
            dont_search: layer.flags & (1 << 11) != 0,
        });
    }

    for (card_id, marked) in order {
        let Some(layer) = card_layers.get(&card_id) else { return corrupt("missing card") };
        let Some(bg) = stack.backgrounds.iter().find(|b| b.id == layer.link as Id) else {
            return corrupt("card without a background");
        };
        let bg_parts: HashMap<Id, (PartKind, bool)> =
            bg.parts.iter().map(|p| (p.id, (p.kind, p.shared_text))).collect();
        let mut parts: Vec<Part> = layer.parts.iter().map(|p| convert_part(p, &fonts)).collect();
        let mut card = Card {
            id: card_id as Id,
            name: layer.name.clone(),
            script: layer.script.clone(),
            background: layer.link as Id,
            picture: picture(layer.bitmap)?,
            show_pict: layer.flags & (1 << 13) == 0,
            marked,
            cant_delete: layer.flags & (1 << 14) != 0,
            dont_search: layer.flags & (1 << 11) != 0,
            ..Card::default()
        };
        for c in &layer.contents {
            if c.id < 0 {
                if let Some(p) = parts.iter_mut().find(|p| p.id == (-(c.id as i32)) as Id) {
                    p.contents = c.text.clone();
                }
            } else {
                // This card's text for a background field, or hilite of a background button.
                match bg_parts.get(&(c.id as Id)) {
                    Some((PartKind::Field, false)) => {
                        card.bg_text.insert(c.id as Id, c.text.clone());
                    }
                    Some((PartKind::Button, _)) => {
                        card.bg_hilite.insert(c.id as Id, c.text == "1");
                    }
                    _ => {}
                }
            }
        }
        max_id = max_id.max(card_id).max(parts.iter().map(|p| p.id as i32).max().unwrap_or(0));
        card.parts = std::mem::take(&mut parts);
        stack.cards.push(card);
    }
    if stack.cards.is_empty() {
        return Err("This stack has no cards.".into());
    }
    stack.next_id = max_id.max(0) as Id + 1;
    Ok(stack)
}

fn convert_part(p: &PartRec, fonts: &HashMap<i16, String>) -> Part {
    let rect = Rect::new(p.rect.left, p.rect.top, p.rect.right, p.rect.bottom);
    let mut part =
        if p.is_field { Part::new_field(p.id as Id, &p.name, rect) } else { Part::new_button(p.id as Id, &p.name, rect) };
    part.script = p.script.clone();
    part.visible = p.flags & 0x80 == 0;
    part.style = match (p.style & 0x0f, p.is_field) {
        (0, _) => PartStyle::Transparent,
        (1, _) => PartStyle::Opaque,
        (2, _) => PartStyle::Rectangle,
        (3, false) => PartStyle::RoundRect,
        (4, _) => PartStyle::Shadow,
        (5, false) => PartStyle::CheckBox,
        (6, false) => PartStyle::Radio,
        (7, true) => PartStyle::Scrolling,
        (8, false) => PartStyle::RoundRect, // "standard"
        (9, false) => PartStyle::Default,
        (10, false) => PartStyle::Oval,
        (_, true) => PartStyle::Rectangle,
        _ => PartStyle::Rectangle, // popup and anything unknown
    };
    part.text_align = match p.align {
        1 => TextAlign::Center,
        -1 => TextAlign::Right,
        _ => TextAlign::Left,
    };
    if let Some(f) = fonts.get(&p.font) {
        part.text_font = f.clone();
    }
    if p.size > 0 {
        part.text_size = p.size as u32;
    }
    let styles: Vec<&str> = [(0x01, "bold"), (0x02, "italic"), (0x04, "underline"), (0x08, "outline"), (0x10, "shadow")]
        .iter()
        .filter(|(bit, _)| p.text_style != 0xff && p.text_style & bit != 0)
        .map(|(_, s)| *s)
        .collect();
    part.text_style = if styles.is_empty() { "plain".into() } else { styles.join(",") };
    if p.is_field {
        part.lock_text = p.flags & 0x01 != 0;
        part.auto_tab = p.flags & 0x02 != 0;
        part.shared_text = p.flags & 0x08 != 0;
        part.dont_search = p.flags & 0x10 != 0;
        part.dont_wrap = p.flags & 0x20 != 0;
        part.auto_select = p.ext & 0x80 != 0;
        part.show_lines = p.ext & 0x40 != 0;
        part.wide_margins = p.ext & 0x20 != 0;
    } else {
        part.enabled = p.flags & 0x01 == 0;
        part.show_name = p.ext & 0x80 != 0;
        part.hilite = p.ext & 0x40 != 0;
        part.auto_hilite = p.ext & 0x20 != 0;
        part.shared_hilite = p.ext & 0x10 == 0;
        part.family = (p.ext & 0x0f) as u32;
        part.icon = p.icon as i32;
    }
    part
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_roman_text() {
        assert_eq!(mac_roman(b"caf\x8e\rna\x95ve \xc2 \x96"), "café\nnaïve ¬ ñ");
    }

    #[test]
    fn content_records() {
        assert_eq!(content_text(b"\0hello\0"), "hello");
        // Styled: 2-byte length (with high bit) covering itself and one 4-byte run.
        assert_eq!(content_text(&[0x80, 0x06, 0, 0, 0, 1, b'h', b'i']), "hi");
        assert_eq!(content_text(b""), "");
    }

    #[test]
    fn resource_chain_supplies_named_sounds_and_used_icons() {
        use resources::tests::{make_fork, standard_snd};
        let dir = std::env::temp_dir().join(format!("randomcard-snd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let snd = |n| standard_snd(&[0x80; 50], 11025, 60, (0, 0)).into_iter().chain(std::iter::repeat_n(0, n)).collect();
        let fork = make_fork(&[
            (b"snd ", 1, "Boing", snd(0)),
            (b"snd ", 2, "Unused", snd(1)),
            (b"ICON", 3001, "Stack", vec![0xff; 128]),
            (b"ICON", 3002, "Other", vec![0; 128]),
        ]);
        std::fs::write(dir.join("HyperCard.rsrc"), fork).unwrap();
        let mut stack = Stack::new("Test");
        stack.script = "on mouseUp\n  play \"boing\"\nend mouseUp".into();
        stack.cards[0].parts.push(Part { icon: 3001, ..Part::new_button(1, "B", Rect::new(0, 0, 10, 10)) });
        add_chain_resources(&mut stack, &dir.join("Test"));
        std::fs::remove_dir_all(&dir).ok();
        let names: Vec<&str> = stack.sounds.values().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Boing"]);
        assert_eq!(stack.icons.keys().copied().collect::<Vec<_>>(), vec![3001]);
    }

    #[test]
    fn rejects_other_files() {
        assert!(!is_hypercard_stack(b"{\"name\": \"json stack\"}"));
        assert!(is_hypercard_stack(b"\0\0\x08\0STAK\xff\xff\xff\xff\0\0\0\0"));
    }

    /// Imports the sample stacks from HyperCardPreview's test suite when
    /// RANDOMCARD_HC_SAMPLES points at that directory:
    /// <https://github.com/PierreLorenzi/HyperCardPreview/tree/master/HyperCardCommon/HyperCardCommonTests>
    #[test]
    #[ignore]
    fn sample_stacks() {
        let Ok(dir) = std::env::var("RANDOMCARD_HC_SAMPLES") else { return };
        let dir = Path::new(&dir);
        let open = |n: &str| load_any(&dir.join(format!("{n}.stack")), None).unwrap_or_else(|e| panic!("{n}: {e}"));

        let s = open("TestCardsBackgrounds");
        let ids: Vec<Id> = s.cards.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![2842, 3767, 4162, 4585, 4660, 5225]);
        let marked: Vec<bool> = s.cards.iter().map(|c| c.marked).collect();
        assert_eq!(marked, vec![false, true, false, true, false, false]);
        let bgs: Vec<Id> = s.backgrounds.iter().map(|b| b.id).collect();
        assert_eq!(bgs, vec![2769, 3887, 5065]);
        assert_eq!(s.cards[2].background, 3887);
        assert_eq!(s.cards[5].background, 5065);

        let s = open("TestButtonProperties");
        let b = &s.backgrounds[0].parts.iter().filter(|p| p.kind == PartKind::Button).collect::<Vec<_>>();
        assert_eq!((b[0].id, b[1].id), (1, 2));
        assert!(!b[1].enabled && b[2].enabled);
        assert!(!b[2].visible && b[3].visible);
        assert_eq!(b[3].rect, Rect::new(44, 188, 139, 210));
        assert_eq!(b[4].family, 6);
        assert!(!b[5].shared_hilite && b[6].shared_hilite);
        assert!(b[6].auto_hilite && !b[7].auto_hilite);
        assert!(b[7].hilite && !b[8].hilite);
        assert!(!b[8].show_name && b[9].show_name);

        let s = open("TestFieldProperties");
        let f = &s.backgrounds[0].parts.iter().filter(|p| p.kind == PartKind::Field).collect::<Vec<_>>();
        assert_eq!((f[0].id, f[1].id), (5, 6));
        assert!(f[1].lock_text && !f[2].lock_text);
        assert!(f[2].auto_tab && !f[3].auto_tab);
        assert!(f[4].shared_text && !f[5].shared_text);
        assert!(f[5].dont_search && !f[6].dont_search);
        assert!(f[6].dont_wrap && !f[7].dont_wrap);
        assert!(!f[7].visible && f[8].visible);
        assert_eq!(f[8].rect, Rect::new(271, 210, 446, 238));

        let s = open("TestContents");
        let bg = &s.backgrounds[0];
        let fields: Vec<&Part> = bg.parts.iter().filter(|p| p.kind == PartKind::Field).collect();
        assert_eq!(fields[1].contents, "shared content");
        let card = &s.cards[0];
        assert_eq!(s.field_text(card.id, crate::model::Layer::Background, fields[0].id), "card content in bg field");
        let buttons: Vec<&Part> = bg.parts.iter().filter(|p| p.kind == PartKind::Button).collect();
        assert!(s.hilite(card.id, crate::model::Layer::Background, buttons[1].id));
        assert_eq!(card.parts.iter().find(|p| p.kind == PartKind::Field).unwrap().contents, "card content");
        assert!(card.parts.iter().find(|p| p.kind == PartKind::Button).unwrap().hilite);

        assert_eq!(open("TestStackScript").script, "-- script of stack\n-- with two lines");
        assert!(load_any(&dir.join("TestV1.stack"), None).unwrap_err().contains("1.x"));
        // Private access needs the password, which decrypts the stack header.
        let private = dir.join("TestPrivateAccess.stack");
        assert_eq!(load_any(&private, None).unwrap_err(), NEEDS_PASSWORD);
        assert_eq!(load_any(&private, Some("coucou")).unwrap_err(), WRONG_PASSWORD);
        let s = load_any(&private, Some("AA ee 1234 aaa")).unwrap();
        assert!(s.protection.private_access && !s.cards.is_empty());
        assert_eq!(s.protection.password_hash, Some(0xCA92_2FEB));
        // Protection settings come across.
        assert_eq!(open("TestPassword").protection.password_hash, Some(0xCA92_2FEB));
        assert!(open("TestCantPeek").protection.cant_peek);
        assert!(open("TestCantAbort").protection.cant_abort);
        assert!(open("TestCantDelete").protection.cant_delete);
        assert!(open("TestCantModify").cant_modify);
        assert_eq!(open("TestUserLevel1").protection.user_level_limit, 1);
        assert_eq!(open("TestUserLevel5").protection.user_level_limit, 5);
        let home = open("Home");
        assert!(home.backgrounds.iter().any(|b| !b.picture.is_blank()));
        // Stacks carry their own 40 paint patterns.
        assert_eq!(home.patterns.len(), 40);
        assert_ne!(home.patterns, crate::paint::DEFAULT_PATTERNS.to_vec());
        // Home.rsrc beside Home.stack holds nine of the icons its buttons use;
        // the rest are HyperCard's built-in icons, from HyperCard's own fork.
        for id in [3000, 3001, 6134, 7024, 11316, 12659, 28226, 28867, 30504] {
            assert!(home.icons.contains_key(&id), "icon {id}");
        }
        let used: HashSet<i32> = home.cards.iter().flat_map(|c| c.parts.iter().map(|p| p.icon)).collect();
        assert!(used.contains(&3001) && used.contains(&28226));

        // Icons a stack lacks come from further along the resource chain, e.g.
        // a HyperCard application fork beside it; only the used ones are copied.
        let tmp = std::env::temp_dir().join(format!("randomcard-chain-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::copy(dir.join("Home.stack"), tmp.join("My Stack")).unwrap();
        std::fs::copy(dir.join("Home.rsrc"), tmp.join("HyperCard.rsrc")).unwrap();
        let chained = load_any(&tmp.join("My Stack"), None);
        std::fs::remove_dir_all(&tmp).ok();
        let chained = chained.unwrap();
        assert!(chained.icons.contains_key(&3001));
        assert!(!chained.icons.contains_key(&3000), "unused icons aren't copied");

        // Every script in every sample stack parses.
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let Ok(s) = load_any(&entry.path(), Some("AA éé 1234 ÀÂä")) else { continue };
            let mut scripts = vec![s.script.clone()];
            for b in &s.backgrounds {
                scripts.push(b.script.clone());
                scripts.extend(b.parts.iter().map(|p| p.script.clone()));
            }
            for c in &s.cards {
                scripts.push(c.script.clone());
                scripts.extend(c.parts.iter().map(|p| p.script.clone()));
            }
            for src in scripts {
                if let Err(e) = crate::hypertalk::parser::parse_script(&src) {
                    panic!("{}: {e}", entry.path().display());
                }
            }
        }
    }
}
