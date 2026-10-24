//! Scripts changing the menu bar: `create menu`, `put … into menu`,
//! `delete menuItem`, `enable`/`disable`, menu properties, `reset menuBar`.

use crate::hypertalk::ast::{ChunkIndex, MenuRef, Prep};
use crate::hypertalk::value::bool_str;

use super::interp::{Interp, R, err, to_bool};
use super::menus::{self, Menu, MenuItem};

impl Interp {
    fn menu_number(&mut self, which: &ChunkIndex) -> R<usize> {
        let count = self.w().menus.len();
        let found = match which {
            ChunkIndex::Ordinal(o) => {
                let n = self.ordinal_index(*o, count);
                (n >= 1 && n as usize <= count).then(|| n as usize - 1)
            }
            ChunkIndex::Expr(e) => {
                let name = self.eval(e)?;
                let found = menus::menu_index(&self.w().menus, &name);
                if found.is_none() {
                    return err(format!("There is no menu \"{name}\""));
                }
                found
            }
        };
        found.map_or_else(|| err("There is no such menu"), Ok)
    }

    fn item_number(&mut self, item: &ChunkIndex, menu: usize) -> R<usize> {
        let count = self.w().menus[menu].items.len();
        let found = match item {
            ChunkIndex::Ordinal(o) => {
                let n = self.ordinal_index(*o, count);
                (n >= 1 && n as usize <= count).then(|| n as usize - 1)
            }
            ChunkIndex::Expr(e) => {
                let name = self.eval(e)?;
                let found = self.w().menus[menu].item_index(&name);
                if found.is_none() {
                    let menu_name = self.w().menus[menu].name.clone();
                    return err(format!("There is no menu item \"{name}\" in menu \"{menu_name}\""));
                }
                found
            }
        };
        found.map_or_else(|| err("There is no such menu item"), Ok)
    }

    /// The menu, and the item if the reference names one.
    fn resolve_menu(&mut self, r: &MenuRef) -> R<(usize, Option<usize>)> {
        match r {
            MenuRef::Menu(m) => Ok((self.menu_number(m)?, None)),
            MenuRef::Item(i, m) => {
                let m = self.menu_number(m)?;
                Ok((m, Some(self.item_number(i, m)?)))
            }
        }
    }

    pub(super) fn menu_exists(&mut self, r: &MenuRef) -> R<String> {
        Ok(bool_str(self.resolve_menu(r).is_ok()))
    }

    /// A menu's value is its items, one per line; an item's is its name.
    pub(super) fn menu_value(&mut self, r: &MenuRef) -> R<String> {
        let (m, i) = self.resolve_menu(r)?;
        let w = self.w();
        Ok(match i {
            Some(i) => w.menus[m].items[i].name.clone(),
            None => w.menus[m].items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join("\n"),
        })
    }

    pub(super) fn create_menu(&mut self, name: &str) -> R<()> {
        let mut w = self.w();
        if menus::menu_index(&w.menus, name).is_some() {
            return err(format!("There is already a menu \"{name}\""));
        }
        w.menus.push(Menu::new(name, &[]));
        Ok(())
    }

    /// `put "New Game,-,High Scores" into menu "Game" with menuMessages …`,
    /// `put "Undo All" after menuItem "Undo" of menu "Edit"`.
    pub(super) fn put_into_menu(&mut self, r: &MenuRef, prep: Prep, value: &str, messages: Option<&str>) -> R<()> {
        let (m, i) = self.resolve_menu(r)?;
        let names = split_items(value);
        let messages = messages.map(split_items).unwrap_or_default();
        let new: Vec<MenuItem> = names
            .iter()
            .enumerate()
            .map(|(k, name)| MenuItem { message: messages.get(k).cloned().unwrap_or_default(), ..MenuItem::new(name) })
            .collect();
        let mut w = self.w();
        let items = &mut w.menus[m].items;
        let range = match (i, prep) {
            (None, Prep::Into) => 0..items.len(),
            (None, Prep::Before) => 0..0,
            (None, Prep::After) => items.len()..items.len(),
            (Some(i), Prep::Into) => i..i + 1,
            (Some(i), Prep::Before) => i..i,
            (Some(i), Prep::After) => i + 1..i + 1,
        };
        items.splice(range, new);
        Ok(())
    }

    pub(super) fn delete_menu(&mut self, r: &MenuRef) -> R<()> {
        let (m, i) = self.resolve_menu(r)?;
        let mut w = self.w();
        match i {
            Some(i) => {
                w.menus[m].items.remove(i);
            }
            None => {
                w.menus.remove(m);
            }
        }
        Ok(())
    }

    pub(super) fn reset_menu_bar(&mut self) {
        self.w().menus = menus::standard_menus();
    }

    pub(super) fn menu_property(&mut self, r: &MenuRef, prop: &str) -> R<String> {
        let (m, i) = self.resolve_menu(r)?;
        let w = self.w();
        let menu = &w.menus[m];
        Ok(match (i, prop) {
            (None, "name") => menu.name.clone(),
            (None, "enabled") => bool_str(menu.enabled),
            (None, "number") => (m + 1).to_string(),
            (Some(i), _) => {
                let item = &menu.items[i];
                match prop {
                    "name" => item.name.clone(),
                    "enabled" => bool_str(item.enabled),
                    "checkmark" => bool_str(item.checked),
                    "markchar" => if item.checked { "✓".into() } else { String::new() },
                    "menumessage" | "menumsg" => item.message.clone(),
                    "commandchar" | "cmdchar" => item.command_char.clone(),
                    "number" => (i + 1).to_string(),
                    "textstyle" => "plain".into(),
                    _ => return err(format!("Menu items don't have a \"{prop}\" property")),
                }
            }
            (None, _) => return err(format!("Menus don't have a \"{prop}\" property")),
        })
    }

    pub(super) fn set_menu_property(&mut self, r: &MenuRef, prop: &str, v: &str) -> R<()> {
        let (m, i) = self.resolve_menu(r)?;
        let on = || to_bool(v);
        let mut w = self.w();
        let menu = &mut w.menus[m];
        match (i, prop) {
            (None, "name") => menu.name = v.to_string(),
            (None, "enabled") => menu.enabled = on()?,
            (Some(i), _) => {
                let item = &mut menu.items[i];
                match prop {
                    "name" => item.name = v.to_string(),
                    "enabled" => item.enabled = on()?,
                    "checkmark" => item.checked = on()?,
                    "markchar" => item.checked = !v.is_empty(),
                    "menumessage" | "menumsg" => item.message = v.to_string(),
                    "commandchar" | "cmdchar" => item.command_char = v.chars().take(1).collect(),
                    // Menu items are drawn in the system's style.
                    "textstyle" => {}
                    _ => return err(format!("Can't set the {prop} of a menu item")),
                }
            }
            (None, _) => return err(format!("Can't set the {prop} of a menu")),
        }
        Ok(())
    }

    pub(super) fn enable_menu(&mut self, r: &MenuRef, on: bool) -> R<()> {
        self.set_menu_property(r, "enabled", &bool_str(on))
    }

    pub(super) fn count_menu_items(&mut self, menu: &ChunkIndex) -> R<usize> {
        let m = self.menu_number(menu)?;
        Ok(self.w().menus[m].items.len())
    }
}

/// Menu items in a value: one per line, or separated by commas.
fn split_items(value: &str) -> Vec<String> {
    let sep = if value.contains('\n') { '\n' } else { ',' };
    value.split(sep).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}
