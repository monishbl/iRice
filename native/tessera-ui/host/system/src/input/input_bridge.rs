//! Settings drawn in Rust: the "input" page group's side, replacing
//! shell/modules/settings/RustBridgeInput.qml and the running half of
//! shell/services/Keymap.qml (files, saving, helper processes; the data and
//! editing are keymap.rs).
//!
//! While Settings is open (`input.live` [true], told by host-shell's settings
//! bridge) it tells the pages what only the shell knows (`feed`, the pages'
//! cx.feed["input"]) with `{"to":"settings","feed":{"input":…}}`: the mice and
//! trackpads (bus "mouse"), the shortcuts and gestures (keybinds.json), Dictation's
//! and Live Text's state (bus "dictation", "screentools"), the tools there are,
//! the updates job (bus "updates") and the import and export's state. It does
//! what they ask (`cx.ask("input", what, args)`): Keymap's changes (with one
//! undo), installs and downloads (forwarded to the services that own them:
//! "dictation.*", "screentools.installOcr"), the import and export (file
//! dialogs, previews).
//!
//! Calls (prefix "input"): "input.act" [what, …args] (or "input.<what>" [args…]),
//! "input.live" [bool]. Prefix "keymap": "keymap.setKeysTakingOver" [id, keys],
//! "keymap.snapshot" is not needed (Settings uses its own undo through act).
//! Bus "keymap": { ready, binds, gestures, modKey } for the cheatsheet and others.

use super::common::*;
use super::keymap::*;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

fn arg_str(args: &[Value], i: usize) -> String {
    args.get(i).map(|v| v.as_str().map(String::from).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() })).unwrap_or_default()
}

fn has_tool(cmd: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
}

fn expand(home: &str, path: &str) -> String {
    let p = path.trim();
    if p == "~" {
        home.to_string()
    } else if let Some(r) = p.strip_prefix("~/") {
        format!("{home}/{r}")
    } else {
        p.to_string()
    }
}

/// One import/export page's state (RustBridgeInput.qml's IeState)
#[derive(Default)]
struct Ie {
    gestures_only: bool,
    path_mode: String,
    path_sections: Vec<String>,
    path_text: String,
    path_seq: i64,
    preview: Option<Value>,
    preview_seq: i64,
    error: String,
    status: String,
    busy: bool,
    undo_snap: String,
    undo_gen: u64,
}

impl Ie {
    fn state(&self, dialog: bool) -> Value {
        let preview = self.preview.as_ref().map(|p| {
            json!({
                "title": p["title"].as_str().unwrap_or(""), "note": p["note"].as_str().unwrap_or(""), "summary": p["summary"].as_str().unwrap_or(""),
                "total": p["total"].as_i64().unwrap_or(0),
                "lines": p["lines"].as_array().into_iter().flatten().map(|l| json!({"kind": l["kind"].as_str().unwrap_or(""), "text": l["text"].as_str().unwrap_or("")})).collect::<Vec<_>>(),
                "skipped": p["skipped"].as_array().into_iter().flatten().map(|l| json!({"text": l["text"].as_str().unwrap_or("")})).collect::<Vec<_>>()
            })
        });
        json!({
            "busy": self.busy, "pathMode": self.path_mode, "pathText": self.path_text, "pathSeq": self.path_seq,
            "preview": preview, "previewSeq": self.preview_seq, "error": self.error, "status": self.status,
            "canUndo": !self.undo_snap.is_empty(), "dialog": dialog
        })
    }
    fn clear(&mut self) {
        self.error.clear();
        self.status.clear();
        self.preview = None;
        self.path_mode.clear();
    }
    fn default_path(secs: &[String]) -> String {
        format!("~/Documents/{}", if secs.len() == 1 && secs[0] == "gestures" { "tessera-gestures.json" } else { "tessera-shortcuts.json" })
    }
    fn ask_path(&mut self, save: bool, secs: Vec<String>) {
        self.path_text = Self::default_path(&secs);
        self.path_sections = secs;
        self.path_mode = if save { "export" } else { "import" }.into();
        self.path_seq += 1;
    }
}

enum M {
    /// 0 defaults, 1 gestures, 2 user: text (None: not found / unreadable), mtime
    Loaded(u8, Option<String>, Option<std::time::SystemTime>, String),
    Probed(String),
    Tools { zenity: bool, kdialog: bool, gtk_launch: bool },
    Poll,
    SaveNow(u64),
    SaveDone(bool, String),
    RefreshBinds,
    Feed,
    UndoExpire(bool, u64),
    Dialog { gestures: bool, save: bool, secs: Vec<String>, code: i32, out: String },
    Exported { gestures: bool, path: String, ok: bool, err: String },
    Read { gestures: bool, path: String, secs: Vec<String>, ok: bool, text: String, err: String },
    End4 { gestures: bool, code: i32, out: String },
    /// the backup page's end4 import (keymap.previewEnd4): the helper's answer, and where to send the preview
    KmEnd4 { reply: Value, code: i32, out: String },
}

