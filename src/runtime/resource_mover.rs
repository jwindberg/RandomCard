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

/// One of a stack's resources, taken out to copy.
#[derive(Clone)]
enum Resource {
    Icon(crate::model::Icon),
    Sound(crate::model::Sound),
    Palette(crate::model::Palette),
    Picture(crate::model::NamedPicture),
    Song(crate::model::Song),
}

/// What happened, in the XCMDs' words: empty if it worked.
type Outcome = Result<(), String>;

/// A resource type as the XCMDs took it ("snd " with its space, any case).
fn kind(t: &str) -> String {
    t.trim().to_ascii_uppercase()
}

fn take(s: &Stack, t: &str, id: i32) -> Option<Resource> {
    Some(match kind(t).as_str() {
        "ICON" => Resource::Icon(s.icons.get(&id)?.clone()),
        "SND" => Resource::Sound(s.sounds.get(&id)?.clone()),
        "PLTE" => Resource::Palette(s.palettes.get(&id)?.clone()),
        "PICT" => Resource::Picture(s.pictures.get(&id)?.clone()),
        "MODR" => Resource::Song(s.songs.get(&id)?.clone()),
        _ => return None,
    })
}

/// A palette's picture is part of its palette here: a PICT with a
/// palette's id is there when the palette is.
fn palette_picture(s: &Stack, t: &str, id: i32) -> bool {
    kind(t) == "PICT" && s.palettes.contains_key(&id) && !s.pictures.contains_key(&id)
}

/// Put a resource in, at its id or, if that's taken, the next free one.
fn put(s: &mut Stack, r: Resource, id: i32) {
    fn free<V>(m: &std::collections::BTreeMap<i32, V>, id: i32) -> i32 {
        if m.contains_key(&id) { m.keys().max().map_or(id, |k| k + 1) } else { id }
    }
    match r {
        Resource::Icon(v) => {
            let id = free(&s.icons, id);
            s.icons.insert(id, v);
        }
        Resource::Sound(v) => {
            let id = free(&s.sounds, id);
            s.sounds.insert(id, v);
        }
        Resource::Palette(v) => {
            let id = free(&s.palettes, id);
            s.palettes.insert(id, v);
        }
        Resource::Picture(v) => {
            let id = free(&s.pictures, id);
            s.pictures.insert(id, v);
        }
        Resource::Song(v) => {
            let id = free(&s.songs, id);
            s.songs.insert(id, v);
        }
    }
}

fn remove(s: &mut Stack, t: &str, id: i32) -> bool {
    match kind(t).as_str() {
        "ICON" => s.icons.remove(&id).is_some(),
        "SND" => s.sounds.remove(&id).is_some(),
        "PLTE" => s.palettes.remove(&id).is_some(),
        "PICT" => s.pictures.remove(&id).is_some(),
        "MODR" => s.songs.remove(&id).is_some(),
        _ => false,
    }
}

fn rename(r: &mut Resource, name: &str) {
    match r {
        Resource::Icon(v) => v.name = name.into(),
        Resource::Sound(v) => v.name = name.into(),
        Resource::Palette(v) => v.name = name.into(),
        Resource::Picture(v) => v.name = name.into(),
        Resource::Song(v) => v.name = name.into(),
    }
}

impl Interp {
    /// Read a stack, the current one or another by name or path.
    fn with_stack<T>(&mut self, name: &str, f: impl FnOnce(&Stack) -> T) -> R<Option<T>> {
        let Some(obj) = self.stack_ref(name)? else { return Ok(None) };
        let w = self.w();
        let stack = match obj {
            ObjRef::OtherStack(k) => w.other(k),
            _ => Some(&w.stack),
        };
        Ok(stack.map(f))
    }

    /// Change a stack, the current one or another, which is saved as other
    /// stacks a script changes are.
    fn change_stack(&mut self, name: &str, f: impl FnOnce(&mut Stack) -> Outcome) -> R<Outcome> {
        let Some(obj) = self.stack_ref(name)? else { return Ok(Err(format!("Error: can't find stack \"{name}\"."))) };
        let change = |me: &mut Interp| {
            let mut w = me.w();
            let r = f(&mut w.stack);
            if r.is_ok() {
                w.dirty = true;
            }
            Ok(r)
        };
        match obj {
            ObjRef::OtherStack(k) => self.in_stack(k, change),
            _ => change(self),
        }
    }

    /// CopyRes type, id, from, to; ResRemove stack, type, id; EditResource
    /// stack, type, id, new name, new id: Power Tools' Resource Mover, on a
    /// stack's icons, sounds, palettes, pictures and songs. Problems go in
    /// the result, starting "Error:", as the XCMDs reported them.
    pub(super) fn resource_mover(&mut self, name: &str, args: &[String]) -> Option<R<()>> {
        if !matches!(name, "copyres" | "resremove" | "editresource") {
            return None;
        }
        let arg = |i: usize| args.get(i).map(|a| a.trim().to_string()).unwrap_or_default();
        let number = |i: usize| arg(i).parse::<f64>().ok().map(|n| n as i32);
        let outcome = match name {
            "copyres" => {
                let (t, from, to) = (arg(0), arg(2), arg(3));
                match number(1) {
                    None => Ok(Err("Error: the resource ID isn't a number.".into())),
                    Some(id) => self.with_stack(&from, |s| (take(s, &t, id), palette_picture(s, &t, id))).and_then(|found| match found {
                        None => Ok(Err(format!("Error: can't find stack \"{from}\"."))),
                        Some((_, true)) => Ok(Ok(())),
                        Some((None, false)) => Ok(Err(format!("Error: there's no {t} {id} in \"{from}\"."))),
                        Some((Some(r), false)) => self.change_stack(&to, |s| {
                            put(s, r, id);
                            Ok(())
                        }),
                    }),
                }
            }
            "resremove" => {
                let (stack, t) = (arg(0), arg(1));
                match number(2) {
                    None => Ok(Err("Error: the resource ID isn't a number.".into())),
                    Some(id) => self.change_stack(&stack, |s| {
                        if remove(s, &t, id) || palette_picture(s, &t, id) {
                            Ok(())
                        } else {
                            Err(format!("Error: there's no {t} {id}."))
                        }
                    }),
                }
            }
            _ => {
                let (stack, t, new_name) = (arg(0), arg(1), arg(3));
                match (number(2), number(4).or(number(2))) {
                    (Some(id), Some(new_id)) => self.change_stack(&stack, |s| {
                        let Some(mut r) = take(s, &t, id) else { return Err(format!("Error: there's no {t} {id}.")) };
                        rename(&mut r, &new_name);
                        remove(s, &t, id);
                        put(s, r, new_id);
                        Ok(())
                    }),
                    _ => Ok(Err("Error: the resource ID isn't a number.".into())),
                }
            }
        };
        Some(outcome.map(|o| self.result = o.err().unwrap_or_default()))
    }
}
