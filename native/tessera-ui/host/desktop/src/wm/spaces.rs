//! Spaces.qml: spaces per display (blocks of 100 ids, SpaceMap.js), the moves
//! the Overview, Mission Control and the menu bar ask for (go, newSpace,
//! sendWindow, sendWindowToDisplay, moveSpaceTo, move), the blocks pushed to
//! Hyprland as workspace rules (spaces.lua) and the bookkeeping when a display
//! comes or goes. The views get {bases, labels, count} from here
//! (RustWindows.qml's sendSpaces) and the menu bar's state `spaces`.

use super::hx::{self, Fresh};
use super::spacemap::{self as map, Disp, Mon, Op, Plan, Ws};
use super::Io;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tessera_core::host::Host;
use tessera_core::services::hypr::Snapshot;

#[derive(Default)]
pub struct Spaces {
    /// connected displays: connector -> base
    pub bases: BTreeMap<String, i32>,
    /// every display we know: base -> short name
    pub labels: BTreeMap<i32, String>,
    /// connected displays: connector -> identity key
    keys: BTreeMap<String, String>,
    show_on: Vec<String>,
    known: Vec<i32>,
    started: bool,
    chain: u8,
    pushed: bool,
    quiet_until: Option<Instant>,
    reading: bool,
    sent: String,
}

fn q(s: &str) -> String {
    hx::q(s)
}

impl Spaces {
    pub fn base_of(&self, name: &str) -> i32 {
        self.bases.get(name).copied().unwrap_or(0)
    }
    /// The connected display a space belongs to ("" when it isn't connected)
    pub fn owner_name(&self, id: i32) -> String {
        let b = map::base_of_id(id);
        self.bases.iter().find(|(_, v)| **v == b).map(|(n, _)| n.clone()).unwrap_or_default()
    }
    /// Every regular space and the display it's on
    fn workspaces(h: &Host) -> Vec<(i32, String)> {
        let mut out: Vec<(i32, String)> = Vec::new();
        let Some(s) = h.st.shell.get::<Snapshot>() else { return out };
        for (m, id) in &s.active {
            if *id > 0 && !out.iter().any(|(i, _)| i == id) {
                out.push((*id, m.clone()));
            }
        }
        for w in &s.workspaces {
            if w.id > 0 && !out.iter().any(|(i, _)| *i == w.id) {
                out.push((w.id, w.monitor.clone()));
            }
        }
        out
    }
    fn exists(h: &Host, id: i32) -> bool {
        Self::workspaces(h).iter().any(|(i, _)| *i == id)
    }
    fn active_of(h: &Host, name: &str) -> i32 {
        h.st.shell.get::<Snapshot>().and_then(|s| s.active.get(name).copied()).unwrap_or(0)
    }
    fn next_free_on(&self, h: &Host, name: &str) -> i32 {
        let ids: Vec<i32> = Self::workspaces(h).iter().map(|w| w.0).collect();
        map::next_free(self.base_of(name), &ids)
    }

    // ── quiet: moves made from the Overview never take you anywhere ─────────
    fn hush(&mut self, ms: u64) {
        let until = Instant::now() + Duration::from_millis(ms);
        self.quiet_until = Some(self.quiet_until.map_or(until, |q| q.max(until)));
    }
    pub fn quiet(&self) -> bool {
        self.quiet_until.is_some_and(|q| Instant::now() < q)
    }
    /// Lua that leaves the pointer and the focused display where they were
    fn keeping(&mut self, lua: &str) -> String {
        self.hush(1200);
        format!("local c = hl.get_cursor_pos(); local m = hl.get_active_monitor()\n{lua}if m then hl.dispatch(hl.dsp.focus({{ monitor = m.name }})) end\nif c then hl.dispatch(hl.dsp.cursor.move({{ x = c.x, y = c.y }})) end\n")
    }

