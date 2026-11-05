//! Popup buttons' menus. Clicking a popup with the Browse tool shows its
//! lines as a menu, placed so the chosen item sits over the button, as the
//! Macintosh placed popup menus. Choosing an item sets the popup's selected
//! line, then sends mouseUp. A line starting with "-" is a divider, and one
//! starting with "(" is disabled, as in HyperCard's menus.

use eframe::egui::{self, Align, Layout, PopupAnchor, Pos2, RectAlign};

use crate::runtime::world::{DialogReply, Event, ObjRef, lock};

use super::App;
use super::menus::menu_item;

/// A popup's menu while it's open.
pub(super) struct PopupMenu {
    /// The popup button, or None for a menu a script asked for, whose
    /// choice goes back to the script.
    pub obj: Option<ObjRef>,
    /// The popup's menu box on screen.
    pub at: egui::Rect,
    pub items: Vec<String>,
    /// The chosen line, from 1, or 0 for none.
    pub selected: u32,
    /// Still holding the click that opened the menu: letting go over an
    /// item chooses it, as dragging to an item did on the Mac.
    pub dragging: bool,
    /// A combo field's choices, rather than a popup button's menu.
    pub field: bool,
}

impl App {
    pub(super) fn popup_menu(&mut self, ctx: &egui::Context) {
        let Some(menu) = &mut self.popup else { return };
        let (pressed, released, pointer) =
            ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.latest_pos()));
        let row = ctx.global_style().spacing.interact_size.y;
        let above = row * menu.selected.saturating_sub(1) as f32;
        let pos = Pos2::new(menu.at.left(), (menu.at.top() - above).max(0.0));
        // As wide as the popup's box, or wider to fit its longest item, as
        // the Macintosh drew popup menus; a narrow box doesn't squeeze them.
        let width = {
            let style = ctx.global_style();
            let font = egui::TextStyle::Button.resolve(&style);
            let widest = ctx.fonts_mut(|f| {
                menu.items
                    .iter()
                    .map(|i| f.layout_no_wrap(i.trim_start_matches('(').to_string(), font.clone(), egui::Color32::WHITE).size().x)
                    .fold(0.0, f32::max)
            });
            // The check mark column, and the button's padding each side.
            let room = font.size * 1.3 + style.spacing.button_padding.x * 2.0 + style.spacing.menu_margin.sum().x;
            menu.at.width().max(widest + room)
        };
        let mut open = true;
        let mut chosen = None;
        let id = egui::Id::unique("popup-button-menu");
        let shown = egui::Popup::new(id, ctx.clone(), PopupAnchor::Position(pos), egui::LayerId::background())
            .kind(egui::PopupKind::Menu)
            .layout(Layout::top_down_justified(Align::Min))
            .style(egui::containers::menu::menu_style)
            .align(RectAlign::BOTTOM_START)
            .width(width)
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
            .open_bool(&mut open)
            .show(|ui| {
                for (i, item) in menu.items.iter().enumerate() {
                    if item.trim_start().starts_with('-') {
                        ui.separator();
                        continue;
                    }
                    let n = i as u32 + 1;
                    let (enabled, name) = match item.strip_prefix('(') {
                        Some(rest) => (false, rest),
                        None => (true, item.as_str()),
                    };
                    let resp = menu_item(ui, name, enabled, n == menu.selected, true, None);
                    let let_go_here = menu.dragging && released && pointer.is_some_and(|p| resp.rect.contains(p));
                    if resp.clicked() || (enabled && let_go_here) {
                        chosen = Some(n);
                    }
                }
            });
        // Only a new click outside closes the menu, not the press that
        // opened it (the menu isn't even shown on its first frame).
        let menu_rect = shown.map(|r| r.response.rect);
        let clicked_outside =
            !menu.dragging && pressed && pointer.is_some_and(|p| menu_rect.is_none_or(|r| !r.contains(p)));
        if released {
            menu.dragging = false;
        }
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let obj = menu.obj;
        let Some(obj) = obj else {
            // A script's menu: send back the choice, or none.
            if chosen.is_some() || !open || clicked_outside || escape {
                self.popup = None;
                lock(&self.rt.world).dialog = None;
                let _ = self.rt.dialog_replies.send(DialogReply::Text(chosen.map(|n| n.to_string())));
            }
            return;
        };
        if let Some(n) = chosen
            && menu.field
        {
            // A combo field: the choice becomes its text, then closeField,
            // as when its text is changed by typing.
            let text = menu.items.get(n as usize - 1).cloned().unwrap_or_default();
            self.popup = None;
            if let ObjRef::Part { layer, id, card } = obj {
                let mut w = self.world();
                w.stack.set_field_content(card, layer, id, text, Vec::new());
                w.dirty = true;
            }
            self.send(Event::Send { target: obj, message: "closeField".into(), args: vec![] });
        } else if let Some(n) = chosen {
            self.popup = None;
            if let ObjRef::Part { layer, id, card } = obj {
                let mut w = self.world();
                if let Some(p) = w.stack.part_mut(card, layer, id) {
                    p.selected_line = n;
                    w.dirty = true;
                }
            }
            self.send(Event::Send { target: obj, message: "mouseUp".into(), args: vec![] });
        } else if !open || clicked_outside || escape {
            self.popup = None;
        }
    }
}
