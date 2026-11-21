//! Dialogs a stack carries, as the Mac kept them: a 'DLOG' (the window: its
//! rect, kind and title) and the 'DITL' it names (its items). Dialoger, the
//! external the Developer Info Assistant and Tools Advisor build their
//! dialogs with, shows them by name.

use serde::{Deserialize, Serialize};

use super::Rect;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DialogTemplate {
    /// The DLOG's resource name, which scripts use.
    pub name: String,
    pub title: String,
    /// Where it goes on the screen.
    pub rect: Rect,
    /// The Mac's window kind: 1 a plain dialog, 0 and 4 document windows,
    /// 2 a plain box, 3 a shadowed box; 16 and up, a rounded window. Above
    /// 2047, a floating window (a WDEF of the stack's own).
    pub proc_id: i16,
    pub close_box: bool,
    pub items: Vec<DialogItem>,
}

/// One item of a dialog, where it is in the dialog.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DialogItem {
    pub kind: DialogItemKind,
    pub rect: Rect,
    /// The Mac's "enabled": a click on it is reported.
    pub enabled: bool,
    /// A button's, check box's or radio button's name; static or editable
    /// text.
    pub text: String,
    /// An icon's or picture's id, a control's 'CNTL', or for a user item,
    /// the kind Dialoger gives it (1 a list, 2 a pop-up, 4 a box).
    pub id: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogItemKind {
    #[default]
    User,
    Button,
    CheckBox,
    RadioButton,
    Control,
    StaticText,
    EditText,
    Icon,
    Picture,
}

impl DialogItemKind {
    /// From a DITL item's type byte (without its "disabled" bit).
    pub fn from_mac(t: u8) -> DialogItemKind {
        match t {
            4 => DialogItemKind::Button,
            5 => DialogItemKind::CheckBox,
            6 => DialogItemKind::RadioButton,
            7 => DialogItemKind::Control,
            8 => DialogItemKind::StaticText,
            16 => DialogItemKind::EditText,
            32 => DialogItemKind::Icon,
            64 => DialogItemKind::Picture,
            _ => DialogItemKind::User,
        }
    }
}
