//! The host's sharing services (wave 4, core/src/host.rs): the QML shell's
//! nearby sharing, casting, mirroring, phone, drop shelf, transfers and App
//! Store services, standalone. One `Sharing` service holds them all (they
//! lean on each other: Sharing merges LocalSend's and Quick Share's devices,
//! SharingPorts watches all of them, the share sheet feed reads five), each
//! in a module that names the QML file it replaces. PORT-NOTES.md says what
//! is ported, what differs, and how to check it.
//!
//! Calls (`Service::calls`): "share", "shelf" and "appstore" (the surfaces'
//! `shell::ask("share", [what, ...])`, and `share.<what>` from the other
//! hosts). IPC targets: sharingdemo, linkshare, cast, mirroring, shelf.
//! The shell state it publishes (Host::set_state): sharing, quickshare,
//! localsend, cast, mirroring, phone, airplay, fastpair, shelf, transfers,
//! appstore (for the Control Center, Settings, the island and the dock,
//! which are other hosts').

#[allow(unused_imports)]
use tessera_core::*;
use tessera_core::host::{Host, Service};

use serde_json::{json, Value};
use std::any::Any;

pub mod airplay;
pub mod appstore;
pub mod cast;
pub mod devices;
pub mod download_tracker;
pub mod fastpair;
pub mod linkshare;
pub mod localsend;
pub mod mirroring;
pub mod phone;
pub mod ports;
pub mod proc;
pub mod quickshare;
pub mod shelf;
pub mod shelf_logic;
pub mod sharing;
pub mod sse;
pub mod transfers;
pub mod updates;

use proc::Sched;

// Colours of the island's alerts, as the QML names them (Theme.accents.*,
// Qt.rgba(1, 1, 1, 0.6)): dark-mode values, Qt's #aarrggbb order
pub const GREEN: &str = "#30D158";
pub const ORANGE: &str = "#FF9F0A";
pub const BLUE: &str = "#0A84FF";
pub const DANGER: &str = "#FF453A";
pub const GREY: &str = "#99FFFFFF";
pub const SECONDARY: &str = "#99EBEBF5";

/// A call an island button makes when pressed ({call, args}: the shell's
/// Island runs it as `Host::ask`)
pub fn act(call: &str, args: Value) -> Value {
    json!({"call": call, "args": args})
}

/// The last part of a path
pub fn base(p: &str) -> String {
    p.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}

/// One-shot commands whose output is wanted (StdioCollector)
#[derive(Debug)]
pub enum Job {
    Ports,
    Trust,
    TextWrite { file: String, text: String, screen: Option<String> },
    Selection,
    UpdStatus,
    MirrorStart(String),
    CamCheck,
    CamSetup,
    AirCheck,
    AirStart,
    FastConnect,
    DlSizes,
    DlRecent,
    Flatpak { app: Value, op: String },
    Pick(String),
}

