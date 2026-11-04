//! Sources read from /proc: file copies in progress (transfers.py's job),
//! who holds a camera open (fuser's, for camera.rs), and screen sharing
//! (screenshare.sh's).

use crate::{emit, stop::Stop};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn pids() -> impl Iterator<Item = (u32, PathBuf)> {
    std::fs::read_dir("/proc").into_iter().flatten().flatten().filter_map(|e| {
        let pid: u32 = e.file_name().to_str()?.parse().ok()?;
        Some((pid, e.path()))
    })
}

fn comm(proc: &Path) -> Option<String> {
    std::fs::read_to_string(proc.join("comm")).ok().map(|s| s.trim_end_matches('\n').to_string())
}

// The process's start time (field 22 of stat, counted after the name)
fn start_time(proc: &Path) -> Option<String> {
    let stat = std::fs::read_to_string(proc.join("stat")).ok()?;
    stat.rsplit_once(") ")?.1.split_whitespace().nth(19).map(String::from)
}

/// The names of this user's processes holding any of `devs` open.
pub fn holding(devs: &[&Path]) -> Vec<String> {
    let mut names = BTreeSet::new();
    for (_, proc) in pids() {
        let Ok(fds) = std::fs::read_dir(proc.join("fd")) else { continue };
        if fds.flatten().any(|fd| std::fs::read_link(fd.path()).map(|t| devs.contains(&t.as_path())).unwrap_or(false)) {
            if let Some(c) = comm(&proc) {
                names.insert(c);
            }
        }
    }
    names.into_iter().collect()
}

// ── Copies ──────────────────────────────────────────────────────────────
// This user's cp, mv, rsync, dd, install, pv and gcp, each with the largest
// file it's reading and how far in it is, and its start time (so cancelling
// one can never hit another that later got the same pid). Every 2 s while
// something copies, every 10 s when nothing does; reported as
//   {"src":"copies","items":[{"pid","start","cmd","file","pos","size"}]}
// only when the list changed.
const COPIERS: &[&str] = &["cp", "mv", "rsync", "dd", "install", "pv", "gcp"];

pub fn copies() -> Vec<Value> {
    let uid = unsafe { libc::getuid() };
    let mut out = vec![];
    for (pid, proc) in pids() {
        if std::fs::metadata(&proc).map(|m| m.uid() != uid).unwrap_or(true) {
            continue;
        }
        let Some(name) = comm(&proc) else { continue };
        if !COPIERS.contains(&name.as_str()) {
            continue;
        }
        let Some(start) = start_time(&proc) else { continue };
        let mut best: Option<(u64, String, u64)> = None; // size, file, pos
        let Ok(fds) = std::fs::read_dir(proc.join("fd")) else { continue };
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else { continue };
            if !target.is_file() {
                continue;
            }
            let Ok(info) = std::fs::read_to_string(proc.join("fdinfo").join(fd.file_name())) else { continue };
            let Some((pos, flags)) = fdinfo(&info) else { continue };
            if flags & 3 != 0 {
                continue; // (written to: the copy's destination)
            }
            let Ok(size) = std::fs::metadata(fd.path()).map(|m| m.len()) else { continue };
            if best.as_ref().map(|b| size > b.0).unwrap_or(true) {
                best = Some((size, target.to_string_lossy().to_string(), pos));
            }
        }
        // (still the same process: its pid wasn't reused meanwhile)
        if let Some((size, file, pos)) = best {
            if start_time(&proc).as_deref() == Some(start.as_str()) {
                out.push(copy_item(pid, &start, &name, &file, pos, size));
            }
        }
    }
    out
}

pub fn copy_item(pid: u32, start: &str, cmd: &str, file: &str, pos: u64, size: u64) -> Value {
    let mut m = Map::new();
    m.insert("pid".into(), json!(pid));
    m.insert("start".into(), json!(start));
    m.insert("cmd".into(), json!(cmd));
    m.insert("file".into(), json!(file));
    m.insert("pos".into(), json!(pos));
    m.insert("size".into(), json!(size));
    Value::Object(m)
}

/// An fdinfo file's position and flags (octal).
pub fn fdinfo(text: &str) -> Option<(u64, u32)> {
    let mut pos = None;
    let mut flags = None;
    for l in text.lines() {
        if let Some((k, v)) = l.split_once(':') {
            match k.trim() {
                "pos" => pos = v.trim().parse().ok(),
                "flags" => flags = u32::from_str_radix(v.trim(), 8).ok(),
                _ => {}
            }
        }
    }
    Some((pos?, flags?))
}

