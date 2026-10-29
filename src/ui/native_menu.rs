//! The menu bar on macOS: the menus the window shows elsewhere, as native
//! menus at the top of the screen. They're rebuilt whenever they change, so
//! scripts' changes to menus show at once.

use std::collections::HashMap;

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, CheckMenuItem, IsMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use super::menus::ShownMenu;

pub struct NativeMenus {
    bar: Option<Menu>,
    shown: Vec<ShownMenu>,
    /// Each item's label and Command key, by id.
    items: HashMap<MenuId, (String, String)>,
}

impl NativeMenus {
    pub fn new() -> NativeMenus {
        NativeMenus { bar: None, shown: Vec::new(), items: HashMap::new() }
    }

    /// Show `menus` in the menu bar, if they've changed.
    pub fn update(&mut self, menus: &[ShownMenu]) {
        if self.bar.is_some() && self.shown == menus {
            return;
        }
        let bar = Menu::new();
        let mut items = HashMap::new();
        // The application menu. Quit is RandomCard's own, so it can ask about
        // an unsaved stack first.
        let app = Submenu::new("RandomCard", true);
        let about = PredefinedMenuItem::about(
            Some("About RandomCard"),
            Some(AboutMetadata {
                name: Some("RandomCard".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                copyright: Some("Copyright 2026 jwindberg. MIT License.".into()),
                ..Default::default()
            }),
        );
        let quit = MenuItem::new("Quit RandomCard", true, accelerator("q"));
        items.insert(quit.id().clone(), ("Quit RandomCard".to_string(), "q".to_string()));
        let _ = app.append_items(&[
            &about,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &quit,
        ]);
        let _ = bar.append(&app);
        for menu in menus {
            let submenu = Submenu::new(&menu.name, menu.enabled);
            for i in &menu.items {
                if i.separator {
                    let _ = submenu.append(&PredefinedMenuItem::separator());
                    continue;
                }
                let key = accelerator(&i.key);
                let item: Box<dyn IsMenuItem> = if i.checked {
                    Box::new(CheckMenuItem::new(&i.label, i.enabled, true, key))
                } else {
                    Box::new(MenuItem::new(&i.label, i.enabled, key))
                };
                let _ = submenu.append(item.as_ref());
                items.insert(item.id().clone(), (i.label.clone(), i.key.clone()));
            }
            let _ = bar.append(&submenu);
        }
        bar.init_for_nsapp();
        self.bar = Some(bar);
        self.items = items;
        self.shown = menus.to_vec();
    }

    /// The label and Command key of the item with this id.
    pub fn item(&self, id: &MenuId) -> Option<(String, String)> {
        self.items.get(id).cloned()
    }
}

/// ⌘ and a key, for a menu item's shortcut.
fn accelerator(key: &str) -> Option<Accelerator> {
    let code = match key.chars().next()?.to_ascii_lowercase() {
        c @ 'a'..='z' => [
            Code::KeyA, Code::KeyB, Code::KeyC, Code::KeyD, Code::KeyE, Code::KeyF, Code::KeyG, Code::KeyH,
            Code::KeyI, Code::KeyJ, Code::KeyK, Code::KeyL, Code::KeyM, Code::KeyN, Code::KeyO, Code::KeyP,
            Code::KeyQ, Code::KeyR, Code::KeyS, Code::KeyT, Code::KeyU, Code::KeyV, Code::KeyW, Code::KeyX,
            Code::KeyY, Code::KeyZ,
        ][(c as u8 - b'a') as usize],
        c @ '0'..='9' => [
            Code::Digit0, Code::Digit1, Code::Digit2, Code::Digit3, Code::Digit4, Code::Digit5, Code::Digit6,
            Code::Digit7, Code::Digit8, Code::Digit9,
        ][(c as u8 - b'0') as usize],
        '.' => Code::Period,
        _ => return None,
    };
    Some(Accelerator::new(Modifiers::META, code))
}

impl super::App {
    /// Each frame on a Mac: keep the native menus up to date, and act on
    /// the items chosen since the last frame.
    pub(super) fn native_menu_bar(&mut self, ctx: &eframe::egui::Context) {
        let shown = self.shown_menus();
        self.native_menus.update(&shown);
        while let Ok(id) = self.native_events.try_recv() {
            let Some((label, key)) = self.native_menus.item(&id) else { continue };
            // A menu takes its Command key before the window sees it, so a
            // chosen item with ⌘ held was typed: it goes through
            // commandKeyDown, as other Command keys do.
            if !key.is_empty() && ctx.input(|i| i.modifiers.command) {
                self.command_key(ctx, &key, crate::runtime::world::KeyAction::Menu(label));
            } else {
                self.choose_menu_item(&label);
            }
        }
    }
}
