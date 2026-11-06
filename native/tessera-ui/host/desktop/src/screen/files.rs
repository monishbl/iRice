//! Opening files and folders (shell/services/Files.qml). Folders and the
//! Trash open in a file manager (Settings' Files app, else the first
//! installed), never in whatever app has claimed folders system-wide; files
//! open in their default app. Tools.qml (which optional tools exist) is
//! `util::has`, a PATH lookup.

use super::util;
use serde_json::Value;
use tessera_core::host::{Host, Service};

pub const MANAGERS: &[&str] = &["nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm-qt", "pcmanfm"];

/// A folder in the file manager
pub fn open_folder(cfg: &Value, path: &str) {
    let m = util::file_manager(cfg);
    if m.is_empty() {
        util::detached(&["xdg-open", path]);
    } else {
        util::detached(&[m.as_str(), path]);
    }
}

/// A folder in the file manager, anything else in its app
pub fn open(cfg: &Value, path: &str) {
    let m = util::file_manager(cfg);
    let opener = if m.is_empty() { "xdg-open".to_string() } else { m };
    util::detached(&["sh", "-c", "if [ -d \"$1\" ]; then exec \"$2\" \"$1\"; else exec xdg-open \"$1\"; fi", "sh", path, &opener]);
}

/// The folder it's in, with it selected where the file manager can
pub fn reveal(cfg: &Value, path: &str) {
    let m = util::file_manager(cfg);
    if matches!(m.as_str(), "nautilus" | "dolphin" | "nemo") {
        util::detached(&[m.as_str(), "--select", path]);
    } else {
        let dir = path.rsplit_once('/').map(|(d, _)| d).filter(|d| !d.is_empty()).unwrap_or("/");
        open_folder(cfg, dir);
    }
}

pub fn open_trash(cfg: &Value) {
    let m = util::file_manager(cfg);
    if m.is_empty() {
        util::detached(&["xdg-open", "trash:///"]);
    } else {
        util::detached(&[m.as_str(), if m == "dolphin" { "trash:/" } else { "trash:///" }]);
    }
}

/// `files.*` calls: files.open (a surface opens a path; absolute only, as
/// RustUi.qml checks), plus the folder, reveal and trash forms
pub struct FilesSvc;

impl Service for FilesSvc {
    fn name(&self) -> &'static str {
        "Files"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["files"]
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let path = args[0].as_str().unwrap_or("").to_string();
        let cfg = h.cfg().clone();
        match call {
            "files.open" if path.starts_with('/') => open(&cfg, &path),
            "files.openFolder" if path.starts_with('/') => open_folder(&cfg, &path),
            "files.reveal" if path.starts_with('/') => reveal(&cfg, &path),
            "files.openTrash" => open_trash(&cfg),
            _ => {}
        }
    }
}
