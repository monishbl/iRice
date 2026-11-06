//! Sharing with nearby devices, both ways Tessera speaks: LocalSend and Quick
//! Share (shell/services/Sharing.qml). The share sheet, the Control Center
//! module and its page use this, not the two protocols directly:
//!   - level: who can see this computer, for both at once (off | 10min |
//!     everyone); each can still be set on its own (the page, Settings)
//!   - devices: everything nearby as one list, a device seen both ways once
//!   - look(): find devices while something is showing them (counted, so the
//!     sheet and a Control Center module can both be open)
//!   - share_text(): text (a selection, a link) goes as a small text file,
//!     the one kind of item both protocols take
//! and the incoming requests (the share card's AirDrop-style alert).

use crate::devices::{self, Device};
use crate::{Job, Sharing};
use serde_json::{json, Value};
use std::collections::HashMap;
use tessera_core::host::Host;

#[derive(Default)]
pub struct Share {
    pub lookers: i32,
    /// the dev sandbox's made-up devices: nothing real is looked for meanwhile
    pub demo: Option<Value>,
    /// [{ id ("qs:in-1"), via, session, from, type, what, files, total, pin,
    ///    state: asking | receiving | done | failed, paths, message, hidden }]
    pub requests: Vec<Value>,
    /// nameKey -> (sent | declined | failed, at ms)
    pub outcomes: HashMap<String, (String, i64)>,
    pub last_real: bool,
}

impl Sharing {
    pub fn level(&self) -> String {
        devices::combine(&self.cfg_str("sharing.receiving", "off"), &self.cfg_str("sharing.quickShare", "off"))
    }
    /// The two differ (set separately on the page): the module says so
    pub fn mixed(&self) -> bool {
        self.cfg_str("sharing.receiving", "off") != self.cfg_str("sharing.quickShare", "off")
    }
    pub fn level_text(&self) -> String {
        let l = self.level();
        if l == "off" {
            "Off".into()
        } else if self.mixed() {
            "Custom".into()
        } else if l == "10min" {
            "Everyone for 10 min".into()
        } else {
            "Everyone".into()
        }
    }
    pub fn set_level(&mut self, l: &str) {
        self.set_config("sharing.receiving", json!(l));
        self.set_config("sharing.quickShare", json!(l));
    }

    // ── Nearby devices ──────────────────────────────────────────────────
    pub fn devices(&self) -> Vec<Device> {
        match &self.share.demo {
            Some(d) => {
                let get = |k: &str, name: &str| -> Vec<(String, String, String)> {
                    d[k].as_array().map(|a| a.iter().map(|p| (crate::localsend::js(&p[name]), crate::localsend::js(&p["type"]), crate::localsend::js(&p["id"]))).collect()).unwrap_or_default()
                };
                devices::merge(&get("localsend", "alias"), &get("quickshare", "name"))
            }
            None => devices::merge(&self.local.peers, &self.quick.devices),
        }
    }
    pub fn looking(&self) -> bool {
        self.share.lookers > 0
    }
    pub fn look(&mut self, on: bool) {
        self.share.lookers = (self.share.lookers + if on { 1 } else { -1 }).max(0);
    }
    /// onLookingForRealChanged (each turn)
    pub fn share_sync(&mut self) {
        let real = self.looking() && self.share.demo.is_none();
        if real != self.share.last_real {
            self.share.last_real = real;
            self.local.sharing = real;
            self.quick.looking = real;
            if real {
                self.local_scan();
            }
        }
    }

    /// Send to a device from `devices`: Quick Share for Android phones (no
    /// app to open on the phone), LocalSend for the rest
    pub fn send_to(&mut self, d: &Device, paths: &[String]) {
        if paths.is_empty() {
            return;
        }
        if d.prefer == "quickshare" {
            if let Some(id) = d.quickshare.clone() {
                self.quick_send(&id, paths);
            }
        } else if let Some(id) = d.localsend.clone() {
            self.local_send(&id, paths);
        }
    }
    /// The transfers in flight (the demo's, else LocalSend's and Quick Share's)
    pub fn sharing_items(&self) -> Vec<Value> {
        match &self.share.demo {
            Some(d) => d["items"].as_array().cloned().unwrap_or_default(),
            None => {
                let mut v = self.local.active.values();
                v.extend(self.quick.active.values());
                v
            }
        }
    }
    /// The send to this device in flight, for its progress ring
    pub fn progress_for(&self, d: &Device) -> Option<Value> {
        devices::progress_for(&self.sharing_items(), &d.name).cloned()
    }