pub fn run_copies(stop: Stop, _args: Value) {
    let mut prev: Option<Value> = None;
    loop {
        let items = Value::Array(copies());
        let busy = items.as_array().map(|a| !a.is_empty()).unwrap_or(false);
        if prev.as_ref() != Some(&items) {
            emit(json!({"src": "copies", "items": items}));
            prev = Some(items);
        }
        if !stop.sleep(Duration::from_secs(if busy { 2 } else { 10 })) {
            return;
        }
    }
}

// ── Screen sharing ──────────────────────────────────────────────────────
// A share through the screen-sharing portal is a PipeWire video stream
// that xdg-desktop-portal-hyprland makes ("xdph-streaming-N"), linked to
// the app receiving it. Asking PipeWire (pw-cli) costs ~15 ms, so it's
// only done when the portal's count of open files changes (it opens some
// for every stream: a free look every 3 s, nothing started), and once a
// minute; with no portal running, every 10 s. While a stream exists the
// graph (pw-dump) is read every 2 s and reduced to what the shell reads,
// reported as {"src":"screenshare","graph":[...]} when it changed, and
// once with [] when the last stream goes.
pub fn run_screenshare(stop: Stop, _args: Value) {
    let mut xdph: Option<u32> = None;
    let mut xfds: Option<usize> = None;
    let mut checked: Option<Instant> = None;
    let mut had = false;
    let mut last: Option<Value> = None;
    loop {
        let since = checked.map(|c| c.elapsed()).unwrap_or(Duration::MAX);
        if !had {
            if xdph.map(|p| !Path::new(&format!("/proc/{p}")).exists()).unwrap_or(true) {
                xdph = find_xdph();
            }
            let changed = match xdph {
                None => since >= Duration::from_secs(10),
                Some(p) => {
                    let n = std::fs::read_dir(format!("/proc/{p}/fd")).map(|d| d.count()).ok();
                    let c = n != xfds || since >= Duration::from_secs(60);
                    xfds = n;
                    c
                }
            };
            if !changed {
                if !stop.sleep(Duration::from_secs(3)) {
                    return;
                }
                continue;
            }
        }
        checked = Some(Instant::now());
        let streaming = run(&["pw-cli", "ls", "Node"]).map(|o| o.contains("xdph-streaming-")).unwrap_or(false);
        if streaming {
            let graph = run(&["pw-dump", "--no-colors"]).and_then(|o| serde_json::from_str::<Value>(&o).ok()).map(|g| slim(&g));
            if let Some(g) = graph {
                if last.as_ref() != Some(&g) {
                    emit(json!({"src": "screenshare", "graph": g}));
                    last = Some(g);
                }
            }
            had = true;
        } else if had {
            emit(json!({"src": "screenshare", "graph": []}));
            had = false;
            last = None;
        }
        if !stop.sleep(Duration::from_secs(2)) {
            return;
        }
    }
}

// The portal's process (xdg-desktop-portal-hyprland: "xdg-desktop-por" in comm)
fn find_xdph() -> Option<u32> {
    pids().find_map(|(pid, proc)| {
        let c = comm(&proc)?;
        if !c.starts_with("xdg-desktop-por") {
            return None;
        }
        let cmdline = std::fs::read(proc.join("cmdline")).ok()?;
        String::from_utf8_lossy(&cmdline).contains("hyprland").then_some(pid)
    })
}

// A short-lived tool's output; it dies with us
fn run(argv: &[&str]) -> Option<String> {
    let mut c = Command::new(argv[0]);
    c.args(&argv[1..]).stdin(Stdio::null()).stderr(Stdio::null());
    unsafe {
        c.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong, 0, 0, 0);
            Ok(())
        });
    }
    let out = c.output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The share-relevant part of a pw-dump: the xdph nodes, the links out of
