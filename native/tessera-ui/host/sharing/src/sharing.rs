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
    /// who sent the hidden requests a helper's end dropped: said once in the
    /// island on the next turn (helper_ended has no Host)
    pub hidden_failed: Vec<String>,
    /// the last shared text's file (a second share in the same second gets its own)
    pub last_text_file: String,
}

/// Where shared text is written: the runtime folder (the user's own), else
/// the user's cache folder, never /tmp
pub fn text_dir_from(runtime: Option<String>, cache: &str) -> String {
    match runtime.filter(|s| !s.is_empty()) {
        Some(r) => format!("{r}/tessera-share"),
        None => format!("{cache}/tessera-share"),
    }
}

/// "dir/stem.txt", or "dir/stem (2).txt"… when that is taken
pub fn text_file_name(dir: &str, stem: &str, taken: impl Fn(&str) -> bool) -> String {
    let first = format!("{dir}/{stem}.txt");
    if !taken(&first) {
        return first;
    }
    (2..1000).map(|n| format!("{dir}/{stem} ({n}).txt")).find(|f| !taken(f)).unwrap_or_else(|| format!("{dir}/{stem} {}.txt", tessera_core::util::now_ms()))
}

impl Sharing {
    /// The one visibility (visibility.rs): `sharing.quickShare`
    pub fn level(&self) -> String {
        crate::visibility::level_of(&self.cfg["sharing"]).to_string()
    }
    /// (there's one level since 17 Nov: never "Custom")
    pub fn mixed(&self) -> bool {
        false
    }
    /// "No one", "Everyone", "7 min left"
    pub fn level_text(&self) -> String {
        crate::visibility::status(&self.level(), self.quick.everyone_until, tessera_core::util::now_ms())
    }
    /// Visible to anyone now (the tile is drawn on while so)
    pub fn visible_now(&self) -> bool {
        crate::visibility::visible(&self.level(), self.quick.everyone_until, tessera_core::util::now_ms())
    }
    /// Minutes left of "Everyone for 10 min" (0 otherwise)
    pub fn minutes_left(&self) -> i64 {
        if self.level() == "10min" {
            crate::visibility::minutes_left(self.quick.everyone_until, tessera_core::util::now_ms())
        } else {
            0
        }
    }
    pub fn set_level(&mut self, l: &str) {
        let l = crate::visibility::norm(l);
        // (the old LocalSend key too: the QML fallback reads it)
        for k in [crate::visibility::KEY, crate::visibility::OLD_KEY] {
            self.set_config(k, json!(l));
            if let Some(m) = self.cfg.as_object_mut() {
                tessera_core::cfgfile::put_path(m, k, json!(l));
            }
        }
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
    // (never a shared /tmp folder: another user could make it first and read
    // or swap what's written there; without a runtime folder, the user's own
    // cache folder, and the folder must be the user's own, 0700)
    fn text_dir() -> String {
        text_dir_from(std::env::var("XDG_RUNTIME_DIR").ok(), &tessera_core::util::xdg("XDG_CACHE_HOME", ".cache").to_string_lossy())
    }
    pub fn share_text(&mut self, h: &mut Host, text: &str, screen: Option<String>) {
        let t = text.trim();
        if t.is_empty() {
            self.note(h, "Nothing selected", "Select some text, then try again.");
            return;
        }
        let is_link = (t.starts_with("http://") || t.starts_with("https://")) && !t.contains(char::is_whitespace);
        let dir = Self::text_dir();
        let stem = format!("{} {}", if is_link { "Link" } else { "Text" }, tessera_core::util::now("%Y-%m-%d %H.%M.%S"));
        // (two shares in one second get two files: the first one's send may still be queued)
        let last = std::mem::take(&mut self.share.last_text_file);
        let file = text_file_name(&dir, &stem, |f| f == last || std::path::Path::new(f).exists());
        self.share.last_text_file = file.clone();
        // (noclobber: never written through something already there)
        let cmd = crate::proc::s(&["sh", "-c", "umask 077; mkdir -p \"$1\" && [ -O \"$1\" ] && [ ! -L \"$1\" ] && chmod 700 \"$1\" && set -C && printf '%s\\n' \"$3\" > \"$2\"", "sh", &dir, &file, t]);
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
            tessera_core::util::notify("Tessera", "", "", title, subtitle);
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
    /// Hidden ones are dropped instead (nothing else would ever drop them: the
    /// helper that would have said how they ended is gone) and said once in
    /// the island
    pub fn helper_ended(&mut self, via: &str) {
        let open = |r: &Value| r["via"] == via && (r["state"] == "asking" || r["state"] == "receiving");
        let hidden = |r: &Value| r["hidden"].as_bool().unwrap_or(false);
        for r in self.share.requests.iter().filter(|r| open(r) && hidden(r)) {
            self.share.hidden_failed.push(crate::localsend::js(&r["from"]));
        }
        self.share.requests.retain(|r| !(open(r) && hidden(r)));
        for r in self.share.requests.iter_mut() {
            if open(&*r) {
                r["state"] = json!("failed");
                r["message"] = json!("The transfer stopped before everything arrived.");
            }
        }
    }
    /// The island alert for hidden requests that helper_ended dropped
    pub fn hidden_failed_alert(&mut self, h: &mut Host) {
        if self.share.hidden_failed.is_empty() {
            return;
        }
        let from = std::mem::take(&mut self.share.hidden_failed);
        let title = if from.len() == 1 && !from[0].is_empty() { format!("Nothing more from {}", from[0]) } else { "Sharing stopped".to_string() };
        self.island(
            h,
            json!({"kind": "custom", "failure": true, "icon": "warning-circle", "title": title, "subtitle": "The transfer stopped before everything arrived.", "color": crate::ORANGE}),
            5000,
        );
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

impl Sharing {
    /// `tessera-ui ipc call sharing <fn> …` (the IPC socket is the user's
    /// only): what the tile, the share card and the send window's buttons
    /// do, for scripts and tests on a real session.
    ///   level <off|10min|everyone>   the one visibility, as the tile sets it
    ///   devices                      the send window's list: name, how, key
    ///   request                      the newest request waiting for an answer
    ///   accept | decline             answer that one (never an older one)
    ///   send <device name or key>    send what the open send window holds,
    ///                                only to a device in its list
    /// Refused while the sandbox's made-up devices are shown (sharingdemo).
    pub fn sharing_ipc(&mut self, func: &str, args: &[String]) -> String {
        if self.share.demo.is_some() && func != "level" {
            return "refused: the demo devices are showing (sharingdemo real ends it)".into();
        }
        match func {
            "level" => {
                let l = args.first().map(String::as_str).unwrap_or("");
                if crate::visibility::LEVELS.contains(&l) {
                    self.set_level(l);
                    format!("visible to {l}")
                } else {
                    "level takes off, 10min or everyone".into()
                }
            }
            "devices" => self.devices().iter().map(|d| format!("{}\t{}\t{}", d.name, d.prefer, d.key)).collect::<Vec<_>>().join("\n"),
            "request" => match self.newest_pending() {
                Some(r) => format!("{}\t{}\t{}", crate::localsend::js(&r["id"]), crate::localsend::js(&r["via"]), crate::localsend::js(&r["what"])),
                None => "none".into(),
            },
            "accept" | "decline" => match self.newest_pending().map(|r| crate::localsend::js(&r["id"])) {
                Some(id) => {
                    eprintln!("tessera-ui: sharing: {func} {id} (over IPC)");
                    self.answer(&id, func == "accept");
                    format!("{}ed {id}", if func == "accept" { "accept" } else { "declin" })
                }
                None => "no request is waiting".into(),
            },
            "send" => {
                let want = args.join(" ");
                match self.devices().into_iter().find(|d| d.key == want || d.name == want) {
                    Some(_) if self.sheet_paths.is_empty() => "nothing to send: open the send window with files first".into(),
                    Some(d) => {
                        let paths = self.sheet_paths.clone();
                        eprintln!("tessera-ui: sharing: send {} file(s) over {} (over IPC)", paths.len(), d.prefer);
                        self.send_to(&d, &paths);
                        format!("sending {} file(s) to {} over {}", paths.len(), d.name, d.prefer)
                    }
                    None => "no such device in the send window's list".into(),
                }
            }
            _ => "sharing: level <l> | devices | request | accept | decline | send <device>".into(),
        }
    }

    /// The newest request still asking (Accept / Decline on show)
    fn newest_pending(&self) -> Option<&Value> {
        self.share.requests.iter().rev().find(|r| r["state"] == "asking")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(sharing: Value) -> Sharing {
        let mut s = Sharing::default();
        s.cfg = json!({"sharing": sharing});
        s
    }

    #[test]
    fn one_level_drives_both_protocols() {
        // the user's config of 17 Nov: the tile's key says everyone, no LocalSend key
        let s = with(json!({"quickShare": "everyone", "quickShareHotspot": true, "quickShareFolder": ""}));
        assert_eq!(s.level(), "everyone");
        assert!(s.quick_visible() && s.local_receiving());
        assert_eq!(s.level_text(), "Everyone");
        assert!(!s.mixed());
        // an old LocalSend "everyone" no longer opens LocalSend alone
        let s = with(json!({"quickShare": "off", "receiving": "everyone"}));
        assert!(!s.quick_visible() && !s.local_receiving());
        assert_eq!(s.level_text(), "No one");
    }

    #[test]
    fn ten_minutes_end_for_both() {
        let mut s = with(json!({"quickShare": "10min"}));
        let now = tessera_core::util::now_ms();
        s.quick.everyone_until = now + 3 * 60_000 - 10;
        assert!(s.quick_visible() && s.local_receiving());
        assert_eq!(s.minutes_left(), 3);
        assert_eq!(s.level_text(), "3 min left");
        s.quick.everyone_until = now - 1;
        assert!(!s.quick_visible() && !s.local_receiving());
    }


    #[test]
    fn ipc_answers_only_the_newest_waiting_request() {
        let mut s = with(json!({"quickShare": "everyone"}));
        assert_eq!(s.sharing_ipc("accept", &[]), "no request is waiting");
        s.add_request(json!({"id": "qs:in-1", "via": "quickshare", "session": "in-1", "what": "a.jpg"}));
        s.add_request(json!({"id": "qs:in-2", "via": "quickshare", "session": "in-2", "what": "b.jpg"}));
        assert!(s.sharing_ipc("request", &[]).starts_with("qs:in-2\tquickshare"));
        assert_eq!(s.sharing_ipc("accept", &[]), "accepted qs:in-2");
        assert_eq!(s.find_request("qs:in-2").unwrap()["state"], "receiving");
        assert_eq!(s.find_request("qs:in-1").unwrap()["state"], "asking");
        assert_eq!(s.sharing_ipc("decline", &[]), "declined qs:in-1");
        assert!(s.find_request("qs:in-1").is_none());
        assert_eq!(s.sharing_ipc("decline", &[]), "no request is waiting");
    }

    #[test]
    fn ipc_sends_only_to_a_listed_device_and_never_in_demo() {
        let mut s = with(json!({"quickShare": "off"}));
        s.quick.devices = vec![("Moto edge".into(), "phone".into(), "Q9".into())];
        assert_eq!(s.sharing_ipc("send", &["Moto edge".into()]), "nothing to send: open the send window with files first");
        s.sheet_paths = vec!["/tmp/a b.bin".into()];
        assert_eq!(s.sharing_ipc("send", &["Someone else".into()]), "no such device in the send window's list");
        assert_eq!(s.sharing_ipc("send", &["Moto".into(), "edge".into()]), "sending 1 file(s) to Moto edge over quickshare");
        assert_eq!(s.quick.queued.len(), 1);
        assert_eq!(s.sharing_ipc("level", &["10min".into()]), "visible to 10min");
        assert_eq!(s.level(), "10min");
        assert_eq!(s.sharing_ipc("level", &["on".into()]), "level takes off, 10min or everyone");
        s.share.demo = Some(json!({"localsend": [], "quickshare": [], "items": []}));
        assert!(s.sharing_ipc("send", &["Moto edge".into()]).starts_with("refused"));
        assert!(s.sharing_ipc("accept", &[]).starts_with("refused"));
    }

    #[test]
    fn a_helpers_end_drops_hidden_requests_and_fails_the_shown_one() {
        let mut s = with(json!({"quickShare": "everyone"}));
        s.add_request(json!({"id": "qs:in-1", "via": "quickshare", "session": "in-1", "from": "Kai", "state": "receiving", "hidden": true}));
        s.add_request(json!({"id": "qs:in-2", "via": "quickshare", "session": "in-2", "from": "Ada"}));
        s.add_request(json!({"id": "ls:x", "via": "localsend", "session": "x", "from": "Desk", "hidden": true}));
        s.helper_ended("quickshare");
        assert!(s.find_request("qs:in-1").is_none());
        assert_eq!(s.find_request("qs:in-2").unwrap()["state"], "failed");
        // (the other protocol's are left alone)
        assert_eq!(s.find_request("ls:x").unwrap()["state"], "asking");
        assert_eq!(s.share.hidden_failed, vec!["Kai".to_string()]);
    }

    #[test]
    fn shared_text_never_goes_to_a_shared_tmp_folder() {
        assert_eq!(text_dir_from(Some("/run/user/1000".into()), "/home/a/.cache"), "/run/user/1000/tessera-share");
        assert_eq!(text_dir_from(None, "/home/a/.cache"), "/home/a/.cache/tessera-share");
        assert_eq!(text_dir_from(Some(String::new()), "/home/a/.cache"), "/home/a/.cache/tessera-share");
    }

    #[test]
    fn two_texts_in_one_second_get_two_files() {
        assert_eq!(text_file_name("/d", "Text 1", |_| false), "/d/Text 1.txt");
        let taken = ["/d/Text 1.txt".to_string(), "/d/Text 1 (2).txt".to_string()];
        assert_eq!(text_file_name("/d", "Text 1", |f| taken.iter().any(|t| t == f)), "/d/Text 1 (3).txt");
    }

    #[test]
    fn a_send_before_the_device_is_known_waits_for_it() {
        let mut s = with(json!({"quickShare": "off"}));
        s.quick_send("Q1", &["/tmp/a b.jpg".to_string()]);
        assert_eq!(s.quick.queued.len(), 1);
        // (the helper is wanted for it, and looks for the device)
        assert_eq!(s.quick_needed(), s.quick_available());
        assert!(s.sending_now());
        s.local_send("fp1", &["/tmp/c.pdf".to_string(), "/tmp/d.pdf".to_string()]);
        assert_eq!(s.local.queued, vec![("fp1".to_string(), vec!["/tmp/c.pdf".to_string(), "/tmp/d.pdf".to_string()])]);
        assert_eq!(s.local.sending_name, "c.pdf and 1 more");
        // the firewall opens for sending, Quick Share's ports too
        assert!(s.ports_need().0.contains("quickshare") && s.ports_need().0.contains("localsend"));
    }
}
