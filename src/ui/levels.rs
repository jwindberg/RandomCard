//! User levels and stack protection in the interface: what each level
//! allows, the Preferences window, Protect Stack, and password prompts.

use eframe::egui::{self, Color32, RichText};

use crate::model::{password, shortcut};
use crate::runtime::settings::level_name;
use crate::runtime::world::Tool;

use super::App;

/// What a password is being asked for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PasswordPurpose {
    /// Lift the stack's user-level limit for this session.
    Unlock,
    /// Open Protect Stack on a password-protected stack.
    OpenProtect,
    /// Set a new password from Protect Stack.
    SetPassword,
}

pub struct PasswordPrompt {
    purpose: PasswordPurpose,
    text: String,
    confirm: String,
    error: Option<String>,
    focus: bool,
}

/// The level each tool needs: paint tools at Painting, the Button and Field
/// tools at Authoring.
pub fn tool_level(t: Tool) -> u8 {
    match t {
        Tool::Browse => 1,
        Tool::Button | Tool::Field => 4,
        _ => 3,
    }
}

impl App {
    /// The level in effect (the user level capped by the stack's protection).
    pub(super) fn level(&self) -> u8 {
        self.world().level()
    }

    /// The level for the user's own changes (Browsing on a Can't Modify stack).
    pub(super) fn editing_level(&self) -> u8 {
        self.world().editing_level()
    }

    /// Keep the tool and open windows to what the current level allows, for
    /// when a script lowers the level.
    pub(super) fn enforce_level(&mut self) {
        let (tool, editing, level) = {
            let w = self.world();
            (w.tool, w.editing_level(), w.level())
        };
        if tool_level(tool) > editing {
            self.choose_tool(Tool::Browse);
        }
        if editing < 2 && self.editing.is_some() {
            self.end_editing();
        }
        if editing < 4 {
            self.infos.clear();
        }
        if level < 5 {
            self.editors.clear();
        }
    }

    pub(super) fn ask_password(&mut self, purpose: PasswordPurpose) {
        self.password_prompt =
            Some(PasswordPrompt { purpose, text: String::new(), confirm: String::new(), error: None, focus: true });
    }

    /// Protect Stack is always reachable, so a stack can't lock its author
    /// out, but a password-protected stack asks for its password first.
    pub(super) fn open_protect_stack(&mut self) {
        let (has_password, unlocked) = {
            let w = self.world();
            (w.stack.protection.password_hash.is_some(), w.unlocked)
        };
        if has_password && !unlocked {
            self.ask_password(PasswordPurpose::OpenProtect);
        } else {
            self.protect_open = true;
        }
    }

