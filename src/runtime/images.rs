//! The pictures image parts show, loaded once and kept for every image that
//! shows the same one: from the web (loaded in the background), a file (by
//! its path, or beside the stack), or the stack's own pictures (by name or
//! id).
//!
//! An image's contents are its source, as a field's are its text; this is
//! where a source becomes a picture.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::model::{Picture, Stack};

/// Where an image's picture is, as far as there is one.
#[derive(Clone, Debug, PartialEq)]
pub enum Image {
    /// On its way from the web.
    Loading,
    Ready(Arc<Picture>),
    /// Why it couldn't be shown.
    Failed(String),
}

/// How long a picture from the web may take.
const TIMEOUT: Duration = Duration::from_secs(30);

fn cache() -> &'static Mutex<HashMap<String, Image>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Image>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Web pictures that have come (or failed) since the UI last asked.
fn arrivals() -> &'static Mutex<Vec<String>> {
    static ARRIVED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    ARRIVED.get_or_init(Default::default)
}

/// What to do when a web picture comes: the UI redraws.
static NOTIFY: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

pub fn on_arrival(f: impl Fn() + Send + Sync + 'static) {
    let _ = NOTIFY.set(Box::new(f));
}

/// The web addresses whose pictures have come or failed since last asked.
pub fn arrived() -> Vec<String> {
    std::mem::take(&mut *guard(arrivals()))
}

fn guard<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// An address on the web: http or https.
pub fn is_web(source: &str) -> bool {
    crate::runtime::movies::is_url(source)
}

/// The picture for a source, loading it the first time (from the web, in
/// the background: Loading until it comes), or None for an empty source.
pub fn lookup(source: &str, stack: &Stack) -> Option<Image> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if is_web(source) {
        let mut known = guard(cache());
        if let Some(i) = known.get(source) {
            return Some(i.clone());
        }
        known.insert(source.to_string(), Image::Loading);
        drop(known);
        let url = source.to_string();
        // A picture under a connection's Address is fetched with its
        // credentials, as the stack's requests are.
        let credentials = super::connections::sign(&mut super::connections::store(), stack, &url);
        std::thread::spawn(move || {
            let fetched = credentials
                .map_err(super::connections::SignError::why)
                .and_then(|c| crate::web::http::get_bytes(&url, c.as_ref(), TIMEOUT));
            let image = match fetched.and_then(|b| crate::paint::image_file::decode(&b)) {
                Ok(p) => Image::Ready(Arc::new(p)),
                Err(e) => Image::Failed(e),
            };
            guard(cache()).insert(url.clone(), image);
            guard(arrivals()).push(url);
            if let Some(f) = NOTIFY.get() {
                f();
            }
        });
        return Some(Image::Loading);
    }
    // The stack's own pictures, by name or id, as the picture command finds
    // them.
    let own = match source.parse::<i32>() {
        Ok(id) => stack.pictures.get(&id),
        Err(_) => stack.pictures.values().find(|p| p.name.eq_ignore_ascii_case(source)),
    };
    let (key, from) = match own {
        Some(p) => (format!("stack {}:{source}:{}x{}", stack.name, p.picture.width, p.picture.height), From::Stack(&p.picture)),
        None => {
            let path = file_path(source, stack);
            (path.to_string_lossy().to_string(), From::File(path))
        }
    };
    let mut known = guard(cache());
    if let Some(i) = known.get(&key) {
        return Some(i.clone());
    }
    let image = match from {
        From::Stack(p) => Image::Ready(Arc::new(p.clone())),
        From::File(path) => match std::fs::read(&path) {
            Ok(bytes) => match crate::paint::image_file::decode(&bytes) {
                Ok(p) => Image::Ready(Arc::new(p)),
                Err(e) => Image::Failed(e),
            },
            Err(_) => Image::Failed(format!("There's no picture file \"{source}\"")),
        },
    };
    known.insert(key, image.clone());
    Some(image)
}

/// A web picture as it stands, without starting to load it.
pub fn web_state(url: &str) -> Option<Image> {
    guard(cache()).get(url.trim()).cloned()
}

enum From<'a> {
    Stack(&'a Picture),
    File(PathBuf),
}

/// A file named by an image: as given, or beside the stack.
fn file_path(source: &str, stack: &Stack) -> PathBuf {
    let path = crate::runtime::paths::native(source);
    if path.is_absolute() {
        return path;
    }
    match stack.path.as_ref().or(stack.imported_from.as_ref()).and_then(|p| p.parent()) {
        Some(dir) => dir.join(path),
        None => path,
    }
}