/// Messages from this service's own threads (Host::sender("Sharing"))
pub enum M {
    /// a child's stdout line: (who, generation, line)
    Line(&'static str, u64, String),
    /// a child ended: (who, generation, code, last stderr lines)
    Exit(&'static str, u64, i32, String),
    /// a one-shot's answer: (job, stdout, stderr, code)
    Out(Job, String, String, i32),
    /// the nearest deadline came
    Due,
    Monitors(Vec<shelf::Mon>),
    /// something changed in the Downloads folder
    DlEvent,
}

pub struct Sharing {
    pub sched: Sched,
    /// config.json, as last read (Host::cfg)
    pub cfg: Value,
    pub local: localsend::LocalSend,
    pub quick: quickshare::QuickShare,
    pub ports: ports::Ports,
    pub share: sharing::Share,
    pub link: linkshare::LinkShare,
    pub cast: cast::Cast,
    pub mirror: mirroring::Mirroring,
    pub phone: phone::Phone,
    pub fast: fastpair::FastPair,
    pub air: airplay::AirPlay,
    pub shelf: shelf::Shelf,
    pub tr: transfers::Transfers,
    pub store: appstore::Store,
    pub upd: updates::Upd,
    /// what the sheet shares (Panels.sharePaths, shareNote)
    pub sheet_paths: Vec<String>,
    pub sheet_note: String,
    /// Sharing.look is on for the open sheet
    pub looking_here: bool,
    pub sheet_seen: bool,
    pub sheet_since: Option<std::time::Instant>,
    last_share: Value,
    last_shelf: Value,
    last_store: Value,
    /// the sharing modes as last seen ("10min" starts its timer)
    last_modes: (String, String),
    last_hotspot: bool,
}

impl Default for Sharing {
    fn default() -> Self {
        Self {
            sched: Sched::default(),
            cfg: Value::Null,
            local: Default::default(),
            quick: Default::default(),
            ports: Default::default(),
            share: Default::default(),
            link: Default::default(),
            cast: Default::default(),
            mirror: Default::default(),
            phone: Default::default(),
            fast: Default::default(),
            air: Default::default(),
            shelf: Default::default(),
            tr: Default::default(),
            store: Default::default(),
            upd: Default::default(),
            sheet_paths: vec![],
            sheet_note: String::new(),
            looking_here: false,
            sheet_seen: false,
            sheet_since: None,
            last_share: Value::Null,
            last_shelf: Value::Null,
            last_store: Value::Null,
            last_modes: (String::new(), String::new()),
            last_hotspot: true,
        }
    }
}

/// The service the binary collects (src/main.rs)
pub fn services() -> Vec<Box<dyn Service>> {
    vec![Box::new(Sharing::default())]
}

impl Sharing {
    // ── Shared helpers ──────────────────────────────────────────────────
    fn cfg_at(&self, path: &str) -> &Value {
        path.split('.').fold(&self.cfg, |v, k| &v[k])
    }
    pub fn cfg_str(&self, path: &str, default: &str) -> String {
        self.cfg_at(path).as_str().map(str::to_string).unwrap_or_else(|| default.to_string())
    }
    pub fn cfg_bool(&self, path: &str, default: bool) -> bool {
        self.cfg_at(path).as_bool().unwrap_or(default)
    }
    pub fn cfg_list(&self, path: &str) -> Vec<String> {
        self.cfg_at(path).as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
    }
    /// Write one setting into config.json (the shell's watcher reads it again)
    pub fn set_config(&mut self, path: &str, v: Value) {
        let file = tessera_core::theme::config_path();
        let mut cfg: Value = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({}));
        if !cfg.is_object() {
            cfg = json!({});
        }
        let keys: Vec<&str> = path.split('.').collect();
        let mut at = &mut cfg;
        for k in &keys[..keys.len() - 1] {
            if !at[*k].is_object() {
                at[*k] = json!({});
            }
            at = &mut at[*k];
        }
        at[keys[keys.len() - 1]] = v;
        let text = serde_json::to_string_pretty(&cfg).unwrap_or_default();
        let _ = tessera_core::util::write_atomic(&file, text.as_bytes());
    }
    pub fn next_gen(&mut self) -> u64 {
        proc::next_gen()
    }
    /// A script of the repo's (shell/assets/scripts/<rel>)
    pub fn script(&self, rel: &str) -> String {
        tessera_core::util::asset(&format!("shell/assets/scripts/{rel}")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| rel.to_string())
    }
    pub fn run(&mut self, h: &mut Host, job: Job, cmd: &[String], input: Option<Vec<u8>>) {
        proc::run(h.sender("Sharing"), job, cmd, input);
    }
    /// Tessera's root helper, as RootHelper.command does: through pkexec, which
    /// asks for the password or not as its polkit rule says. Which helper
    /// (Tessera's, else iRice's) is decided when the command runs.
    pub fn root_command(args: &[&str]) -> Vec<String> {
        let pick = "h=/usr/lib/tessera/tessera-root; policy=/usr/share/polkit-1/actions/org.tessera.root.policy; \
            if [ ! -x \"$h\" ] && [ -x /usr/lib/irice/irice-root ]; then h=/usr/lib/irice/irice-root; policy=/usr/share/polkit-1/actions/org.irice.root.policy; fi; ";
        let mut v = vec!["sh".to_string(), "-c".into(), format!("{pick}exec pkexec \"$h\" \"$@\""), "sh".into()];
        v.extend(args.iter().map(|a| a.to_string()));
        v
    }
    /// Island.post(event, ms): the shell's Island posts it (the event may carry
    /// `onAction` / `onAction2` as {call, args}, and a `key` that replaces a card)
    pub fn island(&mut self, h: &mut Host, event: Value, ms: u64) {
        h.ask("island.post", json!({"event": event, "ms": ms}));
    }
    pub fn play(&mut self, h: &mut Host, sound: &str) {
        h.ask("sound.play", json!([sound]));
    }
    pub fn open_link(&mut self, h: &mut Host, url: &str) {
        h.ask("launch.openLink", json!([url]));
    }
    fn drawing(&self, h: &Host, surface: &str) -> bool {
        h.st.shell.surfaces.on.iter().any(|s| *s == surface)
    }

    // ── Files (services/Files.qml: a file manager for folders) ──────────
    fn file_manager(&self) -> Option<String> {
        let want = self.cfg_str("desktop.fileManager", "");
        if !want.is_empty() && tessera_core::util::on_path(&want) {
            return Some(want);
        }
        ["nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm-qt", "pcmanfm"].iter().find(|b| tessera_core::util::on_path(b)).map(|b| b.to_string())
    }
    pub fn open_folder(&self, path: &str) {
        match self.file_manager() {
            Some(m) => proc::detach(&[&m, path]),
            None => proc::detach(&["xdg-open", path]),
        }
    }
    /// A folder in the file manager, anything else in its app
    pub fn open_path(&self, path: &str) {
        let m = self.file_manager().unwrap_or_else(|| "xdg-open".into());
        proc::detach(&["sh", "-c", "if [ -d \"$1\" ]; then exec \"$2\" \"$1\"; else exec xdg-open \"$1\"; fi", "sh", path, &m]);
    }
    /// The folder it's in, with it selected where the file manager can
    pub fn reveal(&self, path: &str) {
        let dir = match path.rsplit_once('/') {
            Some(("", _)) | None => "/".to_string(),
            Some((d, _)) => d.to_string(),
        };
        match self.file_manager().as_deref() {
            Some(m @ ("nautilus" | "dolphin" | "nemo")) => proc::detach(&[m, "--select", path]),
            _ => self.open_folder(&dir),
        }
    }

    // ── The share sheet's feed (modules/share/ShareFeed.qml) ────────────
    fn castable(&self) -> bool {
        if self.sheet_paths.len() != 1 || !self.sheet_note.is_empty() {
            return false;
        }
        let low = self.sheet_paths[0].to_lowercase();
        let ext = low.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        ["mp4", "m4v", "webm", "mkv", "mov", "mp3", "m4a", "aac", "ogg", "oga", "opus", "flac", "wav", "jpg", "jpeg", "png", "gif", "webp"].contains(&ext)
    }
    fn cast_key(d: &Value) -> String {
        let id = &d["id"];
        format!("cast:{}", if id.is_null() { format!("{}:{}", localsend::js(&d["host"]), localsend::js(&d["port"])) } else { localsend::js(id) })
    }
    /// Search.iconForFile: the theme icon for what's shared, as a file: URL
    fn sheet_icon(&self) -> String {
        let name = if !self.sheet_note.is_empty() {
            "text-x-generic"
        } else if self.sheet_paths.len() == 1 {
            let ext = self.sheet_paths[0].rsplit('.').next().unwrap_or("").to_lowercase();
            if ["png", "jpg", "jpeg", "webp", "gif", "svg"].contains(&ext.as_str()) {
                "image-x-generic"
            } else if ["mp4", "mkv", "webm", "mov"].contains(&ext.as_str()) {
                "video-x-generic"
            } else if ["mp3", "flac", "ogg", "wav", "m4a"].contains(&ext.as_str()) {
                "audio-x-generic"
            } else if ext == "pdf" {
                "application-pdf"
            } else if ["zip", "tar", "gz", "xz", "7z", "rar"].contains(&ext.as_str()) {
                "package-x-generic"
            } else {
                "text-x-generic"
            }
        } else {
            "folder-documents"
        };
        tessera_core::icon_theme::find(name, None).or_else(|| tessera_core::icon_theme::find("text-x-generic", None)).map(|p| format!("file://{}", p.to_string_lossy())).unwrap_or_default()
    }
    fn sheet(&self) -> Value {
        let items = self.sharing_items();
        let mut devs: Vec<Value> = Vec::new();
        let one = |name: &str, key: &str, kind: &str, cast: bool, cast_icon: &str, prefer: &str| -> Value {
            let s = devices::progress_for(&items, name);
            json!({"key": key, "name": name, "type": kind, "cast": cast, "castIcon": cast_icon, "prefer": prefer,
                "sending": s.map(|s| json!({"done": s["done"].as_f64().unwrap_or(0.0), "total": s["total"].as_f64().unwrap_or(0.0), "waiting": s["waiting"].as_bool().unwrap_or(false)})),
                "ended": if s.is_some() { String::new() } else { self.outcome_for(name) }})
        };
        for d in self.devices() {
            devs.push(one(&d.name, &d.key, &d.kind, false, "", &d.prefer));
        }
        if self.castable() {
            for d in &self.cast.devices {
                devs.push(one(&localsend::js(&d["name"]), &Self::cast_key(d), &localsend::js(&d["type"]), true, Self::cast_icon(d), &localsend::js(&d["prefer"])));
            }
        }
        let errors: Vec<&str> = [self.local.error.as_str(), self.quick.error.as_str()].into_iter().filter(|e| !e.is_empty()).collect();
        json!({"paths": self.sheet_paths, "note": self.sheet_note, "icon": self.sheet_icon(), "devices": devs, "available": self.local_available(),
            "portsError": self.ports.error, "errors": errors.join(" ")})
    }
    fn request_feed(&self) -> Value {
        let Some(r) = self.request() else { return Value::Null };
        let it = self.progress_of(r);
        let total = r["total"].as_f64().unwrap_or(0.0);
        let folder = self.cfg_str("sharing.folder", "");
        json!({"id": localsend::js(&r["id"]), "from": localsend::js(&r["from"]), "type": localsend::js(&r["type"]),
            "what": if r["what"].is_null() { "something".to_string() } else { localsend::js(&r["what"]) },
            "files": r["files"].as_array().map(|a| a.iter().map(|f| json!({"name": localsend::js(&f["name"]), "mime": localsend::js(&f["mime"])})).collect::<Vec<_>>()).unwrap_or_default(),
            "total": total, "totalText": if total > 0.0 { transfers::size_text(total) } else { String::new() }, "pin": localsend::js(&r["pin"]), "state": localsend::js(&r["state"]),
            "message": localsend::js(&r["message"]), "paths": r["paths"].clone(),
            "folder": if folder.is_empty() { "Downloads".to_string() } else { base(&folder) },
            "item": it.map(|it| json!({"done": it["done"].as_f64().unwrap_or(0.0), "total": it["total"].as_f64().unwrap_or(0.0), "waiting": it["waiting"].as_bool().unwrap_or(false),
                "transport": localsend::js(&it["transport"]), "doneText": transfers::size_text(it["done"].as_f64().unwrap_or(0.0)), "totalText": transfers::size_text(it["total"].as_f64().unwrap_or(0.0))}))})
    }
    fn focused(&self, h: &Host) -> String {
        h.st.shell.get::<tessera_core::services::hypr::Snapshot>().map(|s| s.focused.clone()).unwrap_or_default()
    }
    fn share_feed(&self, h: &Host) -> Value {
        json!({"sheet": self.sheet(), "request": self.request_feed(), "link": self.link_feed(), "screen": self.focused(h)})
    }

    /// ShareFeed.call: what the share sheet and cards ask
    fn share_call(&mut self, h: &mut Host, what: &str, a: &[Value]) {
        let s = |i: usize| a.get(i).map(localsend::js).unwrap_or_default();
        match what {
            "send" => {
                let key = s(0);
                if let Some(c) = self.cast.devices.iter().find(|d| Self::cast_key(d) == key).cloned().filter(|_| self.castable()) {
                    if let Some(p) = self.sheet_paths.first().cloned() {
                        self.cast_file(h, &c, &p);
                    }
                    h.ask("panels.close", json!([]));
                } else if let Some(d) = self.devices().into_iter().find(|d| d.key == key) {
                    let paths = self.sheet_paths.clone();
                    self.send_to(&d, &paths);
                }
            }
            "copy" => {
                if !self.sheet_note.is_empty() {
                    tessera_core::util::pipe_to(&["wl-copy"], self.sheet_note.clone().into_bytes());
                } else if !self.sheet_paths.is_empty() {
                    let mut cmd = vec!["sh", "-c", "printf '%s\\n' \"$@\" | sed 's|^|file://|' | wl-copy -t text/uri-list", "sh"];
                    let paths = self.sheet_paths.clone();
                    cmd.extend(paths.iter().map(String::as_str));
                    proc::detach(&cmd);
                }
            }
            "reveal" => {
                if let Some(p) = self.sheet_paths.first() {
                    self.reveal(p);
                }
            }
            "qr" => {
                if !self.sheet_paths.is_empty() {
                    let paths = self.sheet_paths.clone();
                    self.link_send(h, &paths);
                }
            }
            "answer" => self.answer(&s(0), a.get(1).and_then(Value::as_bool) == Some(true)),
            "cancel" => self.cancel_request(&s(0)),
            "hide" => self.update_request(&s(0), json!({"hidden": true})),
            "drop" => self.drop_request(&s(0)),
            "open" => {
                if let Some(r) = self.find_request(&s(0)) {
                    if let Some(p) = r["paths"][0].as_str() {
                        self.open_path(p);
                    }
                    self.drop_request(&s(0));
                }
            }
            "show" => {
                if let Some(r) = self.find_request(&s(0)) {
                    self.reveal(r["paths"][0].as_str().unwrap_or(""));
                    self.drop_request(&s(0));
                }
            }
            "linkClose" => self.link_close(h),
            "installQr" => {
                if !self.upd.running() {
                    self.update_run("qr", &["--install", "qrencode"]);
                }
            }
            // the other hosts' and the islands' calls (Panels.share, Files, Sharing.*)
            "paths" => {
                self.sheet_paths = a.first().and_then(Value::as_array).map(|p| p.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
                self.sheet_note = s(1);
                self.sheet_since = Some(std::time::Instant::now());
                self.sched.set("sheet_poll", 100);
            }
            "sheetOpen" => self.looking_here_set(a.first().and_then(Value::as_bool) == Some(true)),
            "look" => self.look(a.first().and_then(Value::as_bool) == Some(true)),
            "setLevel" => self.set_level(&s(0)),
            "text" => self.share_text(h, &s(0), a.get(1).and_then(Value::as_str).map(str::to_string)),
            "selection" => self.share_selection(h),
            "pickFor" => {
                if let Some(d) = self.devices().into_iter().find(|d| d.key == s(0) || d.name == s(0)) {
                    let cmd = vec!["python3".to_string(), self.script("pick-file.py"), "--multiple".into(), "--title".into(), format!("Send files to {}", d.name)];
                    self.run(h, Job::Pick(d.key.clone()), &cmd, None);
                }
            }
            "fileOpen" => self.open_path(&s(0)),
            "fileReveal" => self.reveal(&s(0)),
            "openFolder" => self.open_folder(&s(0)),
            "openDownloads" => self.open_downloads(),
            "openLink" => self.open_link(h, &s(0)),
            "copyText" => tessera_core::util::pipe_to(&["wl-copy"], s(0).into_bytes()),
            "portsRetry" => self.ports_retry(h),
            "trustSet" => self.quick_set_trusted(h, a.first().and_then(Value::as_bool) == Some(true)),
            "trustCheck" => self.quick_check_trust(),
            "bluetoothOn" => self.bluetooth_turn_on(),
            "linkReceive" => self.link_receive(h),
            "transferCancel" => {
                if let Some(i) = a.first() {
                    self.transfer_cancel(&i.clone());
                }
            }
            "castPlaying" => {
                if let Some(d) = a.first() {
                    self.cast_playing(h, &d.clone());
                }
            }
            "castFile" => {
                if let Some(d) = a.first() {
                    self.cast_file(h, &d.clone(), &s(1));
                }
            }
            "castToggle" => self.cast_toggle(),
            "castStop" => self.cast_stop(),
            "castSeek" => self.cast_seek(a.first().and_then(Value::as_f64).unwrap_or(0.0)),
            "castVolume" => self.cast_volume(a.first().and_then(Value::as_f64).unwrap_or(1.0)),
            "mirrorStart" => self.mirror_start(h, &s(0), &s(1)),
            "mirrorStop" => self.mirror_stop(),
            "mirrorLook" => self.mirror_look(a.first().and_then(Value::as_bool) == Some(true)),
            "mirrorInstall" => self.mirror_install(),
            "phoneMirror" => self.phone_mirror(h, &s(0)),
            "phoneWebcam" => self.phone_set_webcam(h, a.first().and_then(Value::as_bool) == Some(true)),
            "phoneSetUpCam" => self.phone_set_up_cam(h),
            "phoneCheckCam" => self.phone_check_cam(h),
            "phonePair" => self.phone_pair(h),
            "phoneCancelPair" => self.phone_cancel_pair(),
            "phoneInstall" => self.phone_install(a.first().and_then(Value::as_bool) == Some(true)),
            "fastConnect" => {
                if let Some(d) = a.first() {
                    self.fast_connect(h, &d.clone());
                }
            }
            "fastRepair" => {
                if let Some(d) = a.first() {
                    self.fast_repair(h, &d.clone(), &s(1));
                }
            }
            "airInstall" => self.air_install(),
            "airCheck" => self.air_check(h),
            _ => eprintln!("tessera-ui: host: no share call {what}"),
        }
    }

    /// Sharing.look while the sheet is open (counted)
    fn looking_here_set(&mut self, on: bool) {
        if on != self.looking_here {
            self.looking_here = on;
            self.look(on);
        }
    }

    // ── The change handlers of the QML's bindings, and the feeds ─────────
    /// "Everyone for 10 minutes" turns itself off; a changed hotspot setting
    /// is told to Quick Share
    fn modes_check(&mut self) {
        let (r, q) = (self.cfg_str("sharing.receiving", "off"), self.cfg_str("sharing.quickShare", "off"));
        let now = tessera_core::util::now_ms();
        if r != self.last_modes.0 {
            if r == "10min" {
                self.local.everyone_until = now + 10 * 60_000;
            }
            self.last_modes.0 = r.clone();
        }
        if q != self.last_modes.1 {
            if q == "10min" {
                self.quick.everyone_until = now + 10 * 60_000;
            }
            self.last_modes.1 = q.clone();
        }
        for (mode, until, name) in [(&r, self.local.everyone_until, "ls10"), (&q, self.quick.everyone_until, "qs10")] {
            if mode == "10min" {
                if !self.sched.has(name) {
                    self.sched.set(name, ((until - now).max(1000)) as u64);
                }
            } else {
                self.sched.cancel(name);
            }
        }
        let hotspot = self.cfg_bool("sharing.quickShareHotspot", true);
        if hotspot != self.last_hotspot {
            self.last_hotspot = hotspot;
            self.quick_configure();
        }
    }

    fn settle(&mut self, h: &mut Host) {
        self.modes_check();
        self.share_sync();
        self.local_sync(h);
        self.quick_sync(h);
        self.ports_check(h);
        self.cast_sync(h);
        self.mirror_sync(h);
        self.phone_sync(h);
        self.fast_sync(h);
        self.air_sync(h);
        self.store_sync(h);
        self.transfers_sync(h);
        self.share_sync();
        self.publish(h);
        self.sched.arm(h.sender("Sharing"));
    }

    /// Tell the surfaces what changed (each one's feed), and the shell state
    fn publish(&mut self, h: &mut Host) {
        if self.drawing(h, "share") {
            let f = self.share_feed(h);
            if f != self.last_share {
                self.last_share = f.clone();
                h.tell("share", json!({"feed": f}));
            }
        }
        if self.drawing(h, "shelf") {
            let f = self.shelf_feed();
            if f != self.last_shelf {
                self.last_shelf = f.clone();
                h.tell("shelf", json!({"state": f}));
            }
        }
        if self.drawing(h, "appstore") {
            let f = self.store_feed();
            if f != self.last_store {
                self.last_store = f.clone();
                h.tell("appstore", json!({"state": f}));
            }
        }
        // the shell state the other hosts read (Control Center, Settings, the island, the dock)
        let devs = self.devices();
        let sharing = json!({"level": self.level(), "mixed": self.mixed(), "on": self.level() != "off", "levelText": self.level_text(), "looking": self.looking(),
            "devices": devs.iter().map(|d| json!({"key": d.key, "name": d.name, "type": d.kind, "prefer": d.prefer, "sending": self.progress_for(d), "ended": self.outcome_for(&d.name)})).collect::<Vec<_>>(),
            "ports": {"error": self.ports.error, "failedWith": self.ports.failed_with, "busy": self.ports.running}});
        h.set_state("sharing", sharing);
        h.set_state("quickshare", json!({"available": self.quick_available(), "error": self.quick.error, "name": self.my_name(), "trusted": self.quick.trusted, "trustBusy": self.quick.trust_busy,
            "trustError": self.quick.trust_error, "bluetoothAvailable": self.bluetooth_available(h), "bluetoothOn": self.bluetooth_on(h), "bluetoothNeeded": self.bluetooth_needed(h), "bluetoothEngine": self.quick.bluetooth_engine}));
        h.set_state("localsend", json!({"available": self.local_available(), "error": self.local.error}));
        h.set_state("cast", self.cast_state(h));
        h.set_state("mirroring", self.mirror_state_json());
        h.set_state("phone", self.phone_state());
        h.set_state("airplay", self.air_state());
        let mut levels = serde_json::Map::new();
        for a in self.fast.levels.keys() {
            levels.insert(a.clone(), self.fast_levels_for(a));
        }
        h.set_state("fastpair", json!({"levels": levels}));
        h.set_state("shelf", json!({"count": self.shelf.items.len(), "opened": self.shelf.opened, "zone": self.shelf.zone, "probing": self.shelf.probing,
            "probe": self.shelf.probe.as_ref().map(|p| json!({"x": p.0, "y": p.1, "screen": p.2}))}));
        h.set_state("transfers", self.transfers_state());
        h.set_state("appstore", self.store_state());
    }

    fn due(&mut self, h: &mut Host, name: &'static str) {
        match name {
            "ports" => self.ports_ask(h),
            "ls10" => {
                if self.cfg_str("sharing.receiving", "off") == "10min" {
                    self.set_config("sharing.receiving", json!("off"));
                }
            }
            "qs10" => {
                if self.cfg_str("sharing.quickShare", "off") == "10min" {
                    self.set_config("sharing.quickShare", json!("off"));
                }
            }
            "ls_scan" => self.local_scan(),
            "ls_pending" => self.local.send_pending = false,
            "qs_pending" => self.quick.send_pending = false,
            "qs_retry" => self.quick_retry(h),
            "link_gather" => self.link_gathered(h),
            "upd_poll" => self.update_poll(h),
            "store_idle" => self.store_idle(),
            "store_quick" => self.store_quick_due(),
            "dl_look" => self.dl_look(h),
            // Startup.later: after the desktop is up
            "later" => {
                self.phone_check_cam(h);
                self.air_check(h);
            }
            "sheet_poll" => self.sheet_poll(h),
            n if n.starts_with("shelf_") => self.shelf_due(h, n),
            // a rest after a helper ended: the sync starts it again
            _ => {}
        }
    }

    /// The sheet is open while the shell's state says so (Panels.open === "share"):
    /// looked at for a moment after the paths are set, and while it stays open
    fn sheet_poll(&mut self, h: &mut Host) {
        let open = h.state("open").as_str() == Some("share") && self.drawing(h, "share");
        if open {
            self.sheet_seen = true;
            self.looking_here_set(true);
            self.sched.set("sheet_poll", 300);
        } else if self.sheet_seen {
            self.sheet_seen = false;
            self.looking_here_set(false);
        } else if self.sheet_since.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(3)) {
            self.sched.set("sheet_poll", 100);
        } else {
            self.looking_here_set(false);
        }
    }

    fn handle_call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let (prefix, rest) = call.split_once('.').unwrap_or((call, ""));
        let list: Vec<Value> = match args {
            Value::Array(a) => a.clone(),
            Value::Null => vec![],
            v => vec![v.clone()],
        };
        // `shell::ask("share", ["send", key])`: the first word is what; `share.send` + [key] the same
        let (what, a): (String, Vec<Value>) = if rest.is_empty() {
            (list.first().and_then(Value::as_str).unwrap_or("").to_string(), list.iter().skip(1).cloned().collect())
        } else {
            (rest.to_string(), list)
        };
        // the other crates' names for these (host-system's bridges and Control Center
        // ask `airplay.check`, `mirroring.start`…): the same action as a `share.*` one
        if let Some((w, a)) = alias(prefix, &what, &a) {
            return self.share_call(h, w, &a);
        }
        match prefix {
            "share" => self.share_call(h, &what, &a),
            "shelf" => self.shelf_call(h, &what, &a),
            "appstore" => self.store_call(h, &what, &a),
            _ => {}
        }
    }
}