#[derive(Default)]
pub struct Input {
    km: Keymap,
    live: bool,
    home: String,
    hypr_dir: String,
    user_path: std::path::PathBuf,
    tools: (bool, bool, bool),
    snap: String,
    save_gen: u64,
    saving: bool,
    save_waiting: bool,
    ie_keys: Ie,
    ie_gestures: Ie,
    mtimes: [Option<std::time::SystemTime>; 3],
    last_feed: String,
    binds_rev: Option<(u64, usize, String, String)>,
    binds_cache: (Vec<Value>, Vec<Value>),
    apps_len: usize,
    poll_started: bool,
}

impl Input {
    fn ie(&mut self, gestures: bool) -> &mut Ie {
        if gestures {
            &mut self.ie_gestures
        } else {
            &mut self.ie_keys
        }
    }

    fn defaults_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.hypr_dir).join("keymap.json")
    }

    /// Reads one of the three files on a thread
    fn load(&self, h: &Host, which: u8) {
        let path = match which {
            0 => self.defaults_path(),
            1 => std::path::PathBuf::from(&self.hypr_dir).join("gestures.json"),
            _ => self.user_path.clone(),
        };
        let send = h.sender("Input");
        std::thread::spawn(move || {
            let mt = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            match std::fs::read_to_string(&path) {
                Ok(t) => send(Box::new(M::Loaded(which, Some(t), mt, path.to_string_lossy().into_owned()))),
                Err(_) => send(Box::new(M::Loaded(which, None, mt, path.to_string_lossy().into_owned()))),
            }
        });
    }

    fn sync_apps(&mut self, h: &Host) {
        if let Some(a) = h.st.shell.get::<tessera_core::services::apps::Apps>() {
            if a.entries.len() != self.apps_len || self.km.apps.is_empty() {
                self.apps_len = a.entries.len();
                self.km.apps = a.entries.iter().map(|e| (e.id.clone(), e.name.clone())).collect();
            }
        }
        let cfg = h.cfg();
        let s = cfg["appearance"]["keySymbols"].as_str().unwrap_or("auto");
        self.km.style = if s != "auto" && !s.is_empty() {
            s.to_string()
        } else if cfg["appearance"]["menuIcon"] == "command" {
            "mac".into()
        } else {
            "linux".into()
        };
    }

    /// Debounced: several quick edits make one write and one reload
    fn save(&mut self, h: &Host) {
        self.km.dirty = false;
        self.save_gen += 1;
        after(h.sender("Input"), 400, M::SaveNow(self.save_gen));
        self.publish();
    }

    /// An answer to a call: `reply` is {call, key, …}; the call gets {key, …fields}
    fn reply(h: &mut Host, reply: &Value, fields: Value) {
        let Some(call) = reply["call"].as_str() else { return };
        let mut v = json!({"key": reply["key"]});
        if !reply["path"].is_null() {
            v["path"] = reply["path"].clone();
        }
        if let Value::Object(f) = fields {
            for (k, x) in f {
                v[k] = x;
            }
        }
        h.ask(call, v);
    }

    fn after_change(&mut self, h: &Host) {
        if self.km.dirty {
            self.save(h);
        }
    }

    fn write_now(&mut self, h: &Host) {
        let text = format!("{}\n", pretty1(&self.km.user));
        self.saving = true;
        let send = h.sender("Input");
        let dir = self.user_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let path = self.user_path.clone();
        let shown = path.to_string_lossy().into_owned();
        std::thread::spawn(move || {
            // Temp file, then rename: Hyprland never reads half a file
            let r = std::fs::create_dir_all(&dir).and_then(|_| {
                let t = path.with_extension(format!("json.tmp.{}", std::process::id()));
                std::fs::write(&t, &text)?;
                std::fs::rename(&t, &path)
            });
            match r {
                Ok(()) => {
                    let _ = std::process::Command::new("hyprctl").args(["reload", "config-only"]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
                    send(Box::new(M::SaveDone(true, String::new())));
                }
                Err(e) => send(Box::new(M::SaveDone(false, format!("Couldn't save your shortcuts to {shown} ({}). Check that the folder is writable, then change a shortcut again.", e.to_string().split(" (os").next().unwrap_or(""))))),
            }
        });
    }

    fn publish(&mut self) {
        let binds = self.km.binds();
        crate::bus::put("keymap", json!({"ready": self.km.ready(), "modKey": self.km.mod_key, "binds": binds, "gestures": self.km.gestures(), "threeFingerDrag": self.km.three_finger_drag()}));
    }

    fn feed(&mut self, h: &Host) -> Value {
        self.sync_apps(h);
        let key = (self.km.revision, self.km.apps.len(), self.km.style.clone(), self.km.mod_key.clone());
        if self.binds_rev.as_ref() != Some(&key) {
            let binds: Vec<Value> = self
                .km
                .binds()
                .into_iter()
                .map(|b| {
                    json!({"id": b["id"], "keys": b["keys"], "defaultKeys": b["defaultKeys"], "group": b["group"], "label": b["label"], "action": b["action"], "opts": b["opts"], "custom": b["custom"], "disabled": b["disabled"], "changed": b["changed"], "keysChanged": b["keysChanged"], "keyList": b["keyList"], "actionText": b["actionText"]})
                })
                .collect();
            let gestures: Vec<Value> = self
                .km
                .gestures()
                .into_iter()
                .map(|g| json!({"id": g["id"], "fingers": g["fingers"], "direction": g["direction"], "action": g["action"], "label": g["label"], "custom": g["custom"], "disabled": g["disabled"], "changed": g["changed"], "text": g["text"], "actionText": g["actionText"]}))
                .collect();
            self.binds_cache = (binds, gestures);
            self.binds_rev = Some(key);
        }
        let mouse = crate::bus::get("mouse");
        let dict = crate::bus::get("dictation");
        let st = crate::bus::get("screentools");
        let dialog = self.tools.0 || self.tools.1;
        let gdefs: Vec<Value> = self.km.gesture_defaults.iter().map(|d| json!({"id": d["id"], "fingers": d["fingers"], "direction": d["direction"], "action": d["action"], "label": d["label"].as_str().unwrap_or(d["id"].as_str().unwrap_or(""))})).collect();
        json!({
            "mice": mouse["mice"], "touchpads": mouse["touchpads"], "boundaryAvailable": mouse["boundaryAvailable"].as_bool().unwrap_or(false),
            "keymap": {
                "ready": self.km.ready(), "defaultsError": self.km.defaults_error, "userError": self.km.user_error, "saveError": self.km.save_error,
                "modKey": self.km.mod_key, "hyprDir": self.hypr_dir, "threeFingerDrag": self.km.three_finger_drag(),
                "binds": self.binds_cache.0, "gestures": self.binds_cache.1, "gestureDefaults": gdefs
            },
            "dictation": {
                "hasBinary": dict["hasBinary"].as_bool().unwrap_or(false), "ready": dict["ready"].as_bool().unwrap_or(false), "installing": dict["installing"].as_bool().unwrap_or(false),
                "installError": dict["installError"].as_str().unwrap_or(""), "models": if dict["models"].is_null() { json!({}) } else { dict["models"].clone() },
                "downloading": dict["downloading"].as_str().unwrap_or(""), "downloadProgress": dict["downloadProgress"].as_f64().unwrap_or(0.0),
                "downloadError": dict["downloadError"].as_str().unwrap_or(""), "lastDownload": dict["lastDownload"].as_str().unwrap_or("")
            },
            "tools": {"ready": true, "swappy": has_tool("swappy"), "wtype": has_tool("wtype")},
            "ocr": {"hasTesseract": st["hasTesseract"].as_bool().unwrap_or(false), "installing": st["ocrInstalling"].as_bool().unwrap_or(false), "error": st["ocrInstallError"].as_str().unwrap_or(""), "languages": if st["ocrLanguages"].is_null() { json!([]) } else { st["ocrLanguages"].clone() }},
            "updatesRunning": crate::bus::get("updates")["running"].as_bool().unwrap_or(false),
            "ie": {"keys": self.ie_keys.state(dialog), "gestures": self.ie_gestures.state(dialog)}
        })
    }

    fn push_feed(&mut self, h: &mut Host, force: bool) {
        if !self.live {
            return;
        }
        let f = self.feed(h);
        let s = f.to_string();
        if force || s != self.last_feed {
            self.last_feed = s;
            h.tell("settings", json!({"feed": {"input": f}}));
        }
    }

    fn snapshot(&mut self) {
        self.snap = self.km.snapshot();
    }

    // ── What the pages ask ──

    fn act(&mut self, h: &mut Host, what: &str, args: &[Value]) {
        self.sync_apps(h);
        match what {
            "threeFingerDrag" => {
                self.km.set_three_finger_drag(args.first().and_then(Value::as_bool).unwrap_or(false));
            }
            // ── Shortcuts ──
            "kbDisable" => {
                self.snapshot();
                self.km.disable(&arg_str(args, 0), args.get(1).is_some_and(|v| v.as_bool().unwrap_or(false)));
            }
            "kbReset" => {
                self.snapshot();
                self.km.reset(&arg_str(args, 0));
            }
            "kbRemove" => {
                self.snapshot();
                self.km.remove_shortcut(&arg_str(args, 0));
            }
            "kbSave" => {
                // { editId, keys, takeOver, custom, entry }
                let o = args.first().cloned().unwrap_or(json!({}));
                self.snapshot();
                for id in o["takeOver"].as_array().into_iter().flatten() {
                    self.km.disable(id.as_str().unwrap_or(""), true);
                }
                let edit_id = o["editId"].as_str().unwrap_or("").to_string();
                let keys = o["keys"].as_str().unwrap_or("").to_string();
                if o["custom"].as_bool() != Some(true) && o["custom"].as_i64().unwrap_or(0) == 0 {
                    self.km.set_keys(&edit_id, &keys);
                } else {
                    let e = &o["entry"];
                    let mut a = e["action"].clone();
                    // (an app: its launch, from its desktop entry)
                    if a.is_object() && !a["desktop"].is_null() && a["exec"].is_null() {
                        let id = a["desktop"].as_str().unwrap_or("");
                        let app = h.st.shell.get::<tessera_core::services::apps::Apps>().and_then(|x| x.by_id(id)).cloned();
                        a = match app {
                            Some(app) => Keymap::app_action(&app.id, &tessera_core::services::apps::exec_argv(&app.exec), self.tools.2),
                            None => self.km.bind(&edit_id).map(|b| b["action"].clone()).unwrap_or(Value::Null),
                        };
                    }
                    let label = e["label"].as_str().unwrap_or("").trim().to_string();
                    let entry = json!({"keys": keys, "label": if label.is_empty() { self.km.describe(&a) } else { label }, "action": a, "opts": if e["opts"].is_null() { json!({}) } else { e["opts"].clone() }});
                    if edit_id.is_empty() {
                        self.km.add_shortcut(&entry);
                    } else {
                        self.km.update_shortcut(&edit_id, &entry);
                    }
                }
            }
            // ── Gestures ──
            "gSet" => self.km.set_gesture(&arg_str(args, 0), &args.get(1).cloned().filter(|v| v.is_object()).unwrap_or(json!({}))),
            "gDisable" => {
                self.snapshot();
                self.km.disable_gesture(&arg_str(args, 0), args.get(1).and_then(Value::as_bool).unwrap_or(false));
            }
            "gReset" => {
                self.snapshot();
                self.km.reset_gesture(&arg_str(args, 0));
            }
            "gRemove" => {
                self.snapshot();
                self.km.remove_gesture(&arg_str(args, 0));
            }
            "gAdd" => {
                self.snapshot();
                self.km.add_gesture(&args.first().cloned().unwrap_or(json!({})));
            }
            "undo" => {
                if !self.snap.is_empty() {
                    let s = std::mem::take(&mut self.snap);
                    self.km.restore(&s);
                }
            }
            // ── Import and export: [scope, op, …] ──
            "ie" => {
                let gestures = args.first().and_then(Value::as_str) == Some("gestures");
                self.ie_run(h, gestures, &arg_str(args, 1), &args[args.len().min(2)..]);
            }
            // ── Dictation and Live Text (their services answer) ──
            "dictRefresh" => h.ask("dictation.refresh", json!([])),
            "dictInstall" => h.ask("dictation.install", json!([])),
            "dictDownload" => h.ask("dictation.download", json!([arg_str(args, 0)])),
            "dictCancel" => h.ask("dictation.cancelDownload", json!([])),
            "dictRemove" => h.ask("dictation.removeModel", json!([arg_str(args, 0)])),
            "installOcr" => h.ask("screentools.installOcr", json!([arg_str(args, 0)])),
            other => eprintln!("settings (input): no act {other}"),
        }
        self.after_change(h);
        self.push_feed(h, false);
    }

    fn ie_run(&mut self, h: &mut Host, gestures: bool, op: &str, a: &[Value]) {
        let secs_of = |v: Option<&Value>| -> Vec<String> { v.and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() };
        match op {
            "import" | "export" => {
                let secs = secs_of(a.first());
                self.ie(gestures).clear();
                self.pick(h, gestures, op == "export", secs);
            }
            "end4" => {
                self.ie(gestures).clear();
                self.ie(gestures).busy = true;
                let secs: &[&str] = if gestures { &["gestures"] } else { &["keybinds", "shortcuts", "gestures"] };
                let with_binds = secs.contains(&"keybinds") || secs.contains(&"shortcuts");
                let with_g = secs.contains(&"gestures");
                let hh = format!("{}/.config/hypr", self.home);
                let mut cmd: Vec<String> = vec!["python3".into(), "-I".into(), crate::util::script("import-end4.py")];
                if with_binds {
                    cmd.push(format!("{hh}/hyprland/keybinds.lua"));
                    cmd.push(format!("{hh}/custom/keybinds.lua"));
                }
                cmd.push("--".into());
                if with_g {
                    cmd.push(format!("{hh}/hyprland/general.lua"));
                    cmd.push(format!("{hh}/custom/general.lua"));
                }
                let send = h.sender("Input");
                std::thread::spawn(move || {
                    let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
                    let (code, out) = crate::util::capture(&c, Duration::from_secs(60)).unwrap_or((127, String::new()));
                    send(Box::new(M::End4 { gestures, code, out }));
                });
            }
            "go" => {
                let path = arg_str(a, 0).trim().to_string();
                if path.is_empty() || self.ie(gestures).busy {
                    return;
                }
                let (mode, secs) = (self.ie(gestures).path_mode.clone(), self.ie(gestures).path_sections.clone());
                if mode == "export" {
                    self.do_export(h, gestures, path, secs);
                } else {
                    self.do_import(h, gestures, path, secs);
                }
            }
            "cancelPath" => self.ie(gestures).path_mode.clear(),
            "apply" => {
                let s = self.km.snapshot();
                if let Some(p) = self.ie(gestures).preview.take() {
                    self.km.apply_preview(&p);
                }
                let ie = self.ie(gestures);
                ie.undo_snap = s;
                ie.status = "Imported.".into();
                ie.undo_gen += 1;
                let g = ie.undo_gen;
                after(h.sender("Input"), 8000, M::UndoExpire(gestures, g));
            }
            "cancelPreview" => self.ie(gestures).preview = None,
            "undo" => {
                let snap = self.ie(gestures).undo_snap.clone();
                if !snap.is_empty() {
                    self.km.restore(&snap);
                    let ie = self.ie(gestures);
                    ie.undo_snap.clear();
                    ie.status = "Import undone.".into();
                    ie.undo_gen += 1;
                    let g = ie.undo_gen;
                    after(h.sender("Input"), 8000, M::UndoExpire(gestures, g));
                }
            }
            "dismiss" => self.ie(gestures).error.clear(),
            _ => {}
        }
    }

    /// A file dialog; the path field without one
    fn pick(&mut self, h: &Host, gestures: bool, save: bool, secs: Vec<String>) {
        let start = expand(&self.home, &Ie::default_path(&secs));
        let cmd: Option<Vec<String>> = if self.tools.0 {
            Some(if save {
                vec!["zenity".into(), "--file-selection".into(), "--save".into(), "--confirm-overwrite".into(), "--title=Export shortcuts".into(), format!("--filename={start}"), "--file-filter=JSON files | *.json".into()]
            } else {
                vec!["zenity".into(), "--file-selection".into(), "--title=Import shortcuts".into(), format!("--filename={}/Documents/", self.home), "--file-filter=JSON files | *.json".into()]
            })
        } else if self.tools.1 {
            Some(if save {
                vec!["kdialog".into(), "--title".into(), "Export shortcuts".into(), "--getsavefilename".into(), start, "*.json".into()]
            } else {
                vec!["kdialog".into(), "--title".into(), "Import shortcuts".into(), "--getopenfilename".into(), format!("{}/Documents", self.home), "*.json".into()]
            })
        } else {
            None
        };
        let Some(cmd) = cmd else {
            self.ie(gestures).ask_path(save, secs);
            return;
        };
        self.ie(gestures).busy = true;
        let send = h.sender("Input");
        std::thread::spawn(move || {
            let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
            let (code, out) = crate::util::capture(&c, Duration::from_secs(3600)).unwrap_or((127, String::new()));
            send(Box::new(M::Dialog { gestures, save, secs, code, out }));
        });
    }

    fn do_export(&mut self, h: &Host, gestures: bool, path: String, secs: Vec<String>) {
        self.ie(gestures).busy = true;
        let text = format!("{}\n", pretty1(&self.km.export_data(&secs)));
        let p = expand(&self.home, &path);
        let send = h.sender("Input");
        std::thread::spawn(move || {
            let pp = std::path::PathBuf::from(&p);
            let r = crate::util::write_atomic(&pp, &text);
            let err = r.as_ref().err().map(|e| e.to_string().split(" (os").next().unwrap_or("").to_string()).unwrap_or_default();
            send(Box::new(M::Exported { gestures, path, ok: r.is_ok(), err }));
        });
    }

    fn do_import(&mut self, h: &Host, gestures: bool, path: String, secs: Vec<String>) {
        self.ie(gestures).busy = true;
        let p = expand(&self.home, &path);
        let send = h.sender("Input");
        std::thread::spawn(move || {
            let r = std::fs::read_to_string(&p);
            let err = r.as_ref().err().map(|e| format!("{p}: {}", e.to_string().split(" (os").next().unwrap_or(""))).unwrap_or_default();
            send(Box::new(M::Read { gestures, path, secs, ok: r.is_ok(), text: r.unwrap_or_default(), err }));
        });
    }
}