    // ── actions ──────────────────────────────────────────────────────────────
    /// Go to a space (made on its own display if it doesn't exist yet)
    pub fn go(&mut self, h: &Host, id: i32) {
        let owner = self.owner_name(id);
        if !Self::exists(h, id) && !owner.is_empty() {
            hx::eval(format!("hl.dispatch(hl.dsp.focus({{ monitor = {} }}))\nhl.dispatch(hl.dsp.focus({{ workspace = {id} }}))\n", q(&owner)));
        } else {
            hx::dispatch(format!("hl.dsp.focus({{ workspace = {id} }})"));
        }
    }
    /// A new empty space on a display, and go there
    pub fn new_space(&mut self, h: &Host, name: &str) {
        let id = self.next_free_on(h, name);
        self.go(h, id);
    }
    /// A window to a space (on that space's own display, even if it doesn't exist yet)
    pub fn send_window(&mut self, h: &Host, address: &str, id: i32) {
        self.hush(1200);
        let mut lua = format!("hl.dispatch(hl.dsp.window.move({{ workspace = {id}, follow = false, window = {} }}))\n", q(&format!("address:{}", hx::addr(address))));
        let owner = self.owner_name(id);
        if !Self::exists(h, id) && !owner.is_empty() {
            let more = format!("{lua}hl.dispatch(hl.dsp.workspace.move({{ workspace = {id}, monitor = {} }}))\n", q(&owner));
            lua = self.keeping(&more);
        }
        hx::eval(lua);
    }
    /// A window to another display: onto the space it shows
    pub fn send_window_to_display(&mut self, h: &Host, address: &str, name: &str) {
        if !self.bases.contains_key(name) {
            return;
        }
        let a = Self::active_of(h, name);
        let id = if a > 0 { a } else { self.next_free_on(h, name) };
        self.send_window(h, address, id);
    }
    /// A whole space to another display: it becomes that display's next number there
    pub fn move_space_to(&mut self, h: &Host, id: i32, name: &str) {
        if id <= 0 || !self.bases.contains_key(name) {
            return;
        }
        let to = self.next_free_on(h, name);
        let lua = self.keeping(&format!("hl.dispatch(hl.dsp.workspace.change_id({{ workspace = {id}, id = {to} }}))\nhl.dispatch(hl.dsp.workspace.move({{ workspace = {to}, monitor = {} }}))\n", q(name)));
        hx::eval(lua);
    }
    /// Reordering (drag a space): the dropped space takes that one's place and
    /// the ones between shift by one; the windows move, through a hidden
    /// holding space, in one Lua chunk. If you were on a space that moved, you follow it.
    pub fn reorder(&mut self, h: &Host, order: &[i32], from: i32, to: i32) {
        let list: Vec<i32> = order.iter().copied().filter(|i| *i > 0).collect();
        let (Some(a), Some(b)) = (list.iter().position(|i| *i == from), list.iter().position(|i| *i == to)) else { return };
        if a == b {
            return;
        }
        let mut next = list.clone();
        next.remove(a);
        next.insert(b, from);
        let (lo, hi) = (a.min(b), a.max(b));
        let Some(win) = h.st.shell.get::<tessera_core::services::clients::Windows>() else { return };
        let mut moves: Vec<(String, i32)> = Vec::new();
        for i in lo..=hi {
            for c in win.clients.iter().filter(|c| c.workspace.id == next[i]) {
                moves.push((hx::addr(&c.address), list[i]));
            }
        }
        if moves.is_empty() {
            return;
        }
        self.hush(1200);
        let step = |addr: &str, ws: &str| format!("hl.dispatch(hl.dsp.window.move({{ workspace = {ws}, follow = false, window = \"address:{addr}\" }}))\n");
        let mut lua = String::new();
        for (a, _) in &moves {
            lua += &step(a, "\"special:tessera-reorder\"");
        }
        for (a, ws) in &moves {
            lua += &step(a, &ws.to_string());
        }
        let here = win.focused_monitor().map(|m| m.active.id).unwrap_or(0);
        if let Some(i) = next.iter().position(|x| *x == here) {
            if here > 0 && i >= lo && i <= hi && list[i] != here {
                lua += &format!("hl.dispatch(hl.dsp.focus({{ workspace = {} }}))\n", list[i]);
            }
        }
        hx::eval(lua);
    }

    // ── keeping track of displays ────────────────────────────────────────────
    /// A display came (back), as its connect effect hands over: it shows the
    /// space it showed before
    pub fn arrive(&mut self, io: &mut Io, name: &str) {
        if !self.show_on.iter().any(|n| n == name) {
            self.show_on.push(name.to_string());
        }
        io.arm("spaces.reconcile", 1);
    }
    pub fn reconcile(&mut self, io: &mut Io, ms: u64) {
        io.arm("spaces.reconcile", ms);
    }
    pub fn tick_reconcile(&mut self, io: &mut Io) {
        if self.reading {
            io.arm("spaces.reconcile", 250);
            return;
        }
        self.reading = true;
        io.fetch_then(|wm, h, io, f| {
            wm.spaces.reading = false;
            wm.spaces.apply(h, io, f);
        });
    }

