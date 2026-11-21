//! The Edit menu for objects and pictures: cut, copy, paste and clear of
//! buttons, fields and paint selections, undo and redo, and part order.

use crate::model::styles::StyleRun;
use crate::model::{Layer, Part, PartKind};
use crate::paint::Region;
pub use crate::runtime::world::Clip;
use crate::runtime::world::{EditOp, ObjRef, Tool};

use super::App;
use super::painting::Marquee;

#[derive(Clone, Copy)]
pub enum Order {
    Closer,
    Farther,
    Front,
    Back,
}

impl App {
    /// The button or field selected with the Button or Field tool.
    fn selected_tool_part(&self) -> Option<ObjRef> {
        let w = self.world();
        w.tool.is_part_tool().then_some(w.selected_part).flatten()
    }

    fn paint_selection(&mut self) -> Option<Region> {
        self.commit_marquee();
        match &self.marquee {
            Some(Marquee::Selected { region }) => Some(region.clone()),
            _ => None,
        }
    }

    /// What the Edit menu's Cut, Copy and Clear would act on, for its labels.
    pub(super) fn edit_target_name(&self) -> Option<&'static str> {
        // With the Browse tool, selected text in a field: "Copy Text".
        {
            let w = self.world();
            if w.tool == Tool::Browse {
                return w.selection.as_ref().filter(|r| r.start < r.end).map(|_| "Text");
            }
        }
        if let Some(ObjRef::Part { layer, id, card }) = self.selected_tool_part() {
            let w = self.world();
            return w.stack.part(card, layer, id).map(|p| kind_label(p.kind));
        }
        matches!(self.marquee, Some(Marquee::Selected { .. } | Marquee::Moving { .. })).then_some("Picture")
    }

    pub(super) fn paste_name(&self) -> Option<&'static str> {
        if self.world().tool == Tool::Browse && self.editing.is_some() {
            return Some("Text");
        }
        match &self.world().clipboard {
            Some(Clip::Part { part, .. }) => Some(kind_label(part.kind)),
            Some(Clip::Picture { .. }) => Some("Picture"),
            // Maybe an image from another program: looking would mean
            // reading the system clipboard every frame, so plain "Paste".
            None => Some(""),
        }
    }

    pub(super) fn edit_command(&mut self, ctx: &eframe::egui::Context, op: EditOp) {
        // Text in the field being edited, with nothing copied from a field
        // to paste: the system clipboard, as typing the key would.
        if self.editing.is_some() && self.world().tool == Tool::Browse {
            use eframe::egui::ViewportCommand;
            match op {
                EditOp::Copy => ctx.send_viewport_cmd(ViewportCommand::RequestCopy),
                EditOp::Cut => ctx.send_viewport_cmd(ViewportCommand::RequestCut),
                EditOp::Paste => ctx.send_viewport_cmd(ViewportCommand::RequestPaste),
                EditOp::Clear => {}
            }
            return;
        }
        match op {
            EditOp::Copy => {
                if self.copy_selection() {
                    self.copied_to_clipboard(ctx);
                }
            }
            EditOp::Cut => {
                if self.copy_selection() {
                    self.copied_to_clipboard(ctx);
                    self.clear_selection();
                }
            }
            EditOp::Clear => self.clear_selection(),
            EditOp::Paste => self.paste(),
        }
    }

    /// Copy the selected part or paint selection; returns whether there was one.
    fn copy_selection(&mut self) -> bool {
        if let Some(obj) = self.selected_tool_part() {
            let mut w = self.world();
            let Some(clip) = w.copy_part(obj) else { return false };
            w.clipboard = Some(clip);
            // A button or field stays in RandomCard: nothing other programs read.
            w.clip_types.clear();
            return true;
        }
        let Some(region) = self.paint_selection() else { return false };
        let pixels = {
            let mut w = self.world();
            let Some(pic) = w.layer_picture_mut() else { return false };
            // Lift from a copy, leaving the picture as it is.
            region.lift(&mut pic.clone())
        };
        let mut w = self.world();
        w.clipboard = Some(Clip::Picture { pixels, rect: region.rect });
        w.clip_types = "PICT".into();
        true
    }

    fn clear_selection(&mut self) {
        if let Some(obj) = self.selected_tool_part() {
            self.delete_part(obj);
        } else if let Some(region) = self.paint_selection() {
            self.push_undo();
            self.lift_selection(&region);
            self.marquee = None;
        }
    }

    fn paste(&mut self) {
        // An image copied in another program since.
        if let Some(pic) = self.newer_system_image() {
            self.paste_image(pic, None);
            return;
        }
        let clip = self.world().clipboard.clone();
        match &clip {
            Some(Clip::Part { part, text, styles, hilite }) => {
                let (part, text, styles, hilite) = ((**part).clone(), text.clone(), styles.clone(), *hilite);
                self.paste_part(part, text, styles, hilite);
            }
            Some(Clip::Picture { pixels, rect }) => {
                let (pixels, rect) = (pixels.clone(), *rect);
                if !matches!(self.world().tool, Tool::Select | Tool::Lasso) {
                    self.choose_tool(Tool::Select);
                }
                self.commit_marquee();
                self.push_undo();
                // The pasted pixels float, selected, ready to drag into place.
                let region = Region::new(rect);
                self.float_selection(&region, &pixels);
                self.marquee = Some(Marquee::Selected { region });
            }
            None => {}
        }
    }

    /// Paste a part onto the layer being edited, keeping its text and hilite.
    fn paste_part(&mut self, part: Part, text: String, styles: Vec<StyleRun>, hilite: bool) {
        let kind = part.kind;
        let obj = self.world().paste_part(part, text, styles, hilite);
        self.choose_tool(Tool::for_part(kind));
        self.set_selected(Some(obj));
    }

    /// Move the selected part in front of or behind the others.
    pub(super) fn reorder_selected(&mut self, order: Order) {
        let Some(ObjRef::Part { layer, id, card }) = self.selected_part() else { return };
        let mut w = self.world();
        w.record_parts(layer);
        let bg = w.bg_id();
        let parts = match layer {
            Layer::Card => w.stack.card_mut(card).map(|c| &mut c.parts),
            Layer::Background => w.stack.background_mut(bg).map(|b| &mut b.parts),
        };
        let Some(parts) = parts else { return };
        let Some(i) = parts.iter().position(|p| p.id == id) else { return };
        let part = parts.remove(i);
        let j = match order {
            Order::Closer => (i + 1).min(parts.len()),
            Order::Farther => i.saturating_sub(1),
            Order::Front => parts.len(),
            Order::Back => 0,
        };
        parts.insert(j, part);
        w.dirty = true;
    }

    pub(super) fn undo(&mut self) {
        self.commit_marquee();
        self.commit_text();
        let undone = self.world().undo();
        if undone {
            self.after_undo();
        }
    }

    pub(super) fn redo(&mut self) {
        let redone = self.world().redo();
        if redone {
            self.after_undo();
        }
    }

    /// Selections may refer to things that are no longer there.
    fn after_undo(&mut self) {
        self.marquee = None;
        self.polygon = None;
        if let Some(ObjRef::Part { layer, id, card }) = self.selected_part()
            && self.world().stack.part(card, layer, id).is_none()
        {
            self.set_selected(None);
        }
    }
}

/// A part's kind as the Edit menu names it: Copy Button, Paste Image.
fn kind_label(kind: PartKind) -> &'static str {
    match kind {
        PartKind::Button => "Button",
        PartKind::Field => "Field",
        PartKind::Image => "Image",
    }
}
