//! Which apps have a camera open (the shell's green dot), as camera.sh did:
//! looked for only when a camera is opened or closed (inotify on
//! /dev/video*), and not at all while every camera is runtime-suspended
//! (nobody can be using it; reading that is free). Cameras plugged in or
//! out are seen as their /dev nodes come and go, so nothing is polled.
//! Reported as {"src":"camera","apps":[process names]} when that changes,
//! and once at the start.

use crate::inotify::Inotify;
use crate::{emit, procs, stop::Stop};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Camera {
    dev: PathBuf,
    power: Option<PathBuf>, // its USB device's power/, when it has one
}

pub fn run(stop: Stop, _args: Value) {
    let Ok(ino) = Inotify::new() else {
        emit(json!({"src": "camera", "kind": "unsupported", "error": "no inotify"}));
        return;
    };
    let dev = ino.add(Path::new("/dev"), libc::IN_CREATE | libc::IN_DELETE);
    let mut watches: HashMap<i32, PathBuf> = HashMap::new();
    let mut cams = find(&ino, &mut watches);
    let mut prev: Option<Vec<String>> = None;
    loop {
        let now = if !cams.is_empty() && maybe_busy(&cams) {
            let devs: Vec<&Path> = cams.iter().map(|c| c.dev.as_path()).collect();
            procs::holding(&devs)
        } else {
            vec![]
        };
        if prev.as_ref() != Some(&now) {
            emit(json!({"src": "camera", "apps": now}));
            prev = Some(now);
        }
        if stop.wait(&[ino.fd()], None).is_none() {
            return;
        }
        // (an open or close, then let the burst settle)
        if !stop.sleep(Duration::from_millis(300)) {
            return;
        }
        let events = ino.read();
        if events.iter().any(|e| Some(e.wd) == dev && e.name.starts_with("video")) {
            cams = find(&ino, &mut watches);
        }
    }
}

fn find(ino: &Inotify, watches: &mut HashMap<i32, PathBuf>) -> Vec<Camera> {
    for wd in watches.keys() {
        ino.remove(*wd);
    }
    watches.clear();
    let mut cams = vec![];
    let Ok(dir) = std::fs::read_dir("/dev") else { return cams };
    let mut names: Vec<String> = dir.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| is_video(n)).collect();
    names.sort();
    for n in names {
        let dev = PathBuf::from("/dev").join(&n);
        if let Some(wd) = ino.add(&dev, libc::IN_OPEN | libc::IN_CLOSE_WRITE | libc::IN_CLOSE_NOWRITE) {
            watches.insert(wd, dev.clone());
        }
        cams.push(Camera { power: power_dir(&n), dev });
    }
    cams
}

fn is_video(name: &str) -> bool {
    name.strip_prefix("video").map(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).unwrap_or(false)
}

// Up from the video device to the USB device that carries power/
fn power_dir(name: &str) -> Option<PathBuf> {
    let mut d = std::fs::canonicalize(format!("/sys/class/video4linux/{name}/device")).ok()?;
    loop {
        if d.join("power/runtime_status").exists() {
            return Some(d.join("power"));
        }
        if !d.pop() || d == Path::new("/") {
            return None;
        }
    }
}

// True when a camera may be in use: one is awake, or one can't tell
fn maybe_busy(cams: &[Camera]) -> bool {
    cams.iter().any(|c| {
        let Some(p) = &c.power else { return true };
        let read = |f: &str| std::fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).ok();
        read("control").as_deref() != Some("auto") || read("runtime_status").as_deref() != Some("suspended")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_nodes() {
        assert!(is_video("video0"));
        assert!(is_video("video12"));
        assert!(!is_video("video"));
        assert!(!is_video("video0p"));
        assert!(!is_video("vhost-net"));
    }

    #[test]
    fn suspended_cameras_are_not_busy() {
        let dir = std::env::temp_dir().join(format!("tessera-cam-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("control"), "auto\n").unwrap();
        std::fs::write(dir.join("runtime_status"), "suspended\n").unwrap();
        let cam = |p: Option<PathBuf>| Camera { dev: "/dev/video0".into(), power: p };
        assert!(!maybe_busy(&[cam(Some(dir.clone()))]));
        std::fs::write(dir.join("runtime_status"), "active\n").unwrap();
        assert!(maybe_busy(&[cam(Some(dir.clone()))]));
        std::fs::write(dir.join("runtime_status"), "suspended\n").unwrap();
        std::fs::write(dir.join("control"), "on\n").unwrap(); // (power management off: the state says nothing)
        assert!(maybe_busy(&[cam(Some(dir.clone()))]));
        assert!(maybe_busy(&[cam(None)]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