    fn apply(&mut self, h: &mut Host, io: &mut Io, f: Fresh) {
        if f.monitors.is_empty() {
            return;
        }
        let sandbox = tessera_core::env::sandboxed();
        let ignored = |n: &str| n.is_empty() || n.starts_with("HEADLESS") || (n.starts_with("TESSERA-") && !sandbox);
        let mons: Vec<Mon> = f
            .monitors
            .iter()
            .filter(|m| m["disabled"] != true && !ignored(m["name"].as_str().unwrap_or("")))
            .map(|m| Mon {
                name: m["name"].as_str().unwrap_or("").into(),
                description: m["description"].as_str().unwrap_or("").into(),
                model: m["model"].as_str().unwrap_or("").into(),
                x: m["x"].as_i64().unwrap_or(0) as i32,
                y: m["y"].as_i64().unwrap_or(0) as i32,
                active: m["activeWorkspace"]["id"].as_i64().unwrap_or(0) as i32,
            })
            .collect();
        if mons.is_empty() {
            return;
        }
        let store = hx::state_get("spaces");
        let before: BTreeMap<String, Disp> = serde_json::from_value(store["displays"].clone()).unwrap_or_default();
        let displays = map::assign(&before, &mons, sandbox);
        let next_bases = map::bases_by_name(&displays, &mons);
        let next_keys: BTreeMap<String, String> = mons.iter().map(|m| (m.name.clone(), map::key_of(m))).collect();
        let next_labels: BTreeMap<i32, String> = displays.values().map(|d| (d.base, d.label.clone())).collect();
        let changed = next_bases != self.bases || displays != before;
        let moved = changed || next_labels != self.labels;
        self.bases = next_bases.clone();
        self.keys = next_keys.clone();
        self.labels = next_labels;
        if changed || !self.pushed {
            self.push(&displays);
        }

        let ws: Vec<Ws> = f
            .workspaces
            .iter()
            .filter(|w| w["id"].as_i64().unwrap_or(0) > 0)
            .map(|w| Ws { id: w["id"].as_i64().unwrap_or(0) as i32, monitor: w["monitor"].as_str().unwrap_or("").into(), windows: w["windows"].as_i64().unwrap_or(0) })
            .collect();
        let active: BTreeMap<String, i32> = mons.iter().map(|m| (m.name.clone(), m.active)).collect();
        let known: Vec<i32> = if self.started { self.known.clone() } else { ws.iter().map(|w| w.id).collect() };
        let migrated = store["migrated"] == true;
        let ops = map::plan(&Plan { bases: &next_bases, workspaces: &ws, active: &active, known: &known, migrated, returns: false });
        let shown_before = store["shown"].clone();
        let show: Vec<String> = self.show_on.iter().filter(|n| next_bases.contains_key(*n)).cloned().collect();
        self.show_on.clear();

        let mut lua = String::new();
        let mut ids: Vec<i32> = ws.iter().map(|w| w.id).collect();
        for op in &ops {
            match op {
                Op::Renumber { from, to } => {
                    lua += &format!("hl.dispatch(hl.dsp.workspace.change_id({{ workspace = {from}, id = {to} }}))\n");
                    ids.retain(|i| i != from);
                    ids.push(*to);
                }
                Op::Move { id, monitor } => lua += &format!("hl.dispatch(hl.dsp.workspace.move({{ workspace = {id}, monitor = {} }}))\n", q(monitor)),
            }
        }
        // A display that came back shows what it showed before (if that space is still there)
        for n in &show {
            let id = next_keys.get(n).and_then(|k| shown_before[k].as_i64()).unwrap_or(0) as i32;
            if id > 0 && map::base_of_id(id) == next_bases[n] && ids.contains(&id) {
                lua += &format!("hl.dispatch(hl.dsp.focus({{ monitor = {} }}))\nhl.dispatch(hl.dsp.focus({{ workspace = {id} }}))\n", q(n));
            }
        }
        self.known = ids;
        self.started = true;
        let mut next = if store.is_object() { store.clone() } else { json!({}) };
        next["displays"] = serde_json::to_value(&displays).unwrap_or(Value::Null);
        next["migrated"] = json!(true);
        if next != store {
            hx::state_set("spaces", next);
        }
        if !lua.is_empty() {
            let wrapped = self.keeping(&lua);
            hx::eval(wrapped);
            // (what Hyprland does in turn: a display left without a space gets one)
            if self.chain < 3 {
                self.chain += 1;
                io.arm("spaces.reconcile", 600);
            }
        } else {
            self.chain = 0;
        }
        self.note_shown(&f);
        if moved {
            self.tell(h);
        }
    }

