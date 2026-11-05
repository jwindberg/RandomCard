//! Pictures from other programs: an image on the system clipboard pasted,
//! or a picture file dropped onto the card, becomes a paint selection, as
//! pasted art did in HyperCard. Pictures copied in RandomCard go onto the
//! system clipboard too, for other programs.

use eframe::egui;

use crate::model::Picture;
use crate::paint::{self, Region, image_file};
use crate::runtime::world::Tool;

use super::App;
use super::editing::Clip;
use super::painting::Marquee;

/// The image on the system clipboard, if there is one.
fn system_image() -> Option<Picture> {
    let data = arboard::Clipboard::new().ok()?.get_image().ok()?;
    Some(image_file::from_rgba_bytes(data.width as u32, data.height as u32, &data.bytes))
}

fn fingerprint(pic: &Picture) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (pic.width, pic.height).hash(&mut h);
    pic.iter().for_each(|p| p.hash(&mut h));
    h.finish()
}

/// Whether two pictures look the same: a picture that went out through the
/// system clipboard and came back may differ in its see-through pixels.
fn same_look(a: &Picture, b: &Picture) -> bool {
    (a.width, a.height) == (b.width, b.height)
        && a.iter().zip(b.iter()).all(|(p, q)| p == q || (p.0[3] < 255 && q.0[3] < 255))
}

impl App {
    /// Copying a picture also puts it on the system clipboard. Copying a
    /// part notes what image is there, so Paste knows if a newer one comes.
    pub(super) fn copied_to_clipboard(&mut self, ctx: &egui::Context) {
        match &self.clipboard {
            Some(Clip::Picture { pixels, .. }) => {
                let size = [pixels.width as usize, pixels.height as usize];
                ctx.copy_image(egui::ColorImage::from_rgba_unmultiplied(size, &image_file::rgba_bytes(pixels)));
                self.system_image_at_copy = None;
            }
            Some(Clip::Part { .. }) => self.system_image_at_copy = system_image().map(|p| fingerprint(&p)),
            None => {}
        }
    }

    /// An image another program put on the clipboard after RandomCard's
    /// own Copy, which Paste takes instead.
    pub(super) fn newer_system_image(&self) -> Option<Picture> {
        if self.editing_level() < 3 {
            return None;
        }
        let pic = system_image()?;
        let newer = match &self.clipboard {
            None => true,
            Some(Clip::Picture { pixels, .. }) => !same_look(pixels, &pic),
            Some(Clip::Part { .. }) => self.system_image_at_copy != Some(fingerprint(&pic)),
        };
        newer.then_some(pic)
    }

    /// A picture from outside pasted onto the layer being painted, scaled
    /// down to fit the card if it's bigger, centered on `at` (or on the
    /// card), and left selected with the Select tool.
    pub(super) fn paste_image(&mut self, pic: Picture, at: Option<(i32, i32)>) {
        let (cw, ch) = {
            let w = self.world();
            (w.stack.width, w.stack.height)
        };
        let pic = image_file::fit_within(pic, cw, ch);
        let (w, h) = (pic.width as i32, pic.height as i32);
        let (x, y) = at.unwrap_or((cw as i32 / 2, ch as i32 / 2));
        let left = (x - w / 2).clamp(0, cw as i32 - w);
        let top = (y - h / 2).clamp(0, ch as i32 - h);
        if !matches!(self.world().tool, Tool::Select | Tool::Lasso) {
            self.choose_tool(Tool::Select);
        }
        self.commit_marquee();
        self.push_undo();
        self.with_picture(|layer| paint::paste(layer, &pic, left, top));
        self.marquee = Some(Marquee::Selected { region: Region { rect: (left, top, left + w, top + h), lasso: None } });
    }

    /// Picture files dropped onto the window.
    pub(super) fn dropped_files(&mut self, ctx: &egui::Context) {
        let (files, pos) = ctx.input(|i| (i.raw.dropped_files.clone(), i.pointer.hover_pos()));
        let Some(file) = files.first() else { return };
        if self.editing_level() < 3 {
            self.alert("Pictures can be dropped onto a card at the Painting user level and above.".into());
            return;
        }
        let (bytes, name) = (file.bytes(), file.path().display().to_string());
        match bytes.and_then(|b| image_file::decode(&b)) {
            Ok(pic) => {
                let at = pos.map(|p| ((p.x - self.card_origin.x) as i32, (p.y - self.card_origin.y) as i32));
                self.paste_image(pic, at);
            }
            Err(e) => self.alert(format!("Can't drop {name}: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pixel;

    #[test]
    fn looks_the_same_after_the_clipboard() {
        let mut a = Picture::new(3, 1);
        a.set(0, 0, Pixel::BLACK);
        a.set(1, 0, Pixel::rgba(10, 20, 30, 128));
        let mut b = a.clone();
        // Half-transparent pixels can come back a little different.
        b.set(1, 0, Pixel::rgba(11, 20, 30, 127));
        assert!(same_look(&a, &b));
        b.set(0, 0, Pixel::WHITE);
        assert!(!same_look(&a, &b));
        assert_ne!(fingerprint(&a), fingerprint(&b));
        assert!(!same_look(&a, &Picture::new(1, 3)));
    }
}