/// The calls other crates make by the QML service's name (`Phone.setWebcam(on)`
/// -> `phone.setWebcam`) and the `share.*` action that does it
fn alias(prefix: &str, what: &str, a: &[Value]) -> Option<(&'static str, Vec<Value>)> {
    let w = match (prefix, what) {
        ("airplay", "check") => "airCheck",
        ("airplay", "install") => "airInstall",
        ("phone", "install") => "phoneInstall",
        ("phone", "mirror") => "phoneMirror",
        ("phone", "pair") => "phonePair",
        ("phone", "cancelPair") => "phoneCancelPair",
        ("phone", "setUpCam") => "phoneSetUpCam",
        ("phone", "checkCam") => "phoneCheckCam",
        ("phone", "setWebcam") => "phoneWebcam",
        ("mirroring", "look") => "mirrorLook",
        ("mirroring", "install") => "mirrorInstall",
        ("mirroring", "stop") => "mirrorStop",
        ("mirroring", "start") => {
            // a display ({uuid, name}) from Control Center, or its uuid from Settings
            let d = a.first().cloned().unwrap_or(Value::Null);
            let (u, n) = if d.is_object() { (d["uuid"].clone(), d["name"].clone()) } else { (d, json!("")) };
            return Some(("mirrorStart", vec![u, n]));
        }
        ("linkshare", "receive") => "linkReceive",
        ("quickshare", "turnOnBluetooth") => "bluetoothOn",
        ("quickshare", "setTrusted") => "trustSet",
        ("sharing", "shareSelection") => "selection",
        ("sharing", "look") => "look",
        ("sharing", "setLevel") => "setLevel",
        ("sharing", "pickFor") => {
            // Control Center passes the device; the action takes its key (or name)
            let d = a.first().cloned().unwrap_or(Value::Null);
            let k = if d.is_object() { if d["key"].is_string() { d["key"].clone() } else { d["name"].clone() } } else { d };
            return Some(("pickFor", vec![k]));
        }
        ("cast", "castPlaying") => "castPlaying",
        ("cast", "stop") => "castStop",
        ("sharingports", "retry") => "portsRetry",
        _ => return None,
    };
    Some((w, a.to_vec()))
}

