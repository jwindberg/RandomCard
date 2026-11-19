//! Resources, from the Resource Mover in Apple's Power Tools: a stack's
//! resources, listed. A RandomCard stack has no resource fork, but it keeps
//! what HyperCard kept there: icons, sounds, palettes, pictures and songs.
//! They're listed as the resources they were.

use super::interp::{Interp, R};
use super::world::ObjRef;
use crate::model::{Picture, Stack};

impl Interp {
    /// Resources([stack]): a line for each of the stack's resources (the
    /// current stack's, with no name), as Power Tools' XFCN gave them:
    /// "type id size name", the type four characters long.
    pub(super) fn resources_function(&mut self, args: &[String]) -> R<String> {
        let name = args.first().map(|a| a.trim().to_string()).unwrap_or_default();
        let Some(obj) = self.stack_ref(&name)? else {
            return Ok(String::new());
        };
        let w = self.w();
        let stack = match obj {
            ObjRef::OtherStack(k) => w.other(k),
            _ => Some(&w.stack),
        };
        Ok(stack.map(resource_list).unwrap_or_default())
    }
}

/// A stack's resources, a line each: "type id size name".
pub fn resource_list(s: &Stack) -> String {
    let picture_size = |p: &Picture| (p.width * p.height) as usize;
    let mut lines = Vec::new();
    let mut line = |kind: &str, id: i32, size: usize, name: &str| lines.push(format!("{kind:<4} {id} {size} {name}"));
    for (id, icon) in &s.icons {
        // HyperCard's black and white icons took 128 bytes.
        let size = if icon.picture.width == 32 && icon.picture.height == 32 { 128 } else { picture_size(&icon.picture) };
        line("ICON", *id, size, &icon.name);
    }
    for (id, sound) in &s.sounds {
        line("snd", *id, sound.samples.len(), &sound.name);
    }
    // A palette was two resources: its PLTE and the PICT it drew.
    for (id, palette) in &s.palettes {
        line("PLTE", *id, 24 + palette.buttons.len() * 16, &palette.name);
        line("PICT", *id, picture_size(&palette.picture), &palette.name);
    }
    for (id, picture) in &s.pictures {
        if !s.palettes.contains_key(id) {
            line("PICT", *id, picture_size(&picture.picture), &picture.name);
        }
    }
    for (id, song) in &s.songs {
        line("MODr", *id, song.data.len(), &song.name);
    }
    lines.join("\n")
}
