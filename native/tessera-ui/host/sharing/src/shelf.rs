//! The drop shelf (shell/services/Shelf.qml, and what modules/shelf/
//! ShelfFeed.qml tells the shelf surface): a place to park files while you
//! get to where they're going. Drop files (or text) on it, then drag them out
//! again, one at a time or all together. It holds references, not copies, and
//! keeps them across restarts until they're removed or cleared.
//! It opens:
//!   - shaking the pointer while dragging something (shelf.shake)
//!   - holding a drag at the left, right or top edge of a screen (shelf.edge)
//! and only for a real drag of files: a selection, a scrollbar, a slider, or
//! a browser's image or link dragged about never opens it. That is the job of
//! ShelfProbes.qml (invisible drop strips), which is a surface, not a
//! service: they read `shelf.probing` / `shelf.probe` from the shell state
//! and call `shelf.dragArrived [where, screen, y]` / `shelf.dragLeft [where]`
//! / `shelf.offer [screen]` (and `shelf.keep` on a drop there).
//!   - Super+Alt+D (keymap.json "shelf"), or its menu bar icon (shelf.menubar)
//! While the left button is held (`shelf.pointerDown` / `shelf.pointerUp`, from
//! the global shortcuts the Hyprland binds fire), the pointer is followed
//! (assets/scripts/pointer-follow.py); never otherwise. Moving a window (the
//! tessera-glass plugin says so: `shelf.windowMove`) doesn't count.
//!   ipc: shelf toggle | open | close | clear | zone

use crate::shelf_logic as logic;
use crate::localsend::js;
use crate::proc::{spawn, Kept};
use crate::{act, Sharing};
use serde_json::{json, Value};
use tessera_core::host::Host;

