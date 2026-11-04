//! Paths as scripts see them. HyperCard's were Mac paths, "Disk:Folder:Stack".
//! RandomCard gives each system's own, through Rust's `std::path`, except on
//! macOS, where scripts get colon paths as HyperCard gave them: Apple's stacks
//! look for the colons to find their own folders. `the pathSeparator` tells a
//! script which separator it gets: ":" on macOS, "\" on Windows, "/" elsewhere.

use std::path::{Component, Path, PathBuf};

/// The separator in the paths scripts see, `the pathSeparator`.
pub const SEPARATOR: &str = if cfg!(target_os = "macos") {
    ":"
} else if cfg!(windows) {
    "\\"
} else {
    "/"
};

/// A path as a script sees it.
pub fn shown(p: &Path) -> String {
    if cfg!(target_os = "macos") { to_mac(p, boot_volume()) } else { p.display().to_string() }
}

/// A folder's path as a script sees it, ending in the separator so a script
/// can put a file's name after it, as after a Mac folder path's colon.
pub fn folder(p: &Path) -> String {
    let s = shown(p);
    if s.ends_with(SEPARATOR) { s } else { s + SEPARATOR }
}

/// A path from a script, as the system names it.
pub fn native(s: &str) -> PathBuf {
    if cfg!(target_os = "macos") { from_mac(s, boot_volume()) } else { PathBuf::from(s) }
}

/// The name of the disk macOS started from: the entry in /Volumes that
/// leads back to "/".
fn boot_volume() -> &'static str {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        std::fs::read_dir("/Volumes")
            .into_iter()
            .flatten()
            .flatten()
            .find(|e| std::fs::canonicalize(e.path()).is_ok_and(|p| p == Path::new("/")))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .unwrap_or_else(|| "Macintosh HD".into())
    })
}

/// A file's name in a Mac path: a "/" in a name is a ":" on disk, as the
/// Finder shows it.
fn mac_name(c: &std::ffi::OsStr) -> String {
    c.to_string_lossy().replace(':', "/")
}

/// "/Users/x/Stack" as "Boot:Users:x:Stack", and "/Volumes/Disk/Stack" as
/// "Disk:Stack". A relative path starts with a colon, as on the Mac.
fn to_mac(p: &Path, boot: &str) -> String {
    let mut names = Vec::new();
    for c in p.components() {
        match c {
            Component::Normal(n) => names.push(mac_name(n)),
            Component::ParentDir => names.push(String::new()),
            _ => {}
        }
    }
    if !p.is_absolute() {
        return match names.as_slice() {
            [one] => one.clone(),
            _ => format!(":{}", names.join(":")),
        };
    }
    let (disk, rest) = match names.as_slice() {
        [v, disk, rest @ ..] if v == "Volumes" => (disk.clone(), rest),
        rest => (boot.to_string(), rest),
    };
    if rest.is_empty() { format!("{disk}:") } else { format!("{disk}:{}", rest.join(":")) }
}

/// A Mac path back to the system's: "Disk:Folder:File" from its disk,
/// ":Folder:File" from where the script is, "::" a folder up. A path with no
/// colon, or starting with "/", is a POSIX path already.
fn from_mac(s: &str, boot: &str) -> PathBuf {
    let s = s.trim();
    if s.starts_with('/') || !s.contains(':') {
        return PathBuf::from(s);
    }
    let s = s.strip_suffix(':').unwrap_or(s);
    let (mut p, rest) = match s.strip_prefix(':') {
        Some(rest) => (PathBuf::new(), rest),
        None => {
            let (disk, rest) = s.split_once(':').unwrap_or((s, ""));
            let p = if disk.eq_ignore_ascii_case(boot) { PathBuf::from("/") } else { Path::new("/Volumes").join(disk) };
            (p, rest)
        }
    };
    if !rest.is_empty() {
        for name in rest.split(':') {
            if name.is_empty() { p.push("..") } else { p.push(name.replace('/', ":")) }
        }
    }
    p
}

// Mac colon paths are used on macOS only, and these cases are POSIX paths,
// which Windows doesn't treat as absolute.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn mac_paths_both_ways() {
        let boot = "Macintosh HD";
        for (posix, mac) in [
            ("/Users/x/Tech Notes", "Macintosh HD:Users:x:Tech Notes"),
            ("/Volumes/Dev CD/Stacks/A", "Dev CD:Stacks:A"),
            ("/Volumes/Dev CD", "Dev CD:"),
            ("/", "Macintosh HD:"),
            ("/Users/x/a:b", "Macintosh HD:Users:x:a/b"),
            ("Stack", "Stack"),
            ("Folder/Stack", ":Folder:Stack"),
        ] {
            assert_eq!(to_mac(Path::new(posix), boot), mac, "{posix}");
            assert_eq!(from_mac(mac, boot), PathBuf::from(posix), "{mac}");
        }
        assert_eq!(from_mac("Dev CD:Stacks:", boot), PathBuf::from("/Volumes/Dev CD/Stacks"));
        assert_eq!(from_mac("macintosh hd:Users", boot), PathBuf::from("/Users"));
        assert_eq!(from_mac(":Folder::Other:A", boot), PathBuf::from("Folder/../Other/A"));
        assert_eq!(from_mac("/already/posix", boot), PathBuf::from("/already/posix"));
    }
}
