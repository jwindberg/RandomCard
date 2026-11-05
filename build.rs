//! Gives the Windows executable its icon: RandomCard's icon (src/icon.rs,
//! the one the window shows) as a .ico with several sizes, embedded as a
//! resource. Embedding needs Windows' resource compiler, so it happens
//! when building on Windows, as releases are; a Windows check from
//! another system skips it.

#[path = "src/icon.rs"]
#[allow(dead_code)]
mod icon;
#[path = "tools/icon_files.rs"]
#[allow(dead_code)]
mod icon_files;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=tools/icon_files.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") || !cfg!(windows) {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("randomcard.ico");
    let images: Vec<(u32, Vec<u8>)> =
        [16u32, 32, 48, 64, 128, 256].iter().map(|&s| (s, icon_files::png(s, &icon::rgba(s as usize)))).collect();
    std::fs::write(&out, icon_files::ico(&images)).expect("writing the icon");
    let mut res = winresource::WindowsResource::new();
    res.set_icon(out.to_str().expect("a UTF-8 path"));
    res.compile().expect("embedding the icon");
}