    /// The blocks to Hyprland: every display we know, by identity, as its
    /// workspace rules (now, and in spaces.lua for the next login), and the
    /// connected ones by connector for the keybinds
    pub fn push(&mut self, displays: &BTreeMap<String, Disp>) {
        let known = if displays.is_empty() { serde_json::from_value(hx::state_get("spaces")["displays"].clone()).unwrap_or_default() } else { displays.clone() };
        let mut b = String::from("TESSERA_SPACE_BASES = {");
        for (k, d) in &known {
            b += &format!(" [{}] = {},", q(k), d.base);
        }
        b += " }\n";
        let mut t = String::from("TESSERA_SPACES = {");
        for (n, base) in &self.bases {
            t += &format!(" [{}] = {base},", q(n));
        }
        t += " }\n";
        hx::eval(format!("{b}{t}if TESSERA_SPACE_RULES then TESSERA_SPACE_RULES(TESSERA_SPACE_BASES) end\n"));
        if !tessera_core::env::sandboxed() || !known.is_empty() {
            let text = format!("-- Generated by Tessera (shell/services/Spaces.qml): which display each block of spaces\n-- belongs to. Loaded by hypr/tessera/keybinds.lua; changes here are overwritten.\n{b}");
            let path = hx::config_dir().join("spaces.lua");
            std::thread::spawn(move || {
                let _ = tessera_core::util::write_atomic(&path, text.as_bytes());
            });
        }
        self.pushed = true;
    }
    pub fn rules_again(&mut self) {
        let d = BTreeMap::new();
        self.push(&d);
    }

    /// The space each display shows (its own ones only), kept for when it's
    /// unplugged and comes back
    fn note_shown(&mut self, f: &Fresh) {
        let store = hx::state_get("spaces");
        let mut shown = if store["shown"].is_object() { store["shown"].clone() } else { json!({}) };
        let mut changed = false;
        for m in &f.monitors {
            let name = m["name"].as_str().unwrap_or("");
            let id = m["activeWorkspace"]["id"].as_i64().unwrap_or(0) as i32;
            if let (Some(k), Some(base)) = (self.keys.get(name), self.bases.get(name)) {
                if id > 0 && map::base_of_id(id) == *base && shown[k] != json!(id) {
                    shown[k] = json!(id);
                    changed = true;
                }
            }
        }
        if changed {
            let mut next = if store.is_object() { store } else { json!({}) };
            next["shown"] = shown;
            hx::state_set("spaces", next);
        }
    }
    pub fn tick_shown(&mut self, io: &mut Io) {
        io.fetch_then(|wm, _h, _io, f| wm.spaces.note_shown(&f));
    }

    /// Hyprland's events (Spaces.qml's Connections)
    pub fn event(&mut self, io: &mut Io, name: &str) {
        match name {
            "configreloaded" => self.rules_again(), // (a reload starts Hyprland's Lua afresh)
            "workspacev2" | "focusedmonv2" => io.arm("spaces.shown", 150),
            "monitoraddedv2" | "monitorremovedv2" | "createworkspacev2" | "moveworkspacev2" => self.reconcile(io, 250),
            _ => {}
        }
    }

    // ── what the views get ───────────────────────────────────────────────────
    pub fn json(&self, count: i64) -> Value {
        let labels: serde_json::Map<String, Value> = self.labels.iter().map(|(b, l)| (b.to_string(), json!(l))).collect();
        json!({"bases": self.bases, "labels": labels, "count": count})
    }
    /// To the views and the menu bar's state, when it changed
    pub fn tell(&mut self, h: &mut Host) {
        let count = h.cfg()["menubar"]["workspaceCount"].as_i64().unwrap_or(5);
        let v = self.json(count);
        h.set_state("spaces", json!({"bases": v["bases"], "labels": v["labels"]}));
        self.sent = v.to_string();
        for n in ["overview", "missioncontrol", "switcher"] {
            h.tell(n, json!({"spaces": v}));
        }
    }
    /// Said again (a surface started drawing)
    pub fn resend(&mut self, h: &mut Host) {
        if !self.bases.is_empty() {
            self.tell(h);
        }
    }
}