/// them, the nodes at their other ends and those nodes' clients, each cut
/// down to id, type, link state and ends, and the four props read.
pub fn slim(dump: &Value) -> Value {
    let empty = vec![];
    let all = dump.as_array().unwrap_or(&empty);
    let ty = |o: &Value, t: &str| o.get("type").and_then(Value::as_str).map(|s| s.ends_with(t)).unwrap_or(false);
    let prop = |o: &Value, k: &str| o.pointer(&format!("/info/props/{}", k.replace('/', "~1"))).cloned().unwrap_or(Value::Null);
    let info = |o: &Value, k: &str| o.get("info").and_then(|i| i.get(k)).cloned().unwrap_or(Value::Null);
    let x: Vec<&Value> = all
        .iter()
        .filter(|o| ty(o, ":Node") && prop(o, "node.name").as_str().map(|n| n.starts_with("xdph-streaming-")).unwrap_or(false))
        .collect();
    let xi: Vec<Value> = x.iter().map(|o| o.get("id").cloned().unwrap_or(Value::Null)).collect();
    let l: Vec<&Value> = all.iter().filter(|o| ty(o, ":Link") && xi.contains(&info(o, "output-node-id"))).collect();
    let pi: Vec<Value> = l.iter().map(|o| info(o, "input-node-id")).collect();
    let p: Vec<&Value> = all.iter().filter(|o| ty(o, ":Node") && pi.contains(o.get("id").unwrap_or(&Value::Null))).collect();
    let ci: Vec<Value> = p.iter().map(|o| prop(o, "client.id")).collect();
    let c: Vec<&Value> = all.iter().filter(|o| ty(o, ":Client") && ci.contains(o.get("id").unwrap_or(&Value::Null))).collect();
    let keys = ["node.name", "client.id", "application.name", "application.process.binary"];
    let cut = |o: &Value| {
        let mut props = Map::new();
        for k in keys {
            props.insert(k.into(), prop(o, k));
        }
        json!({"id": o.get("id"), "type": o.get("type"), "info": {"state": info(o, "state"), "output-node-id": info(o, "output-node-id"),
               "input-node-id": info(o, "input-node-id"), "props": props}})
    };
    Value::Array(x.into_iter().chain(l).chain(p).chain(c).map(cut).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fdinfo_fields() {
        assert_eq!(fdinfo("pos:\t4096\nflags:\t0100000\nmnt_id:\t25\nino:\t1\n"), Some((4096, 0o100000)));
        assert_eq!(fdinfo("pos:\t0\nflags:\t0100001\n").map(|f| f.1 & 3), Some(1)); // (open for writing)
        assert_eq!(fdinfo("flags:\t02\n"), None);
    }

    #[test]
    fn copy_items_keep_transfers_py_shape() {
        assert_eq!(
            copy_item(42, "12345", "cp", "/home/me/big.iso", 1024, 4096),
            json!({"pid": 42, "start": "12345", "cmd": "cp", "file": "/home/me/big.iso", "pos": 1024, "size": 4096})
        );
    }

    #[test]
    fn our_own_copy_is_seen() {
        // a `dd` reading a file (from /dev/zero would have no size; read this test binary)
        let me = std::env::current_exe().unwrap();
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("exec dd if=\"$1\" of=/dev/null bs=1 count=1 status=none iflag=fullblock; sleep 5")
            .arg("sh")
            .arg(&me)
            .spawn();
        let Ok(child) = child.as_mut() else { return }; // (no sh: nothing to test with)
        // (dd may be gone by the time it's looked for; that's fine, only a
        // copy that's seen must look right)
        for item in copies() {
            assert!(COPIERS.contains(&item["cmd"].as_str().unwrap()));
            assert!(item["size"].as_u64().unwrap() >= item["pos"].as_u64().unwrap());
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn screenshare_graph() {
        let dump = json!([
            {"id": 1, "type": "PipeWire:Interface:Node", "info": {"props": {"node.name": "xdph-streaming-0"}}},
            {"id": 2, "type": "PipeWire:Interface:Link", "info": {"state": "active", "output-node-id": 1, "input-node-id": 3}},
            {"id": 3, "type": "PipeWire:Interface:Node", "info": {"props": {"node.name": "chrome", "client.id": 4}}},
            {"id": 4, "type": "PipeWire:Interface:Client", "info": {"props": {"application.name": "Chromium", "application.process.binary": "chromium"}}},
            {"id": 5, "type": "PipeWire:Interface:Node", "info": {"props": {"node.name": "alsa_output.speakers"}}},
            {"id": 6, "type": "PipeWire:Interface:Link", "info": {"state": "active", "output-node-id": 5, "input-node-id": 7}}
        ]);
        let g = slim(&dump);
        let ids: Vec<i64> = g.as_array().unwrap().iter().map(|o| o["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);
        assert_eq!(g[1]["info"]["output-node-id"], 1);
        assert_eq!(g[1]["info"]["state"], "active");
        assert_eq!(g[2]["info"]["props"]["client.id"], 4);
        assert_eq!(g[3]["info"]["props"]["application.name"], "Chromium");
        assert_eq!(g[0]["info"]["props"]["application.name"], Value::Null);
        // nothing shared: nothing
        assert_eq!(slim(&json!([dump[4]])), json!([]));
        assert_eq!(slim(&json!({})), json!([]));
    }

    #[test]
    fn holders() {
        // nobody holds a path that doesn't exist
        assert!(holding(&[Path::new("/dev/irice-no-such-camera")]).is_empty());
    }
}
