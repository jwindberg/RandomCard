//! How RandomCard's built-in menus and items look at the moment: which the
//! user level and tool allow, which are enabled, how Cut and Paste name what
//! they act on, and which carry a check mark; and how an item is drawn.

use eframe::egui;

use crate::model::styles;
use crate::runtime::menus::item_key;
use crate::runtime::world::Tool;

pub struct MenuState {
    pub edit_bg: bool,
    pub tool: Tool,
    pub msg_visible: bool,
    pub level: u8,
    /// The level for the user's own changes (Browsing on a Can't Modify stack).
    pub editing: u8,
    pub cant_modify: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub tools_shown: bool,
    pub message_watcher: bool,
    pub variable_watcher: bool,
    /// What Cut, Copy and Clear act on ("Button", "Picture"…).
    pub target: Option<&'static str>,
    /// What Paste would paste.
    pub paste: Option<&'static str>,
    /// Font, size and style of the selected text, while editing text.
    pub text_style: Option<(String, u32, String)>,
}

impl MenuState {
    pub fn menu_shown(&self, menu: &str) -> bool {
        match menu {
            "Paint" => self.tool.is_paint(),
            "Font" | "Style" => self.text_style.is_some(),
            "Objects" => self.editing >= 4,
            _ => true,
        }
    }

    pub fn item_shown(&self, menu: &str, item: &str) -> bool {
        match (menu, item_key(item).as_str()) {
            ("Edit", "new card" | "delete card" | "cut card" | "copy card" | "paste card") => self.editing >= 2,
            ("Edit", "background" | "clear picture") => self.editing >= 3,
            ("Objects", "card script" | "bkgnd script" | "stack script") => self.level >= 5,
            ("View", "message watcher" | "variable watcher") => self.level >= 5,
            _ => true,
        }
    }

    /// The label, whether it's enabled, and whether it's checked.
    pub fn item_look(&self, item: &str) -> (String, bool, bool) {
        let key = item_key(item);
        let mut label = item.to_string();
        let enabled = match key.as_str() {
            "save stack" => !self.cant_modify,
            "undo" => self.can_undo && self.editing >= 2,
            "redo" => self.can_redo && self.editing >= 2,
            "cut" | "copy" | "clear" => {
                // As in HyperCard, they name what they act on: "Copy Button".
                if let Some(t) = self.target.filter(|t| !(key == "clear" && *t == "Picture")) {
                    label = format!("{item} {t}");
                }
                self.target.is_some()
            }
            "paste" => {
                if let Some(t) = self.paste {
                    label = format!("Paste {t}");
                }
                self.paste.is_some()
            }
            "tool panel" => self.editing >= 3,
            _ => true,
        };
        let checked = match key.as_str() {
            "background" => self.edit_bg,
            "message" | "message box" => self.msg_visible,
            "tool panel" => self.tools_shown,
            "message watcher" => self.message_watcher,
            "variable watcher" => self.variable_watcher,
            _ => match &self.text_style {
                Some((font, size, style)) => {
                    font.eq_ignore_ascii_case(&key)
                        || (key == "plain" && style == "plain")
                        || styles::STYLES.contains(&key.as_str()) && styles::has_style(style, &key)
                        || key.strip_suffix(" point").and_then(|n| n.parse::<u32>().ok()) == Some(*size)
                }
                None => false,
            },
        };
        (label, enabled, checked)
    }
}

/// A menu item. With `indent`, its text starts after a column for the check
/// mark, so a menu's items line up whichever of them is checked, as on the
/// Macintosh.
pub fn menu_item(
    ui: &mut egui::Ui,
    name: &str,
    enabled: bool,
    checked: bool,
    indent: bool,
    shortcut: Option<String>,
) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let format = egui::TextFormat { font_id: font.clone(), color: egui::Color32::PLACEHOLDER, ..Default::default() };
    let mut label = egui::text::LayoutJob::single_section(name.to_string(), format);
    if indent {
        label.sections[0].leading_space = font.size * 1.3;
    }
    let mut button = egui::Button::new(label);
    if let Some(s) = shortcut {
        button = button.shortcut_text(s);
    }
    let resp = ui.add_enabled(enabled, button);
    if checked {
        let at = egui::Pos2::new(resp.rect.left() + ui.spacing().button_padding.x, resp.rect.center().y);
        let color = ui.style().interact(&resp).text_color();
        ui.painter().text(at, egui::Align2::LEFT_CENTER, "✔", font, color);
    }
    resp
}