impl Service for Sharing {
    fn name(&self) -> &'static str {
        "Sharing"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["shelf", "share", "appstore", "airplay", "phone", "mirroring", "linkshare", "quickshare", "sharing", "cast", "sharingports"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["sharingdemo", "linkshare", "cast", "mirroring", "shelf"]
    }

    fn start(&mut self, h: &mut Host) {
        self.cfg = h.cfg().clone();
        // the readings it follows (the focused screen, what plays, Bluetooth)
        h.st.shell.want("hypr");
        h.st.shell.want("mpris");
        h.st.shell.want("bluez");
        self.last_modes = (self.cfg_str("sharing.receiving", "off"), self.cfg_str("sharing.quickShare", "off"));
        self.last_hotspot = self.cfg_bool("sharing.quickShareHotspot", true);
        self.quick.last_mode = self.last_modes.1.clone();
        self.shelf_load();
        self.quick_check_trust();
        self.ports_schedule();
        self.transfers_start(h);
        self.shelf_fetch_monitors(h);
        self.sched.set("later", 15_000);
        self.settle(h);
    }

    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        self.handle_call(h, call, args);
        self.settle(h);
    }

    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        let out = match target {
            "sharingdemo" => {
                self.sharingdemo(h, func, args);
                None
            }
            "linkshare" => {
                match func {
                    "receive" => self.link_receive(h),
                    "close" => self.link_close(h),
                    "demo" => self.link_demo(args.first().map(String::as_str).unwrap_or(""), args.get(1).map(String::as_str).unwrap_or("")),
                    _ => {}
                }
                None
            }
            "cast" => {
                match func {
                    "stop" => self.cast_stop(),
                    "toggle" => self.cast_toggle(),
                    _ => {}
                }
                None
            }
            "mirroring" => {
                if func == "stop" {
                    self.mirror_stop();
                }
                None
            }
            "shelf" => self.shelf_ipc(h, func),
            _ => None,
        };
        self.settle(h);
        out
    }

    fn config_changed(&mut self, h: &mut Host) {
        self.cfg = h.cfg().clone();
        // (the surfaces get everything again: one may have started since)
        self.last_share = Value::Null;
        self.last_shelf = Value::Null;
        self.last_store = Value::Null;
        self.settle(h);
    }

    fn outputs_changed(&mut self, h: &mut Host) {
        self.shelf_fetch_monitors(h);
        self.last_share = Value::Null;
        self.last_shelf = Value::Null;
        self.last_store = Value::Null;
        self.settle(h);
    }

    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Line(who, gen, line) => match who {
                "localsend" => self.local_line(h, gen, &line),
                "quickshare" => self.quick_line(h, gen, &line),
                "linkshare" => self.link_line(gen, &line),
                "cast" => self.cast_line(h, gen, &line),
                "cast-browse" => self.cast_avahi_line(gen, &line),
                "mirror-watch" => self.mirror_line(gen, &line),
                "phone-watch" => self.phone_watch_line(h, gen, &line),
                "phone-pair" => self.phone_pair_line(gen, &line),
                "fast-watch" => self.fast_line(h, gen, &line),
                "appstore" => self.store_line(h, gen, &line),
                "pointer" => self.shelf_pointer_line(h, gen, &line),
                "copies" => self.copies_line(gen, &line),
                _ => {}
            },
            M::Exit(who, gen, code, err) => match who {
                "localsend" => self.local_exit(gen),
                "quickshare" => self.quick_exit(gen),
                "linkshare" => self.link_exit(gen),
                "cast" => self.cast_exit(h, gen),
                "cast-browse" => self.cast_browse_ended(gen),
                "mirror-watch" => self.mirror_exit(gen),
                "phone-watch" => self.phone_watch_exit(gen),
                "scrcpy" => self.phone_mirror_exit(h, gen, code, &err),
                "phone-cam" => self.phone_cam_exit(gen, code, &err),
                "phone-pair" => self.phone_pair_exit(gen),
                "fast-watch" => self.fast_exit(h, gen),
                "airplay" => self.air_child_exit(gen, code, &err),
                "appstore" => self.store_exit(gen),
                "pointer" => self.shelf_pointer_exit(gen),
                "copies" => self.copies_exit(gen),
                _ => {}
            },
            M::Out(job, out, err, code) => match job {
                Job::Ports => self.ports_done(&out, code),
                Job::Trust => self.quick_trust_done(&err, code),
                Job::TextWrite { file, text, screen } => self.text_written(h, file, text, screen, code),
                Job::Selection => self.share_text(h, &out, None),
                Job::UpdStatus => self.update_status(h, &out),
                Job::MirrorStart(uuid) => self.mirror_started(&uuid, &out),
                Job::CamCheck => self.phone_cam_checked(&out),
                Job::CamSetup => self.phone_cam_setup_done(h, &err, code),
                Job::AirCheck => self.air_checked(h, &out),
                Job::AirStart => self.air_started(h, &err, code),
                Job::FastConnect => self.fast_connected(h, &out),
                Job::DlSizes => self.dl_sizes(h, &out),
                Job::DlRecent => self.dl_recent(h, &out),
                Job::Flatpak { app, op } => self.flatpak_done(h, app, &op, &err, code),
                Job::Pick(key) => {
                    let paths: Vec<String> = serde_json::from_str::<Vec<String>>(out.trim()).unwrap_or_default();
                    if code == 0 && !paths.is_empty() {
                        if let Some(d) = self.devices().into_iter().find(|d| d.key == key) {
                            self.send_to(&d, &paths);
                        }
                    }
                }
            },
            M::Due => {
                for name in self.sched.take() {
                    self.due(h, name);
                }
            }
            M::Monitors(m) => self.shelf.monitors = m,
            M::DlEvent => self.sched.set_if_later("dl_look", 300),
        }
        self.settle(h);
    }

    fn quit(&mut self, h: &mut Host) {
        self.shelf_write();
        self.local.helper.stop();
        self.quick.helper.stop();
        self.link.job.stop();
        self.cast.session.stop();
        self.cast.browser.stop();
        self.mirror.watch.kept.stop();
        self.phone.watch.kept.stop();
        self.fast.watch.kept.stop();
        self.store.backend.kept.stop();
        self.tr.watch.kept.stop();
        self.shelf.follower.stop();
        let _ = h;
    }
}
