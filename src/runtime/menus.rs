//! The menu bar as data that scripts can change: `create menu`, `put … into
//! menu`, `delete menuItem`, `disable menu`, `the checkMark of menuItem`,
//! `the menuMessage`, `reset menuBar`. The UI draws it, hiding built-in menus
//! and items that the user level or the current tool rules out.

use crate::model::styles;

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub name: String,
    pub items: Vec<MenuItem>,
    pub enabled: bool,
    /// One of RandomCard's own menus, which the UI shows only when they apply
    /// (Paint with a paint tool, Objects at the Authoring level, …).
    pub builtin: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuItem {
    /// "-" is a separator.
    pub name: String,
    pub enabled: bool,
    pub checked: bool,
    /// A message to send instead of `doMenu` when the item is chosen.
    pub message: String,
    /// The Command-key shortcut, shown in the menu.
    pub command_char: String,
    /// One of RandomCard's own items, which may also be hidden or disabled
    /// by the user level and need not be checked by scripts.
    pub builtin: bool,
}

impl MenuItem {
    pub fn new(name: &str) -> MenuItem {
        MenuItem { name: name.to_string(), enabled: true, ..MenuItem::default() }
    }

    pub fn is_separator(&self) -> bool {
        self.name.starts_with('-')
    }
}

impl Menu {
    pub fn new(name: &str, items: &[&str]) -> Menu {
        Menu { name: name.to_string(), items: items.iter().map(|i| MenuItem::new(i)).collect(), enabled: true, builtin: false }
    }

    fn builtin(name: &str, items: &[&str]) -> Menu {
        let mut m = Menu::new(name, items);
        m.builtin = true;
        for i in &mut m.items {
            i.builtin = true;
        }
        m
    }

    /// An item by name (ignoring case and a trailing "…") or number.
    pub fn item_index(&self, which: &str) -> Option<usize> {
        if let Ok(n) = which.trim().parse::<usize>() {
            return (n >= 1 && n <= self.items.len()).then(|| n - 1);
        }
        let key = item_key(which);
        self.items.iter().position(|i| item_key(&i.name) == key)
    }
}

/// How menu item names compare: no case, no trailing "…" or "...".
pub fn item_key(name: &str) -> String {
    name.trim().trim_end_matches('…').trim_end_matches("...").trim().to_ascii_lowercase()
}

/// RandomCard's standard menu bar, in HyperCard's order.
pub fn standard_menus() -> Vec<Menu> {
    let fonts: Vec<&str> = styles::FONTS.to_vec();
    let mut style: Vec<String> = vec!["Plain".into()];
    style.extend(styles::STYLES.iter().map(|s| format!("{}{}", s[..1].to_ascii_uppercase(), &s[1..])));
    style.push("-".into());
    style.extend(styles::SIZES.iter().map(|s| format!("{s} Point")));
    let style: Vec<&str> = style.iter().map(String::as_str).collect();
    let paint: Vec<&str> =
        ["Select All", "-"].into_iter().chain(crate::paint::PaintOp::ALL.iter().map(|op| op.name())).collect();
    vec![
        Menu::builtin(
            "File",
            &[
                "New Stack…",
                "Open Stack…",
                "Import HyperCard Stack…",
                "Save Stack",
                "Save a Copy…",
                "-",
                "Protect Stack…",
                "Preferences…",
                "-",
                "Print Card",
                "Print Stack…",
                "-",
                "Quit RandomCard",
            ],
        ),
        Menu::builtin(
            "Edit",
            &[
                "Undo",
                "Redo",
                "-",
                "Cut",
                "Copy",
                "Paste",
                "Clear",
                "-",
                "New Card",
                "Delete Card",
                "Cut Card",
                "Copy Card",
                "Paste Card",
                "-",
                "Background",
                "Clear Picture",
            ],
        ),
        Menu::builtin("Go", &["Back", "Home", "-", "First", "Prev", "Next", "Last", "-", "Message"]),
        Menu::builtin("View", &["Tool Panel", "Message Box", "-", "Message Watcher", "Variable Watcher"]),
        Menu::builtin("Font", &fonts),
        Menu::builtin("Style", &style),
        Menu::builtin("Paint", &paint),
        Menu::builtin(
            "Objects",
            &[
                "Button Info…",
                "Field Info…",
                "Card Info…",
                "Bkgnd Info…",
                "Stack Info…",
                "-",
                "Bring Closer",
                "Send Farther",
                "Bring to Front",
                "Send to Back",
                "-",
                "New Button",
                "New Field",
                "New Background",
                "-",
                "Card Script",
                "Bkgnd Script",
                "Stack Script",
            ],
        ),
    ]
}

/// A menu by name or number.
pub fn menu_index(menus: &[Menu], which: &str) -> Option<usize> {
    if let Ok(n) = which.trim().parse::<usize>() {
        return (n >= 1 && n <= menus.len()).then(|| n - 1);
    }
    menus.iter().position(|m| m.name.eq_ignore_ascii_case(which.trim()))
}

/// The item a user or `doMenu` chose, looked up by name in every menu.
pub fn find_item<'a>(menus: &'a [Menu], name: &str) -> Option<&'a MenuItem> {
    let key = item_key(name);
    menus.iter().flat_map(|m| &m.items).find(|i| !i.is_separator() && item_key(&i.name) == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finding_menus_and_items() {
        let menus = standard_menus();
        let edit = menu_index(&menus, "edit").unwrap();
        assert_eq!(menus[edit].item_index("new card"), Some(8));
        assert_eq!(menus[edit].item_index("1"), Some(0));
        assert_eq!(menus[0].item_index("protect stack"), Some(6));
        assert_eq!(menu_index(&menus, "2"), Some(1));
        assert!(find_item(&menus, "Bold").is_some());
        assert!(find_item(&menus, "-").is_none());
    }
}