    // ── Text ────────────────────────────────────────────────────────────
    // Written to a private folder as a .txt file; the sheet shows the text
    // itself. (Quick Share can also send text and links natively; that needs
    // a sendText in the helper, which doesn't exist yet: TODO as in the QML.)
    fn text_dir() -> String {
        format!("{}/tessera-share", std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into()))
    }
    pub fn share_text(&mut self, h: &mut Host, text: &str, screen: Option<String>) {
        let t = text.trim();
        if t.is_empty() {
            self.note(h, "Nothing selected", "Select some text, then try again.");
            return;
        }
        let is_link = (t.starts_with("http://") || t.starts_with("https://")) && !t.contains(char::is_whitespace);
        let dir = Self::text_dir();
        let file = format!("{dir}/{} {}.txt", if is_link { "Link" } else { "Text" }, tessera_core::util::now("%Y-%m-%d %H.%M.%S"));
        let cmd = crate::proc::s(&["sh", "-c", "umask 077; mkdir -p \"$1\" && printf '%s\\n' \"$3\" > \"$2\"", "sh", &dir, &file, t]);
        self.run(h, Job::TextWrite { file, text: t.to_string(), screen }, &cmd, None);
    }
    pub fn text_written(&mut self, h: &mut Host, file: String, text: String, screen: Option<String>, code: i32) {
        if code != 0 {
            let dir = Self::text_dir();
            self.note(h, "Couldn't share the text", &format!("The temporary folder isn't writable. Check that {dir} can be created."));
            return;
        }
        self.panels_share(h, vec![file], screen, &text);
    }
    /// Share what's selected (the primary selection), else what's copied
    pub fn share_selection(&mut self, h: &mut Host) {
        let cmd = crate::proc::s(&["sh", "-c", "t=$(wl-paste -p -n 2>/dev/null); [ -n \"$t\" ] || t=$(wl-paste -n -t text 2>/dev/null); printf '%s' \"$t\""]);
        self.run(h, Job::Selection, &cmd, None);
    }
    /// Panels.share(paths, screen, note): the sheet's paths are set here, then
    /// the shell opens the sheet
    pub fn panels_share(&mut self, h: &mut Host, paths: Vec<String>, screen: Option<String>, note: &str) {
        self.sheet_paths = paths.clone();
        self.sheet_note = note.to_string();
        h.ask("panels.share", json!([paths, screen, note]));
    }

    pub fn note(&mut self, h: &mut Host, title: &str, subtitle: &str) {
        if self.cfg_bool("island.enabled", true) {
            self.island(h, json!({"kind": "custom", "icon": "share-network", "title": title, "subtitle": subtitle}), 4000);
        } else {
            crate::proc::detach(&["notify-send", "-a", "Tessera", title, subtitle]);
        }
    }

    // ── Incoming requests (the share surface's card: AirDrop's alert) ────
    // One shown at a time, both protocols alike: asking (Accept / Decline),
    // then receiving (a ring around the sender), then done (Open, Show in
    // Files) or failed. Hidden: the transfer goes on in the island, and what
    // arrives is said there as before.
    pub fn request(&self) -> Option<&Value> {
        self.share.requests.iter().find(|r| !r["hidden"].as_bool().unwrap_or(false))
    }
    pub fn add_request(&mut self, r: Value) {
        let id = r["id"].clone();
        self.share.requests.retain(|x| x["id"] != id);
        let mut base = json!({"state": "asking", "hidden": false, "paths": [], "message": ""});
        if let (Some(b), Some(n)) = (base.as_object_mut(), r.as_object()) {
            for (k, v) in n {
                b.insert(k.clone(), v.clone());
            }
        }
        self.share.requests.push(base);
    }
    pub fn find_request(&self, id: &str) -> Option<Value> {
        self.share.requests.iter().find(|r| r["id"] == id).cloned()
    }
    pub fn update_request(&mut self, id: &str, patch: Value) {
        for r in self.share.requests.iter_mut() {
            if r["id"] == id {
                if let (Some(rm), Some(pm)) = (r.as_object_mut(), patch.as_object()) {
                    for (k, v) in pm {
                        rm.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    }
    pub fn drop_request(&mut self, id: &str) {
        self.share.requests.retain(|r| r["id"] != id);
    }
    /// A protocol's helper stopped: its open requests failed with it (the card
    /// stayed on "receiving" for good)
    pub fn helper_ended(&mut self, via: &str) {
        for r in self.share.requests.iter_mut() {
            if r["via"] == via && (r["state"] == "asking" || r["state"] == "receiving") {
                r["state"] = json!("failed");
                r["message"] = json!("The transfer stopped before everything arrived.");
            }
        }
    }
    pub fn answer(&mut self, id: &str, accept: bool) {
        let Some(r) = self.find_request(id) else { return };
        if r["via"] == "demo" {
            if accept {
                self.update_request(id, json!({"state": "receiving"}));
            } else {
                self.drop_request(id);
            }
            return;
        }
        let session = crate::localsend::js(&r["session"]);
        let quick = r["via"] == "quickshare";
        let msg = json!({"cmd": "answer", "session": session, "accept": accept});
        if quick {
            self.quick_tell(msg);
        } else {
            self.local_tell(msg);
        }
        if accept {
            self.update_request(id, json!({"state": "receiving"}));
        } else {
            if quick {
                self.quick_finish(&session);
            } else {
                self.local.active.remove(&session);
            }
            self.drop_request(id);
        }
    }
    /// The card shows this transfer's progress (the service's own item)
    pub fn progress_of(&self, r: &Value) -> Option<Value> {
        if r["via"] == "demo" {
            return r.get("progress").filter(|p| !p.is_null()).cloned();
        }
        let session = crate::localsend::js(&r["session"]);
        if r["via"] == "quickshare" {
            self.quick.active.get(&session).cloned()
        } else {
            self.local.active.get(&session).cloned()
        }
    }
    /// A request that ended while hidden is said in the island, as before
    pub fn shown_request(&self, id: &str) -> bool {
        self.find_request(id).is_some_and(|r| !r["hidden"].as_bool().unwrap_or(false))
    }
    pub fn cancel_request(&mut self, id: &str) {
        let Some(r) = self.find_request(id) else { return };
        if r["via"] != "demo" {
            let session = crate::localsend::js(&r["session"]);
            if r["via"] == "quickshare" {
                self.quick_cancel(&session);
            } else {
                self.local_cancel(&session);
            }
        }
        self.drop_request(id);
    }

    // ── How sends ended, for the sheet ("Sent" under a device, a while) ──
    pub fn outcome(&mut self, name: &str, state: &str) {
        self.share.outcomes.insert(devices::name_key(name), (state.to_string(), tessera_core::util::now_ms()));
    }
    pub fn outcome_for(&self, name: &str) -> String {
        match self.share.outcomes.get(&devices::name_key(name)) {
            Some((s, at)) if tessera_core::util::now_ms() - at < 60000 => s.clone(),
            _ => String::new(),
        }
    }

    /// The dev sandbox only: the incoming card without a phone, the sheet
    /// with made-up devices
    ///   sharingdemo ask | progress <0-100> | done | fail
    ///   sharingdemo peers | none | sending <0-100> | sent | real
    pub fn sharingdemo(&mut self, h: &mut Host, func: &str, args: &[String]) {
        if !tessera_core::env::sandboxed() {
            return;
        }
        let pct = args.first().and_then(|a| a.parse::<f64>().ok()).unwrap_or(0.0);
        match func {
            "ask" => self.add_request(json!({"id": "demo:1", "via": "demo", "session": "1", "from": "Maya's phone", "type": "phone", "what": "2 photos",
                "files": [{"name": "IMG_2041.jpg", "mime": "image/jpeg"}, {"name": "IMG_2042.jpg", "mime": "image/jpeg"}], "total": 6200000, "pin": "4821"})),
            "progress" => self.update_request("demo:1", json!({"state": "receiving", "progress": {"done": 6200000.0 * pct / 100.0, "total": 6200000, "waiting": false, "transport": "Wi-Fi Direct"}})),
            "done" => self.update_request("demo:1", json!({"state": "done", "paths": ["/tmp/IMG_2041.jpg", "/tmp/IMG_2042.jpg"]})),
            // (the share sheet with made-up devices, never the real network's)
            "peers" => {
                self.share.demo = Some(json!({
                    "localsend": [{"alias": "Ada's laptop", "type": "laptop", "id": "demo-1"}, {"alias": "Studio desktop", "type": "desktop", "id": "demo-2"}],
                    "quickshare": [{"name": "Kai's Pixel", "type": "phone", "id": "demo-3"}, {"name": "Ines's tablet", "type": "tablet", "id": "demo-4"}],
                    "items": []}))
            }
            "none" => self.share.demo = Some(json!({"localsend": [], "quickshare": [], "items": []})),
            "sending" => {
                if let Some(d) = self.share.demo.as_mut() {
                    d["items"] = json!([{"kind": "send", "peer": "Ada's laptop", "done": pct, "total": 100, "waiting": false}]);
                }
            }
            "sent" => {
                if let Some(d) = self.share.demo.as_mut() {
                    d["items"] = json!([]);
                }
                self.outcome("Ada's laptop", "sent");
            }
            "real" => self.share.demo = None,
            "fail" => self.update_request("demo:1", json!({"state": "failed", "message": "Maya's phone stopped sending."})),
            _ => {}
        }
        let _ = h;
    }
}
