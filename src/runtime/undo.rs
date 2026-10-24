//! Undo and redo for the stack: paint, parts and cards share one history,
//! newest last. Undoing an entry applies it and records its inverse for redo.

use std::collections::HashMap;

use crate::model::{Background, Card, Id, Layer, Part, Picture};
use crate::model::styles::StyleRun;

use super::world::World;

const MAX_UNDO: usize = 30;

#[derive(Clone, Debug)]
pub enum UndoEntry {
    /// A card's or background's picture as it was.
    Picture { layer: Layer, owner: Id, picture: Picture },
    /// A card's or background's parts as they were. For a background, also
    /// each of its cards' text and hilites for background parts.
    Parts { layer: Layer, owner: Id, parts: Vec<Part>, card_state: Vec<CardPartState> },
    /// A card that was deleted (with its background, if that went too).
    CardDeleted { index: usize, card: Box<Card>, background: Option<Box<Background>> },
    /// A card that was added.
    CardAdded { id: Id },
}

#[derive(Clone, Debug)]
pub struct CardPartState {
    card: Id,
    text: HashMap<Id, String>,
    styles: HashMap<Id, Vec<StyleRun>>,
    hilite: HashMap<Id, bool>,
}

impl World {
    fn owner(&self, layer: Layer) -> Id {
        match layer {
            Layer::Card => self.card_id,
            Layer::Background => self.bg_id(),
        }
    }

    /// Remember the current picture of the layer being painted.
    pub fn record_picture(&mut self) {
        let layer = self.current_layer();
        let owner = self.owner(layer);
        let picture = match layer {
            Layer::Card => self.stack.card(owner).map(|c| c.picture.clone()),
            Layer::Background => self.stack.background(owner).map(|b| b.picture.clone()),
        };
        if let Some(picture) = picture {
            self.record(UndoEntry::Picture { layer, owner, picture });
        }
    }

    /// Remember a layer's parts before changing them.
    pub fn record_parts(&mut self, layer: Layer) {
        let owner = self.owner(layer);
        if let Some(entry) = self.parts_entry(layer, owner) {
            self.record(entry);
        }
    }

    fn parts_entry(&self, layer: Layer, owner: Id) -> Option<UndoEntry> {
        let parts = match layer {
            Layer::Card => self.stack.card(owner)?.parts.clone(),
            Layer::Background => self.stack.background(owner)?.parts.clone(),
        };
        let card_state = if layer == Layer::Background {
            self.stack
                .cards
                .iter()
                .filter(|c| c.background == owner)
                .map(|c| CardPartState {
                    card: c.id,
                    text: c.bg_text.clone(),
                    styles: c.bg_styles.clone(),
                    hilite: c.bg_hilite.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        Some(UndoEntry::Parts { layer, owner, parts, card_state })
    }

    /// Remember a card about to be deleted.
    pub fn record_card_deleted(&mut self, id: Id) {
        let Some(index) = self.stack.card_index(id) else { return };
        let card = self.stack.cards[index].clone();
        let bg = card.background;
        let last_of_bg = !self.stack.cards.iter().any(|c| c.background == bg && c.id != id);
        let background = if last_of_bg { self.stack.background(bg).cloned().map(Box::new) } else { None };
        self.record(UndoEntry::CardDeleted { index, card: Box::new(card), background });
    }

    pub fn record(&mut self, entry: UndoEntry) {
        self.undo.push(entry);
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.undo.pop() else { return false };
        if let Some(inverse) = self.apply_undo(entry) {
            self.redo.push(inverse);
        }
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.redo.pop() else { return false };
        if let Some(inverse) = self.apply_undo(entry) {
            self.undo.push(inverse);
        }
        true
    }

    /// Restore an entry, returning the entry that would reverse it.
    fn apply_undo(&mut self, entry: UndoEntry) -> Option<UndoEntry> {
        self.dirty = true;
        match entry {
            UndoEntry::Picture { layer, owner, picture } => {
                let slot = match layer {
                    Layer::Card => &mut self.stack.card_mut(owner)?.picture,
                    Layer::Background => &mut self.stack.background_mut(owner)?.picture,
                };
                let current = std::mem::replace(slot, picture);
                self.picture_rev += 1;
                Some(UndoEntry::Picture { layer, owner, picture: current })
            }
            UndoEntry::Parts { layer, owner, parts, card_state } => {
                let inverse = self.parts_entry(layer, owner);
                match layer {
                    Layer::Card => self.stack.card_mut(owner)?.parts = parts,
                    Layer::Background => self.stack.background_mut(owner)?.parts = parts,
                }
                for state in card_state {
                    if let Some(c) = self.stack.card_mut(state.card) {
                        c.bg_text = state.text;
                        c.bg_styles = state.styles;
                        c.bg_hilite = state.hilite;
                    }
                }
                inverse
            }
            UndoEntry::CardDeleted { index, card, background } => {
                let id = card.id;
                if let Some(bg) = background
                    && self.stack.background(bg.id).is_none()
                {
                    self.stack.backgrounds.push(*bg);
                }
                let index = index.min(self.stack.cards.len());
                self.stack.cards.insert(index, *card);
                self.card_id = id;
                self.picture_rev += 1;
                Some(UndoEntry::CardAdded { id })
            }
            UndoEntry::CardAdded { id } => {
                if self.stack.cards.len() <= 1 {
                    return None;
                }
                let index = self.stack.card_index(id)?;
                let card = self.stack.cards.remove(index);
                let bg = card.background;
                let background = if self.stack.cards.iter().any(|c| c.background == bg) {
                    None
                } else {
                    let i = self.stack.bg_index(bg)?;
                    Some(Box::new(self.stack.backgrounds.remove(i)))
                };
                if self.card_id == id {
                    // Back to the card it was added after.
                    self.card_id = self.stack.cards[index.saturating_sub(1).min(self.stack.cards.len() - 1)].id;
                }
                self.history.retain(|h| *h != id);
                if self.history.is_empty() {
                    let current = self.card_id;
                    self.history.push(current);
                }
                self.history_pos = self.history_pos.min(self.history.len() - 1);
                Some(UndoEntry::CardDeleted { index, card: Box::new(card), background })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Part, Pixel, Rect, Stack};
    use crate::runtime::world::World;

    #[test]
    fn undo_and_redo_parts_pictures_and_cards() {
        let mut w = World::new(Stack::new("t"));
        // Parts
        w.record_parts(crate::model::Layer::Card);
        let card = w.card_id;
        w.stack.card_mut(card).unwrap().parts.push(Part::new_button(50, "B", Rect::new(0, 0, 10, 10)));
        assert!(w.undo());
        assert!(w.stack.card(card).unwrap().parts.is_empty());
        assert!(w.redo());
        assert_eq!(w.stack.card(card).unwrap().parts.len(), 1);
        // Pictures
        w.record_picture();
        w.layer_picture_mut().unwrap().set(1, 1, Pixel::Black);
        w.undo();
        assert_eq!(w.stack.card(card).unwrap().picture.get(1, 1), Pixel::Clear);
        // Cards: delete, undo brings it back, redo deletes again.
        let mut extra = w.stack.cards[0].clone();
        extra.id = 77;
        w.stack.cards.push(extra);
        w.record_card_deleted(77);
        w.stack.cards.retain(|c| c.id != 77);
        assert!(w.undo());
        assert!(w.stack.card(77).is_some());
        assert_eq!(w.card_id, 77);
        assert!(w.redo());
        assert!(w.stack.card(77).is_none());
    }
}
