//! The Edit menu for objects and pictures: cut, copy, paste and clear of
//! buttons, fields and paint selections, undo and redo, and part order.

use crate::model::styles::StyleRun;
use crate::model::{Layer, Part, PartKind, Picture};
use crate::paint::{self, Region};
use crate::runtime::world::{EditOp, ObjRef, Tool};

use super::App;
use super::painting::Marquee;

pub enum Clip {
    /// A button or field, with the text and hilite it had on its card.
    Part { part: Box<Part>, text: String, styles: Vec<StyleRun>, hilite: bool },
    /// Painted pixels and where they came from.
    Picture { pixels: Picture, rect: (i32, i32, i32, i32) },
}

#[derive(Clone, Copy)]
pub enum Order {
    Closer,
    Farther,
    Front,
    Back,
}

impl App {
    /// The button or field selected with the Button or Field tool.
    fn selected_part(&self) -> Option<ObjRef> {
        let tool = self.world().tool;
        matches!(tool, Tool::Button | Tool::Field).then_some(self.selected).flatten()
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
        if let Some(ObjRef::Part { layer, id, card }) = self.selected_part() {
            let w = self.world();
            return w.stack.part(card, layer, id).map(|p| if p.kind == PartKind::Button { "Button" } else { "Field" });
        }
        matches!(self.marquee, Some(Marquee::Selected { .. } | Marquee::Moving { .. })).then_some("Picture")
    }

    pub(super) fn paste_name(&self) -> Option<&'static str> {
        match &self.clipboard {
            Some(Clip::Part { part, .. }) => Some(if part.kind == PartKind::Button { "Button" } else { "Field" }),
            Some(Clip::Picture { .. }) => Some("Picture"),
            None => None,
        }
    }

    pub(super) fn edit_command(&mut self, op: EditOp) {
        match op {
            EditOp::Copy => {
                self.copy_selection();
            }
            EditOp::Cut => {
                if self.copy_selection() {
                    self.clear_selection();
                }
            }
            EditOp::Clear => self.clear_selection(),
            EditOp::Paste => self.paste(),
        }
    }

    /// Copy the selected part or paint selection; returns whether there was one.
    fn copy_selection(&mut self) -> bool {
        if let Some(ObjRef::Part { layer, id, card }) = self.selected_part() {
            let w = self.world();
            let Some(part) = w.stack.part(card, layer, id).cloned() else { return false };
            let text = w.stack.field_text(card, layer, id);
            let styles = w.stack.field_styles(card, layer, id);
            let hilite = w.stack.hilite(card, layer, id);
            drop(w);
            self.clipboard = Some(Clip::Part { part: Box::new(part), text, styles, hilite });
            return true;
        }
        let Some(region) = self.paint_selection() else { return false };
        let pixels = {
            let mut w = self.world();
            let Some(pic) = w.layer_picture_mut() else { return false };
            // Lift from a copy, leaving the picture as it is.
            region.lift(&mut pic.clone())
        };
        self.clipboard = Some(Clip::Picture { pixels, rect: region.rect });
        true
    }

    fn clear_selection(&mut self) {
        if let Some(obj) = self.selected_part() {
            self.delete_part(obj);
        } else if let Some(region) = self.paint_selection() {
            self.push_undo();
            self.with_picture(|pic| region.lift(pic));
            self.marquee = None;
        }
    }

    fn paste(&mut self) {
        match &self.clipboard {
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
                self.with_picture(|pic| paint::paste(pic, &pixels, rect.0, rect.1));
                // The pasted pixels stay selected, ready to drag into place.
                self.marquee = Some(Marquee::Selected { region: Region { rect, lasso: None } });
            }
            None => {}
        }
    }

    /// Paste a part onto the layer being edited, keeping its text and hilite.
    fn paste_part(&mut self, mut part: Part, text: String, styles: Vec<StyleRun>, hilite: bool) {
        let kind = part.kind;
        let obj = {
            let mut w = self.world();
            let layer = w.current_layer();
            w.record_parts(layer);
            let (card, bg) = (w.card_id, w.bg_id());
            let id = w.stack.alloc_id();
            part.id = id;
            match layer {
                Layer::Card => {
                    if kind == PartKind::Field {
                        part.contents = text;
                        part.styles = styles;
                    }
                    part.hilite = hilite;
                    if let Some(c) = w.stack.card_mut(card) {
                        c.parts.push(part);
                    }
                }
                Layer::Background => {
                    // Background text and hilites may belong to each card.
                    let per_card_text = kind == PartKind::Field && !part.shared_text;
                    let per_card_hilite = kind == PartKind::Button && !part.shared_hilite;
                    if kind == PartKind::Field {
                        part.contents = if per_card_text { String::new() } else { text.clone() };
                        part.styles = if per_card_text { Vec::new() } else { styles.clone() };
                    }
                    if let Some(c) = w.stack.card_mut(card) {
                        if per_card_text {
                            c.bg_text.insert(id, text);
                            if !styles.is_empty() {
                                c.bg_styles.insert(id, styles);
                            }
                        }
                        if per_card_hilite {
                            c.bg_hilite.insert(id, hilite);
                        }
                    }
                    if !per_card_hilite {
                        part.hilite = hilite;
                    }
                    if let Some(b) = w.stack.background_mut(bg) {
                        b.parts.push(part);
                    }
                }
            }
            w.dirty = true;
            ObjRef::Part { layer, id, card }
        };
        self.choose_tool(if kind == PartKind::Button { Tool::Button } else { Tool::Field });
        self.selected = Some(obj);
    }

    /// Move the selected part in front of or behind the others.
    pub(super) fn reorder_selected(&mut self, order: Order) {
        let Some(ObjRef::Part { layer, id, card }) = self.selected else { return };
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
        if let Some(ObjRef::Part { layer, id, card }) = self.selected
            && self.world().stack.part(card, layer, id).is_none()
        {
            self.selected = None;
        }
    }
}
