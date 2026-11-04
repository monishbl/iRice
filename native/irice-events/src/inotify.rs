//! inotify, and the clipboard history source built on it.
//!
//! The clipboard: the shell only needs to know that cliphist stored
//! something (the session's `wl-paste --watch cliphist store` does that on
//! every copy), so it lists the history again. cliphist's database being
//! written says exactly that, without a Wayland connection of our own.
//! Reported as {"src":"clipboard","kind":"changed"} (args {"path": db}).
//! Only writes count: `cliphist list` opens the database read-write too, so
//! a close-after-write would answer the shell's own look.

use crate::{emit, stop::Stop};
use serde_json::{json, Value};
use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Inotify(OwnedFd);

pub struct Event {
    pub wd: i32,
    pub mask: u32,
    pub name: String,
}

impl Inotify {
    pub fn new() -> std::io::Result<Inotify> {
        let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Inotify(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    pub fn fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }

    pub fn add(&self, path: &Path, mask: u32) -> Option<i32> {
        let c = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let wd = unsafe { libc::inotify_add_watch(self.fd(), c.as_ptr(), mask) };
        (wd >= 0).then_some(wd)
    }

    pub fn remove(&self, wd: i32) {
        unsafe { libc::inotify_rm_watch(self.fd(), wd) };
    }

    /// Everything waiting (nothing when there isn't).
    pub fn read(&self) -> Vec<Event> {
        let mut out = vec![];
        let mut buf = [0u8; 8192];
        loop {
            let n = unsafe { libc::read(self.fd(), buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                return out;
            }
            out.extend(parse(&buf[..n as usize]));
        }
    }
}

/// inotify_event records: wd, mask, cookie, name length, then the name
/// (NUL-padded).
pub fn parse(mut b: &[u8]) -> Vec<Event> {
    let mut out = vec![];
    while b.len() >= 16 {
        let w = |i: usize| u32::from_ne_bytes(b[i..i + 4].try_into().unwrap());
        let (wd, mask, len) = (w(0) as i32, w(4), w(12) as usize);
        if b.len() < 16 + len {
            break;
        }
        let name = String::from_utf8_lossy(&b[16..16 + len]).trim_end_matches('\0').to_string();
        out.push(Event { wd, mask, name });
        b = &b[16 + len..];
    }
    out
}

pub fn run_clipboard(stop: Stop, args: Value) {
    let Some(db) = args.get("path").and_then(Value::as_str).filter(|p| p.starts_with('/')).map(PathBuf::from) else {
        emit(json!({"src": "clipboard", "kind": "unsupported", "error": "no database path"}));
        return;
    };
    let Ok(ino) = Inotify::new() else {
        emit(json!({"src": "clipboard", "kind": "unsupported", "error": "no inotify"}));
        return;
    };
    let name = db.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let dir = db.parent().unwrap_or(Path::new("/")).to_path_buf();
    loop {
        // Its folder, or (not made yet: nothing copied ever) the nearest
        // folder above it that's there, until it is
        let (wd, watching_dir) = match ino.add(&dir, libc::IN_MODIFY | libc::IN_CREATE | libc::IN_MOVED_TO | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF) {
            Some(wd) => (wd, true),
            None => {
                let mut up = dir.as_path();
                let wd = loop {
                    up = match up.parent() {
                        Some(p) => p,
                        None => return,
                    };
                    if let Some(wd) = ino.add(up, libc::IN_CREATE | libc::IN_MOVED_TO | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF) {
                        break wd;
                    }
                };
                (wd, false)
            }
        };
        loop {
            if stop.wait(&[ino.fd()], None).is_none() {
                return;
            }
            let events = ino.read();
            let gone = events.iter().any(|e| e.wd == wd && e.mask & (libc::IN_DELETE_SELF | libc::IN_MOVE_SELF | libc::IN_IGNORED) != 0);
            if !watching_dir {
                // something appeared on the way down: look again
                if gone || events.iter().any(|e| e.wd == wd && e.mask & (libc::IN_CREATE | libc::IN_MOVED_TO) != 0) {
                    ino.remove(wd);
                    break;
                }
                continue;
            }
            if events.iter().any(|e| e.wd == wd && e.name == name && e.mask & (libc::IN_MODIFY | libc::IN_CREATE | libc::IN_MOVED_TO) != 0) {
                emit(json!({"src": "clipboard", "kind": "changed"}));
                // (one store is several writes: one cue for them all)
                if !stop.sleep(Duration::from_millis(150)) {
                    return;
                }
                ino.read();
            }
            if gone {
                ino.remove(wd);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(wd: i32, mask: u32, name: &str) -> Vec<u8> {
        let mut n = name.as_bytes().to_vec();
        if !n.is_empty() {
            n.resize((n.len() + 1).next_multiple_of(16), 0);
        }
        let mut r = vec![];
        for w in [wd as u32, mask, 0, n.len() as u32] {
            r.extend(w.to_ne_bytes());
        }
        r.extend(n);
        r
    }

    #[test]
    fn events() {
        let mut b = record(1, libc::IN_MODIFY, "db");
        b.extend(record(2, libc::IN_OPEN, ""));
        b.extend(record(1, libc::IN_CREATE, "video10"));
        b.extend(&[1, 2, 3]); // (a cut-off record is left alone)
        let e = parse(&b);
        assert_eq!(e.len(), 3);
        assert_eq!((e[0].wd, e[0].mask, e[0].name.as_str()), (1, libc::IN_MODIFY, "db"));
        assert_eq!((e[1].wd, e[1].name.as_str()), (2, ""));
        assert_eq!(e[2].name, "video10");
    }

    #[test]
    fn clipboard_database_writes() {
        let dir = std::env::temp_dir().join(format!("irice-events-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("cliphist/db");
        let stop = Stop::new();
        let (s, path) = (stop.clone(), db.to_string_lossy().to_string());
        // (emit writes to stdout; here only that it keeps running and stops is checked)
        let h = std::thread::spawn(move || run_clipboard(s, json!({"path": path})));
        std::thread::sleep(Duration::from_millis(100));
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(&db, b"x").unwrap();
        std::thread::sleep(Duration::from_millis(100));
        stop.set();
        h.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