/// JSON.stringify(x, null, 1): one space of indent
fn pretty1(v: &Value) -> String {
    use serde::Serialize;
    let mut buf = Vec::new();
    let f = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, f);
    if v.serialize(&mut ser).is_err() {
        return v.to_string();
    }
    String::from_utf8(buf).unwrap_or_default()
}

impl Service for Input {
    fn name(&self) -> &'static str {
        "Input"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["input", "keymap"]
    }
    fn start(&mut self, h: &mut Host) {
        self.home = crate::util::home().to_string_lossy().into_owned();
        // $MOD is Super, except in the nested sandbox (Alt)
        self.km.mod_key = if tessera_core::env::sandboxed() && std::env::var("TESSERA_TTY").or_else(|_| std::env::var("IRICE_TTY")).as_deref() != Ok("1") { "ALT".into() } else { "SUPER".into() };
        self.user_path = tessera_core::theme::config_path().with_file_name("keybinds.json");
        // The hypr data: $TESSERA_ROOT/hypr/tessera; else the repo next to the
        // shell when keymap.json is there; else the installed copy
        let env_root = std::env::var("TESSERA_ROOT").or_else(|_| std::env::var("IRICE_ROOT")).ok().filter(|r| !r.is_empty());
        let cfg_home = std::env::var("XDG_CONFIG_HOME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{}/.config", self.home));
        self.hypr_dir = match env_root {
            Some(r) => format!("{r}/hypr/tessera"),
            None => {
                let cands = [crate::util::root().join("hypr/tessera").to_string_lossy().into_owned(), format!("{cfg_home}/hypr/tessera"), format!("{cfg_home}/hypr/irice")];
                cands.iter().find(|d| std::path::Path::new(d).join("keymap.json").is_file()).cloned().unwrap_or_else(|| cands[1].clone())
            }
        };
        let send = h.sender("Input");
        std::thread::spawn(move || {
            let has = |c: &str| has_tool(c);
            send(Box::new(M::Tools { zenity: has("zenity"), kdialog: has("kdialog"), gtk_launch: has("gtk-launch") }));
        });
        for w in 0..3 {
            self.load(h, w);
        }
        self.poll_started = true;
        after(h.sender("Input"), 2000, M::Poll);
        let _ = M::Probed;
        let _ = M::RefreshBinds;
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let list: Vec<Value> = args.as_array().cloned().unwrap_or_else(|| if args.is_null() { vec![] } else { vec![args.clone()] });
        let (prefix, rest) = call.split_once('.').unwrap_or((call, ""));
        if prefix == "keymap" {
            match rest {
                "setKeysTakingOver" => {
                    self.km.set_keys_taking_over(&arg_str(&list, 0), &arg_str(&list, 1));
                    self.after_change(h);
                }
                "setKeys" => {
                    self.km.set_keys(&arg_str(&list, 0), &arg_str(&list, 1));
                    self.after_change(h);
                }
                // Settings -> About's backup (host-system's About): {path, sections, reply} …
                "exportTo" => {
                    let a = &list.first().cloned().unwrap_or(Value::Null);
                    let secs: Vec<String> = a["sections"].as_array().map(|l| l.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
                    let path = expand(&self.home, a["path"].as_str().unwrap_or(""));
                    let text = format!("{}\n", serde_json::to_string_pretty(&self.km.export_data(&secs)).unwrap_or_default());
                    let r = std::path::Path::new(&path).parent().map(std::fs::create_dir_all).unwrap_or(Ok(())).and_then(|_| std::fs::write(&path, text));
                    let err = r.as_ref().err().map(|e| e.to_string().split(" (os error").next().unwrap_or("").to_string()).unwrap_or_default();
                    Self::reply(h, &a["reply"], json!({"ok": r.is_ok(), "err": err}));
                }
                "previewFile" => {
                    let a = &list.first().cloned().unwrap_or(Value::Null);
                    let secs: Vec<String> = a["sections"].as_array().map(|l| l.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
                    let p = self.km.preview_file(a["text"].as_str().unwrap_or(""), &secs);
                    Self::reply(h, &a["reply"], json!({"preview": p}));
                }
                "previewEnd4" => {
                    let a = &list.first().cloned().unwrap_or(Value::Null);
                    let hh = format!("{}/.config/hypr", self.home);
                    let mut cmd: Vec<String> = vec!["python3".into(), "-I".into(), crate::util::script("import-end4.py")];
                    if a["shortcuts"] == true {
                        cmd.push(format!("{hh}/hyprland/keybinds.lua"));
                        cmd.push(format!("{hh}/custom/keybinds.lua"));
                    }
                    cmd.push("--".into());
                    if a["gestures"] == true {
                        cmd.push(format!("{hh}/hyprland/general.lua"));
                        cmd.push(format!("{hh}/custom/general.lua"));
                    }
                    let (send, reply) = (h.sender("Input"), a["reply"].clone());
                    std::thread::spawn(move || {
                        let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
                        let (code, out) = crate::util::capture(&c, Duration::from_secs(60)).unwrap_or((127, String::new()));
                        send(Box::new(M::KmEnd4 { reply, code, out }));
                    });
                }
                // Keymap takes the snapshot before it applies, and answers with it (the page's Undo)
                "applyPreview" => {
                    let a = &list.first().cloned().unwrap_or(Value::Null);
                    let snap = self.km.snapshot();
                    self.km.apply_preview(&a["preview"]);
                    self.after_change(h);
                    Self::reply(h, &a["reply"], json!({"snapshot": snap}));
                }
                "restore" => {
                    self.km.restore(list.first().and_then(|a| a["snapshot"].as_str()).unwrap_or(""));
                    self.after_change(h);
                }
                _ => {}
            }
            return;
        }
        match rest {
            "" | "act" => {
                let what = arg_str(&list, 0);
                self.act(h, &what, &list[list.len().min(1)..]);
            }
            "live" => {
                self.live = list.first().and_then(Value::as_bool).unwrap_or(true);
                if self.live {
                    self.sync_apps(h);
                    self.push_feed(h, true);
                    after(h.sender("Input"), 300, M::Feed);
                }
            }
            what => self.act(h, what, &list),
        }
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.sync_apps(h);
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Probed(_) | M::RefreshBinds => {}
            M::Tools { zenity, kdialog, gtk_launch } => {
                self.tools = (zenity, kdialog, gtk_launch);
                self.push_feed(h, false);
            }
            M::Loaded(which, text, mt, path) => {
                self.mtimes[which as usize] = mt;
                match which {
                    0 => {
                        match text.as_deref().map(serde_json::from_str::<Value>) {
                            Some(Ok(Value::Array(d))) => {
                                self.km.defaults = d.into_iter().filter(|x| x["id"].as_str().is_some_and(|s| !s.is_empty()) && x["keys"].as_str().is_some_and(|s| !s.is_empty())).collect();
                                self.km.defaults_error.clear();
                            }
                            Some(Ok(_)) => {
                                self.km.defaults.clear();
                                self.km.defaults_error = format!("Tessera's default shortcuts ({path}) aren't a list. Run tools/gen-keymap.py again, or reinstall Tessera.");
                            }
                            Some(Err(e)) => {
                                self.km.defaults.clear();
                                self.km.defaults_error = format!("Tessera's default shortcuts ({path}) can't be read: {e}. Run tools/gen-keymap.py again, or reinstall Tessera.");
                            }
                            None => {
                                self.km.defaults.clear();
                                self.km.defaults_error = format!("Tessera's default shortcuts weren't found at {path}. Reinstall Tessera (./install.sh), or set TESSERA_ROOT to the repo when running it from there.");
                            }
                        }
                        self.km.defaults_loaded = true;
                        self.km.revision += 1;
                    }
                    1 => {
                        self.km.gesture_defaults = match text.as_deref().map(serde_json::from_str::<Value>) {
                            Some(Ok(Value::Array(d))) => d.into_iter().filter(|x| x["id"].as_str().is_some_and(|s| !s.is_empty())).collect(),
                            _ => vec![],
                        };
                        self.km.revision += 1;
                    }
                    _ => {
                        match text {
                            Some(t) if t.trim().is_empty() => {
                                self.km.user = blank_user();
                                self.km.user_error.clear();
                            }
                            Some(t) => match serde_json::from_str::<Value>(&t) {
                                Ok(v) => {
                                    self.km.user = normalize_user(&v);
                                    self.km.user_error.clear();
                                }
                                // Keep what we had; never overwrite a file the user is fixing
                                Err(e) => {
                                    self.km.user_error = format!("Your shortcuts file ({path}) isn't valid JSON: {e}. Fix it in a text editor, or delete it to go back to the defaults. Changes here won't be saved until then.");
                                }
                            },
                            None => {
                                if mt.is_none() {
                                    self.km.user = blank_user();
                                    self.km.user_error.clear();
                                } else {
                                    self.km.user_error = format!("Your shortcuts file ({path}) can't be read. Check its permissions.");
                                }
                            }
                        }
                        self.km.user_loaded = true;
                        self.km.revision += 1;
                        // A broken file stays as it is until the user fixes it
                        if !self.km.user_error.is_empty() {
                            self.save_gen += 1;
                        }
                    }
                }
                self.publish();
                self.push_feed(h, false);
            }
            M::Poll => {
                // (FileView.watchChanges: a changed file is read again; our own
                // write comes back as a change and is ignored while edits wait)
                for w in 0..3u8 {
                    let path = match w {
                        0 => self.defaults_path(),
                        1 => std::path::PathBuf::from(&self.hypr_dir).join("gestures.json"),
                        _ => self.user_path.clone(),
                    };
                    let mt = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                    if mt != self.mtimes[w as usize] && !(w == 2 && (self.saving || self.km.dirty)) {
                        self.mtimes[w as usize] = mt;
                        self.load(h, w);
                    }
                }
                after(h.sender("Input"), 2000, M::Poll);
            }
            M::SaveNow(g) => {
                if g == self.save_gen && self.km.user_error.is_empty() {
                    self.write_now(h);
                }
            }
            M::SaveDone(ok, err) => {
                self.saving = false;
                self.km.save_error = if ok { String::new() } else { err };
                // the user file just written: remember its time so it isn't re-read
                self.mtimes[2] = std::fs::metadata(&self.user_path).and_then(|m| m.modified()).ok();
                // (the cheatsheet re-reads the live binds, 600 ms on)
                h.ask("keybinds.refresh", json!([]));
                self.push_feed(h, false);
            }
            M::RefreshBinds => {}
            M::Feed => {
                if self.live {
                    self.push_feed(h, false);
                    after(h.sender("Input"), 300, M::Feed);
                }
            }
            M::UndoExpire(g, n) => {
                let ie = self.ie(g);
                if ie.undo_gen == n {
                    ie.undo_snap.clear();
                    ie.status.clear();
                    self.push_feed(h, false);
                }
            }
            M::Dialog { gestures, save, secs, code, out } => {
                self.ie(gestures).busy = false;
                let p = out.trim().split('\n').next().unwrap_or("").to_string();
                if code == 0 && !p.is_empty() {
                    if save {
                        self.do_export(h, gestures, p, secs);
                    } else {
                        self.do_import(h, gestures, p, secs);
                    }
                } else if code != 1 {
                    self.ie(gestures).ask_path(save, secs);
                }
                self.push_feed(h, false);
            }
            M::Exported { gestures, path, ok, err } => {
                let home = self.home.clone();
                let ie = self.ie(gestures);
                ie.busy = false;
                ie.path_mode.clear();
                if ok {
                    ie.status = format!("Exported to {}", path.replace(&home, "~"));
                } else {
                    ie.error = format!("Couldn't write {path}{}. Choose a folder you can write to, such as ~/Documents.", if err.is_empty() { String::new() } else { format!(" ({err})") });
                }
                self.push_feed(h, false);
            }
            M::Read { gestures, path, secs, ok, text, err } => {
                self.ie(gestures).busy = false;
                self.ie(gestures).path_mode.clear();
                if !ok {
                    self.ie(gestures).error = format!("Couldn't read {path}{}. Check the path and that the file exists.", if err.is_empty() { String::new() } else { format!(" ({err})") });
                } else {
                    self.sync_apps(h);
                    let p = self.km.preview_file(&text, &secs);
                    let err = p["error"].as_str().map(String::from);
                    let ie = self.ie(gestures);
                    if let Some(e) = err {
                        ie.error = e;
                    } else {
                        ie.preview = Some(p);
                        ie.preview_seq += 1;
                    }
                }
                self.push_feed(h, false);
            }
            M::KmEnd4 { reply, code, out } => {
                let preview = if code != 0 {
                    json!({"error": if code == 127 { "Python 3 is needed to read end4's files. Install python, then try again.".to_string() } else { format!("Couldn't read end4's files (exit {code}).") }})
                } else {
                    match serde_json::from_str::<Value>(&out) {
                        Err(_) => json!({"error": "Couldn't read end4's files: the helper's output was cut off. Try again."}),
                        Ok(p) if p["files"].as_array().is_some_and(|f| f.is_empty()) => json!({"error": "No end4 config was found in ~/.config/hypr/hyprland/. Install end4's dotfiles, or import a Tessera file instead."}),
                        Ok(p) => self.km.match_end4(&p),
                    }
                };
                Self::reply(h, &reply, json!({"preview": preview}));
            }
            M::End4 { gestures, code, out } => {
                self.sync_apps(h);
                self.ie(gestures).busy = false;
                let res: Result<Value, String> = if code != 0 {
                    Err(if code == 127 { "Python 3 is needed to read end4's files. Install python, then try again.".into() } else { format!("Couldn't read end4's files (exit {code}).") })
                } else {
                    match serde_json::from_str::<Value>(&out) {
                        Err(_) => Err("Couldn't read end4's files: the helper's output was cut off. Try again.".into()),
                        Ok(p) if p["files"].as_array().is_some_and(|f| f.is_empty()) => Err("No end4 config was found in ~/.config/hypr/hyprland/. Install end4's dotfiles, or import a Tessera file instead.".into()),
                        Ok(p) => Ok(self.km.match_end4(&p)),
                    }
                };
                let ie = self.ie(gestures);
                match res {
                    Err(e) => ie.error = e,
                    Ok(p) => {
                        ie.preview = Some(p);
                        ie.preview_seq += 1;
                    }
                }
                self.push_feed(h, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_with_one_space_indent() {
        assert_eq!(pretty1(&json!({"a": [1, 2], "b": {}})), "{\n \"a\": [\n  1,\n  2\n ],\n \"b\": {}\n}");
    }

    #[test]
    fn paths_and_import_state() {
        assert_eq!(expand("/home/u", "~/Documents/x.json"), "/home/u/Documents/x.json");
        assert_eq!(expand("/home/u", " ~ "), "/home/u");
        assert_eq!(Ie::default_path(&["gestures".to_string()]), "~/Documents/tessera-gestures.json");
        assert_eq!(Ie::default_path(&["keybinds".to_string(), "shortcuts".to_string()]), "~/Documents/tessera-shortcuts.json");
        let mut ie = Ie::default();
        ie.ask_path(true, vec!["keybinds".into()]);
        assert_eq!((ie.path_mode.as_str(), ie.path_seq), ("export", 1));
        assert_eq!(ie.state(false)["canUndo"], false);
    }
}