    pub(super) fn show_password_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = &mut self.password_prompt else { return };
        let setting = prompt.purpose == PasswordPurpose::SetPassword;
        let mut done: Option<bool> = None;
        egui::Modal::new(egui::Id::unique("password-prompt")).show(ctx, |ui| {
            ui.set_min_width(300.0);
            ui.label(RichText::new(match prompt.purpose {
                PasswordPurpose::Unlock => "Enter the stack's password to raise the user level:",
                PasswordPurpose::OpenProtect => "Enter the stack's password to change its protection:",
                PasswordPurpose::SetPassword => "New password for this stack:",
            }));
            let resp = ui.add(egui::TextEdit::singleline(&mut prompt.text).password(true).desired_width(280.0));
            if prompt.focus {
                resp.request_focus();
                prompt.focus = false;
            }
            if setting {
                ui.label("Type it again:");
                ui.add(egui::TextEdit::singleline(&mut prompt.confirm).password(true).desired_width(280.0));
            }
            if let Some(e) = &prompt.error {
                ui.colored_label(Color32::from_rgb(170, 0, 0), e);
            }
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("OK").strong()).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    done = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    done = Some(false);
                }
            });
        });
        match done {
            Some(false) => self.password_prompt = None,
            Some(true) => self.submit_password(),
            None => {}
        }
    }

    fn submit_password(&mut self) {
        let hash = self.world().stack.protection.password_hash;
        let Some(prompt) = &mut self.password_prompt else { return };
        let (purpose, text) = (prompt.purpose, prompt.text.clone());
        if purpose == PasswordPurpose::SetPassword {
            if text.is_empty() {
                prompt.error = Some("The password can't be empty.".into());
                return;
            }
            if text != prompt.confirm {
                prompt.error = Some("The two passwords don't match.".into());
                prompt.focus = true;
                return;
            }
            self.password_prompt = None;
            let mut w = self.world();
            w.stack.protection.password_hash = Some(password::stored_hash(&text));
            w.unlocked = true;
            w.dirty = true;
            return;
        }
        if !hash.is_some_and(|h| password::check(&text, h)) {
            prompt.error = Some("That isn't the stack's password.".into());
            prompt.text.clear();
            prompt.focus = true;
            return;
        }
        self.password_prompt = None;
        self.world().unlocked = true;
        if purpose == PasswordPurpose::OpenProtect {
            self.protect_open = true;
        }
    }

    /// Preferences: the user level (HyperCard kept it in the Home stack).
    pub(super) fn show_preferences(&mut self, ctx: &egui::Context) {
        if !self.prefs_open {
            return;
        }
        let mut open = true;
        let (current, limit, has_password) = {
            let w = self.world();
            (w.level(), w.level_limit(), w.stack.protection.password_hash.is_some())
        };
        let mut unlock = false;
        egui::Window::new("Preferences").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new("User level").strong());
            for level in (1..=5u8).rev() {
                let allowed = level <= limit;
                let label = format!("{level}  {}", level_name(level));
                let resp = ui.add_enabled(allowed, egui::RadioButton::new(current == level, label));
                if resp.clicked() {
                    let mut w = self.world();
                    w.user_level = level;
                    drop(w);
                    self.settings.user_level = level;
                    self.settings.save();
                }
            }
            if limit < 5 {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("This stack allows up to {}.", level_name(limit))).weak());
                if has_password && ui.button("Unlock with Password…").clicked() {
                    unlock = true;
                }
            }
            ui.separator();
            let mut show = self.settings.show_tools;
            if ui.checkbox(&mut show, format!("Show the tool panel ({})", shortcut("T"))).changed() {
                self.settings.show_tools = show;
                self.settings.save();
            }
        });
        self.prefs_open = open;
        if unlock {
            self.ask_password(PasswordPurpose::Unlock);
        }
    }

    /// Protect Stack: Can't Modify and friends, the level limit and a password.
    pub(super) fn show_protect_stack(&mut self, ctx: &egui::Context) {
        if !self.protect_open {
            return;
        }
        let mut open = true;
        let mut set_password = false;
        egui::Window::new("Protect Stack").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            let mut w = self.world();
            let before = (w.stack.cant_modify, w.stack.protection.clone());
            ui.checkbox(&mut w.stack.cant_modify, "Can't modify stack");
            let p = &mut w.stack.protection;
            ui.checkbox(&mut p.cant_delete, "Can't delete stack");
            ui.checkbox(&mut p.cant_abort, format!("Can't abort ({} won't stop scripts)", shortcut(".")));
            ui.checkbox(&mut p.cant_peek, "Can't peek (no script shortcuts)");
            let has_password = p.password_hash.is_some();
            ui.add_enabled(has_password, egui::Checkbox::new(&mut p.private_access, "Private access (password to open)"))
                .on_disabled_hover_text("Set a password first");
            ui.separator();
            ui.label(RichText::new("Limit user level to").strong());
            for level in (1..=5u8).rev() {
                ui.radio_value(&mut p.user_level_limit, level, format!("{level}  {}", level_name(level)));
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(if has_password { "Change Password…" } else { "Set Password…" }).clicked() {
                    set_password = true;
                }
                if has_password && ui.button("Remove Password").clicked() {
                    p.password_hash = None;
                    p.private_access = false;
                }
            });
            ui.label(RichText::new("Protection keeps a stack from being changed by accident. It isn't encryption.").weak().small());
            if (w.stack.cant_modify, w.stack.protection.clone()) != before {
                w.dirty = true;
            }
        });
        if set_password {
            self.ask_password(PasswordPurpose::SetPassword);
        }
        if !open {
            // Like HyperCard, the new limit applies as soon as the dialog
            // closes; getting back in takes the password again.
            self.protect_open = false;
            self.world().unlocked = false;
        }
    }
}