#[derive(Clone, Debug, Default)]
pub struct Mon {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct Shelf {
    /// [{ uri, path, name, kind: "image" | "folder" | "file" | "text", text? }]
    pub items: Vec<Value>,
    pub cleared: Vec<Value>,
    pub opened: bool,
    pub screen: Option<String>,
    /// left | right (which edge it sits at)
    pub side: String,
    /// the pointer's height on that screen (-1: near the top)
    pub at_y: f64,
    /// opened mid-drag: it goes again if nothing lands
    pub from_drag: bool,
    /// (the shelf surface: the pointer, or a drag, is over it)
    pub hovered: bool,
    pub zone: bool,
    pub zone_screen: Option<String>,
    // following a drag
    pub down: bool,
    pub down_at: i64,
    pub start: Option<(f64, f64)>,
    pub last: (f64, f64),
    pub dragging: bool,
    pub window_move: bool,
    pub samples: Vec<(i64, f64)>,
    pub probing: bool,
    /// { x, y, screen } just after a shake
    pub probe: Option<(f64, f64, Option<String>)>,
    pub edge_at: String,
    pub edge_screen: Option<String>,
    pub press: i64,
    pub followed: i64,
    pub stale_up_until: i64,
    pub last_move: i64,
    pub follower: Kept,
    pub monitors: Vec<Mon>,
    pub last_feed: Value,
}

impl Default for Shelf {
    fn default() -> Self {
        Self {
            items: vec![],
            cleared: vec![],
            opened: false,
            screen: None,
            side: "right".into(),
            at_y: -1.0,
            from_drag: false,
            hovered: false,
            zone: false,
            zone_screen: None,
            down: false,
            down_at: 0,
            start: None,
            last: (0.0, 0.0),
            dragging: false,
            window_move: false,
            samples: vec![],
            probing: false,
            probe: None,
            edge_at: String::new(),
            edge_screen: None,
            press: 0,
            followed: 0,
            stale_up_until: 0,
            last_move: 0,
            follower: Kept::default(),
            monitors: vec![],
            last_feed: Value::Null,
        }
    }
}

fn strings(a: &Value) -> Vec<String> {
    a.as_array().map(|a| a.iter().filter_map(|u| u.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

impl Sharing {
    fn shelf_file() -> std::path::PathBuf {
        tessera_core::util::state_dir().join("shelf.json")
    }
    pub fn shelf_paths(&self) -> Vec<String> {
        self.shelf.items.iter().filter_map(|i| i["path"].as_str().filter(|p| !p.is_empty()).map(str::to_string)).collect()
    }
    fn shelf_watching(&self) -> bool {
        self.cfg_bool("shelf.shake", true) || self.cfg_bool("shelf.edge", true)
    }
    /// Kept across restarts (references only)
    pub fn shelf_load(&mut self) {
        if let Ok(text) = std::fs::read_to_string(Self::shelf_file()) {
            if let Ok(j) = serde_json::from_str::<Value>(&text) {
                if let Some(a) = j["items"].as_array() {
                    self.shelf.items = a.iter().filter(|i| i.is_object() && (!i["uri"].as_str().unwrap_or("").is_empty() || !i["text"].as_str().unwrap_or("").is_empty())).cloned().collect();
                }
            }
        }
    }
    fn shelf_save(&mut self) {
        self.sched.set("shelf_save", 300);
    }
    pub fn shelf_write(&mut self) {
        let text = serde_json::to_string_pretty(&json!({"items": self.shelf.items})).unwrap_or_default();
        let _ = tessera_core::util::write_atomic(&Self::shelf_file(), text.as_bytes());
    }

    // ── What's on it ────────────────────────────────────────────────────
    /// Dropped: file URLs, else text (ShelfLogic.drop)
    pub fn shelf_drop(&mut self, urls: &[String], text: &str) -> bool {
        let (items, added) = logic::drop(&self.shelf.items, urls, text);
        if added == 0 {
            return false;
        }
        self.shelf.items = items;
        self.shelf_save();
        true
    }
    pub fn shelf_remove_at(&mut self, i: usize) {
        if i >= self.shelf.items.len() {
            return;
        }
        self.shelf.items.remove(i);
        self.shelf_save();
        if self.shelf.items.is_empty() && self.shelf.opened {
            self.sched.set("shelf_close_soon", 1200);
        }
    }
    pub fn shelf_clear(&mut self, h: &mut Host) {
        if self.shelf.items.is_empty() {
            return;
        }
        self.shelf.cleared = std::mem::take(&mut self.shelf.items);
        self.shelf_save();
        let n = self.shelf.cleared.len();
        self.island(
            h,
            json!({"kind": "custom", "icon": "tray", "title": "Shelf cleared", "subtitle": format!("{n} {} removed (the files themselves are untouched)", if n == 1 { "item" } else { "items" }),
                "color": crate::SECONDARY, "action": "Undo", "onAction": act("shelf.undoClear", json!([]))}),
            8000,
        );
    }
    pub fn shelf_undo_clear(&mut self, h: &mut Host) {
        let mut items = std::mem::take(&mut self.shelf.cleared);
        items.extend(std::mem::take(&mut self.shelf.items));
        self.shelf.items = items;
        self.shelf_save();
        self.shelf_show(h, "menubar", None);
    }

    // ── Showing it ──────────────────────────────────────────────────────
    fn focused_screen(&self, h: &Host) -> Option<String> {
        h.st.shell.get::<tessera_core::services::hypr::Snapshot>().map(|s| s.focused.clone()).filter(|s| !s.is_empty()).or_else(|| self.shelf.monitors.first().map(|m| m.name.clone()))
    }
    pub fn shelf_show(&mut self, h: &mut Host, how: &str, scr: Option<String>) {
        self.shelf.screen = scr.or_else(|| self.focused_screen(h));
        if how != "shake" && how != "edge" && how != "drop" {
            self.shelf.side = "right".into();
            self.shelf.at_y = -1.0;
        }
        self.shelf.from_drag = how == "shake" || how == "edge";
        self.sched.cancel("shelf_close_soon");
        self.shelf.opened = true;
    }
    pub fn shelf_hide(&mut self) {
        self.shelf.opened = false;
        self.shelf.from_drag = false;
    }
    pub fn shelf_toggle(&mut self, h: &mut Host, scr: Option<String>) {
        if self.shelf.opened {
            self.shelf_hide();
        } else {
            self.shelf_show(h, "shortcut", scr);
        }
    }

    // ── The drop zone (the island's big drop zone) ───────────────────────
    // A drag of files that asks for the shelf (a shake, an edge) gets the
    // island's big drop zone first: keep the files on the shelf, or send them
    // to a phone. Dropped on Keep, it shrinks to the shelf's card. It goes by
    // itself too: a file drag swallows the button's release (which ends it,
    // below) until the next click, so it also goes a moment after the drag
    // leaves it (1.5 s) or if the drag never comes over it (6 s); and a click
    // on it, or its close button, puts it away.
    pub fn shelf_offer(&mut self, h: &mut Host, scr: Option<String>) {
        self.shelf.zone_screen = scr.or_else(|| self.focused_screen(h));
        self.sched.cancel("shelf_zone_end");
        self.shelf.zone = true;
        self.shelf_zone_hover(false, Some(6000));
    }
    pub fn shelf_zone_hover(&mut self, over: bool, ms: Option<u64>) {
        if over {
            self.sched.cancel("shelf_zone_idle");
        } else if self.shelf.zone {
            self.sched.set("shelf_zone_idle", ms.unwrap_or(1500));
        }
    }
    pub fn shelf_keep(&mut self, h: &mut Host, urls: &[String], text: &str) -> bool {
        let ok = self.shelf_drop(urls, text);
        self.shelf.zone = false;
        let z = self.shelf.zone_screen.clone();
        self.shelf_show(h, "drop", z);
        ok
    }
    pub fn shelf_send_on(&mut self, h: &mut Host, urls: &[String]) -> bool {
        let paths: Vec<String> = urls.iter().filter(|u| u.starts_with("file://")).map(|u| tessera_core::services::mpris::percent_decode(&u[7..])).collect();
        self.shelf.zone = false;
        if !paths.is_empty() {
            let z = self.shelf.zone_screen.clone();
            self.panels_share(h, paths.clone(), z, "");
        }
        !paths.is_empty()
    }

    fn mon_named(&self, name: &Option<String>) -> Option<Mon> {
        let n = name.as_ref()?;
        self.shelf.monitors.iter().find(|m| &m.name == n).cloned()
    }

    pub fn shelf_drag_arrived(&mut self, h: &mut Host, wher: &str, scr: Option<String>) {
        if self.shelf.opened || self.shelf.zone {
            return;
        }
        if wher == "shake" {
            let m = self.mon_named(&scr);
            let probe = self.shelf.probe.clone();
            self.shelf.side = match (&probe, &m) {
                (Some(p), Some(m)) if p.0 > m.x + m.w / 2.0 => "right",
                _ => "left",
            }
            .into();
            self.shelf.at_y = match (&probe, &m) {
                (Some(p), Some(m)) => p.1 - m.y,
                _ => -1.0,
            };
            self.shelf_offer(h, scr);
            self.shelf.probe = None;
        } else {
            self.shelf.edge_at = wher.into();
            self.shelf.edge_screen = scr;
            self.sched.set("shelf_edge_hold", 350);
        }
    }
    pub fn shelf_drag_left(&mut self, wher: &str) {
        if wher != "shake" {
            self.sched.cancel("shelf_edge_hold");
        }
    }
    fn shelf_edge_held(&mut self, h: &mut Host) {
        if self.shelf.opened || self.shelf.zone || !self.shelf.down {
            return;
        }
        let m = self.mon_named(&self.shelf.edge_screen.clone());
        // (from the top: the side nearer the pointer)
        self.shelf.side = if self.shelf.edge_at == "top" {
            match &m {
                Some(m) if self.shelf.last.0 > m.x + m.w / 2.0 => "right".into(),
                _ => "left".into(),
            }
        } else {
            self.shelf.edge_at.clone()
        };
        self.shelf.at_y = self.shelf.last.1 - m.map(|m| m.y).unwrap_or(0.0);
        let s = self.shelf.edge_screen.clone();
        self.shelf_offer(h, s);
    }

    // (each press is numbered: a follower still ending from the last one,
    // whose release went missing, mustn't end this one)
    pub fn shelf_pointer_down(&mut self, h: &mut Host) {
        if !self.shelf_watching() {
            return;
        }
        let now = tessera_core::util::now_ms();
        self.shelf.press += 1;
        // A drag of files swallows the button's release; Hyprland hands it
        // over at the next press instead. A press arriving while the last is
        // still "down" ignores the release that follows it at once
        self.shelf.stale_up_until = if self.shelf.down { now + 150 } else { 0 };
        // (fresh drop areas for each press: ones left from a press whose
        // release went missing don't hear a new drag arrive)
        self.shelf.probing = false;
        self.sched.cancel("shelf_edge_hold");
        self.shelf.down = true;
        self.shelf.down_at = now;
        self.shelf.start = None;
        self.shelf.dragging = false;
        self.shelf.window_move = false;
        self.shelf.samples.clear();
        self.shelf.probe = None;
        self.shelf.last_move = now;
        self.sched.set("shelf_follow", 120);
        self.shelf_fetch_monitors(h);
    }
    pub fn shelf_pointer_up(&mut self) {
        let now = tessera_core::util::now_ms();
        if now < self.shelf.stale_up_until {
            self.shelf.stale_up_until = 0;
            return;
        }
        if !self.shelf.down {
            return;
        }
        self.shelf.down = false;
        self.shelf.probing = false;
        if self.shelf.zone {
            self.sched.set("shelf_zone_end", 500);
        }
        self.shelf.probe = None;
        self.sched.cancel("shelf_edge_hold");
        self.sched.cancel("shelf_follow");
        self.shelf.follower.signal(libc::SIGTERM);
        if self.shelf.opened && self.shelf.from_drag && self.shelf.items.is_empty() {
            self.sched.set("shelf_close_soon", 1200);
        } else if self.shelf.opened && self.shelf.from_drag {
            self.shelf.from_drag = false; // (something landed: it stays)
        }
    }
    /// The glass plugin says a window is being moved ("tessera-drag" /
    /// "irice-drag", a plugin built before the rename)
    pub fn shelf_window_move(&mut self) {
        if self.shelf.down {
            self.shelf.window_move = true;
            self.shelf.probing = false;
            self.shelf.follower.signal(libc::SIGTERM);
        }
    }
    fn shelf_follow_due(&mut self, h: &mut Host) {
        // Most clicks are over before this: only a held button is followed
        if self.shelf.down && !self.shelf.window_move {
            self.shelf.probing = true;
            self.shelf.followed = self.shelf.press;
            if !self.shelf.follower.running {
                // (one at a time: a second would be queued)
                let cmd = vec!["python3".to_string(), "-I".into(), self.script("pointer-follow.py")];
                let gen = crate::proc::next_gen();
                if let Some(k) = spawn(h.sender("Sharing"), "pointer", gen, &cmd, &[], false, true) {
                    self.shelf.follower = k;
                    // A release can go missing (a surface appearing under the
                    // pointer): a pointer that hasn't moved for 4 s, or a
                    // follower that ended, counts as the button coming up
                    self.sched.set("shelf_lock", 1000);
                }
            }
        }
    }
    pub fn shelf_pointer_line(&mut self, h: &mut Host, gen: u64, line: &str) {
        if gen != self.shelf.follower.gen {
            return;
        }
        let Some((a, b)) = line.trim().split_once(',') else { return };
        if let (Ok(x), Ok(y)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            self.shelf_moved(h, x, y);
        }
    }
    pub fn shelf_pointer_exit(&mut self, gen: u64) {
        if gen == self.shelf.follower.gen {
            self.shelf.follower.ended();
        }
    }
    fn shelf_moved(&mut self, _h: &mut Host, x: f64, y: f64) {
        let now = tessera_core::util::now_ms();
        if self.shelf.start.is_none() || (x, y) != self.shelf.last {
            self.shelf.last_move = now;
        }
        self.shelf.last = (x, y);
        let start = *self.shelf.start.get_or_insert((x, y));
        if !self.shelf.dragging && (x - start.0).hypot(y - start.1) > 16.0 {
            self.shelf.dragging = true;
        }
        if !self.shelf.dragging || self.shelf.window_move {
            return;
        }
        let scr = self.shelf.monitors.iter().find(|m| x >= m.x && x < m.x + m.w && y >= m.y && y < m.y + m.h).map(|m| m.name.clone());
        // Shaken: the pointer turned back and forth quickly. Whether it's
        // carrying something is for the drop area put under it to tell
        // (ShelfProbes: dragArrived)
        self.shelf.samples.retain(|s| now - s.0 < 700);
        self.shelf.samples.push((now, x));
        if self.cfg_bool("shelf.shake", true) && !self.shelf.opened && logic::shaken(&self.shelf.samples, 40.0) {
            self.shelf.probe = Some((x, y, scr));
            self.sched.set("shelf_probe_end", 900);
            self.shelf.samples.clear();
        }
        // (held at an edge: the edge strips tell, ShelfProbes)
    }
    pub fn shelf_fetch_monitors(&mut self, h: &mut Host) {
        let to = h.sender("Sharing");
        std::thread::spawn(move || {
            let Ok(text) = tessera_core::services::hypr::request("j/monitors") else { return };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { return };
            let mons: Vec<Mon> = v
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|m| {
                            let scale = m["scale"].as_f64().filter(|s| *s > 0.0).unwrap_or(1.0);
                            Mon { name: js(&m["name"]), x: m["x"].as_f64().unwrap_or(0.0), y: m["y"].as_f64().unwrap_or(0.0), w: m["width"].as_f64().unwrap_or(0.0) / scale, h: m["height"].as_f64().unwrap_or(0.0) / scale }
                        })
                        .collect()
                })
                .unwrap_or_default();
            to(Box::new(crate::M::Monitors(mons)));
        });
    }

    /// The shelf's deadlines
    pub fn shelf_due(&mut self, h: &mut Host, name: &str) {
        match name {
            "shelf_save" => self.shelf_write(),
            "shelf_close_soon" => {
                if !self.shelf.hovered && self.shelf.items.is_empty() {
                    self.shelf_hide();
                }
            }
            "shelf_probe_end" => self.shelf.probe = None,
            "shelf_edge_hold" => self.shelf_edge_held(h),
            "shelf_zone_idle" | "shelf_zone_end" => self.shelf.zone = false,
            "shelf_follow" => self.shelf_follow_due(h),
            "shelf_lock" => {
                if self.shelf.follower.running {
                    if self.shelf.down && tessera_core::util::now_ms() - self.shelf.last_move > 4000 {
                        self.shelf_pointer_up();
                    }
                    if self.shelf.follower.running {
                        self.sched.set("shelf_lock", 1000);
                    }
                } else if self.shelf.down && self.shelf.followed == self.shelf.press && self.shelf.probing {
                    // (a follower that ended counts as the button coming up)
                    self.shelf_pointer_up();
                }
            }
            _ => {}
        }
    }

    /// What the shelf surface is told (ShelfFeed.qml's feed)
    pub fn shelf_feed(&self) -> Value {
        json!({
            "opened": self.shelf.opened,
            "items": self.shelf.items.iter().map(|i| json!({"uri": js(&i["uri"]), "path": js(&i["path"]), "name": js(&i["name"]),
                "kind": if i["kind"].is_null() { "file".to_string() } else { js(&i["kind"]) }, "text": js(&i["text"])})).collect::<Vec<_>>(),
            "screen": self.shelf.screen.clone().unwrap_or_default(),
            "side": self.shelf.side,
            "atY": self.shelf.at_y,
            "zone": self.shelf.zone,
            "zoneScreen": self.shelf.zone_screen.clone().unwrap_or_default(),
        })
    }

    /// What the shelf surface asks (`shell::ask("shelf", [what, ...])`) and the
    /// other shelf calls (`shelf.toggle`...)
    pub fn shelf_call(&mut self, h: &mut Host, what: &str, a: &[Value]) {
        let arg = |i: usize| a.get(i).cloned().unwrap_or(Value::Null);
        match what {
            "drop" => {
                self.shelf_drop(&strings(&arg(0)), arg(1).as_str().unwrap_or(""));
            }
            "removeAt" => {
                if let Some(i) = arg(0).as_u64() {
                    self.shelf_remove_at(i as usize);
                }
            }
            "clear" => self.shelf_clear(h),
            "hide" => self.shelf_hide(),
            "share" => {
                let paths = self.shelf_paths();
                if !paths.is_empty() {
                    let s = self.shelf.screen.clone();
                    self.panels_share(h, paths, s, "");
                }
            }
            "hovered" => self.shelf.hovered = arg(0).as_bool() == Some(true),
            "zoneHover" => self.shelf_zone_hover(arg(0).as_bool() == Some(true), None),
            "cancelZone" => self.shelf.zone = false,
            "keep" => {
                self.shelf_keep(h, &strings(&arg(0)), arg(1).as_str().unwrap_or(""));
            }
            "sendOn" => {
                self.shelf_send_on(h, &strings(&arg(0)));
            }
            // the other hosts' and the probes' calls
            "toggle" => self.shelf_toggle(h, arg(0).as_str().map(str::to_string)),
            "show" => self.shelf_show(h, arg(0).as_str().unwrap_or("shortcut"), arg(1).as_str().map(str::to_string)),
            "open" => self.shelf_show(h, "shortcut", None),
            "close" => self.shelf_hide(),
            "undoClear" => self.shelf_undo_clear(h),
            "offer" => self.shelf_offer(h, arg(0).as_str().map(str::to_string)),
            "dragArrived" => self.shelf_drag_arrived(h, arg(0).as_str().unwrap_or(""), arg(1).as_str().map(str::to_string)),
            "dragLeft" => self.shelf_drag_left(arg(0).as_str().unwrap_or("")),
            "pointerDown" => self.shelf_pointer_down(h),
            "pointerUp" => self.shelf_pointer_up(),
            "windowMove" => self.shelf_window_move(),
            _ => eprintln!("tessera-ui: host: no shelf call {what}"),
        }
    }

    pub fn shelf_ipc(&mut self, h: &mut Host, func: &str) -> Option<String> {
        match func {
            "toggle" => self.shelf_toggle(h, None),
            "zone" => self.shelf_offer(h, None),
            "open" => self.shelf_show(h, "shortcut", None),
            "close" => self.shelf_hide(),
            "clear" => self.shelf_clear(h),
            _ => {}
        }
        None
    }
}
