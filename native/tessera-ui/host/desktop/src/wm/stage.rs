//! StageManager.qml: each space shows one stage (a group of windows) at a
//! time; the others wait as thumbnails in the strip. A new app opens in a new
//! stage, more windows of the same app join it. Hidden stages park their
//! windows on the special workspace "stage"; focusing a parked window brings
//! its stage forward. The waiting stages go to the stagemanager surface
//! (RustWindows.qml's sendStages), its clicks come back as wm stage.show /
//! stage.merge.

use super::hx;
use super::Io;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tessera_core::host::Host;
use tessera_core::services::clients::{Client, Windows};

const PARK: &str = "special:stage";

#[derive(Clone, Debug, PartialEq)]
struct St {
    id: i32,
    /// addresses without "0x", as Hyprland's events send them
    wins: Vec<String>,
}

pub enum To {
    Space(i32),
    Park,
}

#[derive(Default)]
pub struct Stage {
    enabled: bool,
    stages: BTreeMap<i32, Vec<St>>,
    active: BTreeMap<i32, Option<i32>>,
    next_id: i32,
    /// our own moves shift focus around; ignore focus events until they settle
    quiet_until: Option<Instant>,
    pending_open: String,
    sent: String,
}

fn class_of<'a>(w: &'a Windows, addr: &str) -> &'a str {
    let k = hx::key(addr);
    w.clients.iter().find(|c| hx::key(&c.address) == k).map(|c| c.class.as_str()).unwrap_or("")
}

impl Stage {
    pub fn new() -> Self {
        Self { next_id: 1, ..Default::default() }
    }

    fn quiet(&self) -> bool {
        self.quiet_until.is_some_and(|q| Instant::now() < q)
    }

    fn move_win(&mut self, addr: &str, to: To) {
        self.quiet_until = Some(Instant::now() + Duration::from_millis(500));
        let ws = match to {
            To::Space(i) => i.to_string(),
            To::Park => hx::q(PARK),
        };
        hx::window_action(&format!("0x{}", hx::key(addr)), "move", &[("workspace", ws), ("follow", "false".into())]);
    }

    /// Stages for a space, other than the one shown: what the strip lists
    fn waiting(&self, ws: i32) -> Vec<&St> {
        let act = self.active.get(&ws).copied().flatten();
        self.stages.get(&ws).map(|l| l.iter().filter(|s| Some(s.id) != act).collect()).unwrap_or_default()
    }

    /// {"<ws>": [{id, wins}]}
    fn waiting_json(&self) -> Value {
        let mut out = serde_json::Map::new();
        for ws in self.stages.keys() {
            out.insert(ws.to_string(), Value::Array(self.waiting(*ws).iter().map(|s| json!({"id": s.id, "wins": s.wins})).collect()));
        }
        Value::Object(out)
    }

    pub fn publish(&mut self, h: &mut Host) {
        let w = self.waiting_json();
        let s = w.to_string();
        if s != self.sent {
            self.sent = s;
            h.tell("stagemanager", json!({"stage": {"waiting": w}}));
        }
    }
    pub fn resend(&mut self, h: &mut Host) {
        self.sent.clear();
        self.publish(h);
    }

    /// Show stage `id` on space `ws`; park the current one
    pub fn show(&mut self, h: &mut Host, ws: i32, id: i32, focus: Option<&str>) {
        let list = self.stages.get(&ws).cloned().unwrap_or_default();
        let Some(target) = list.iter().find(|s| s.id == id).cloned() else { return };
        let act = self.active.get(&ws).copied().flatten();
        if let Some(cur) = list.iter().find(|s| Some(s.id) == act) {
            if cur.id != id {
                for a in cur.wins.clone() {
                    self.move_win(&a, To::Park);
                }
            }
        }
        for a in &target.wins {
            self.move_win(a, To::Space(ws));
        }
        self.active.insert(ws, Some(id));
        let mut reordered = vec![target.clone()];
        reordered.extend(list.into_iter().filter(|s| s.id != id));
        self.stages.insert(ws, reordered);
        if let Some(f) = focus.map(String::from).or_else(|| target.wins.last().cloned()) {
            hx::focus_window(&f);
        }
        self.publish(h);
    }

    /// Bring another stage's windows into the one shown (Shift+click in the strip)
    pub fn merge(&mut self, h: &mut Host, ws: i32, id: i32) {
        let list = self.stages.get(&ws).cloned().unwrap_or_default();
        let act = self.active.get(&ws).copied().flatten();
        let (Some(from), Some(into)) = (list.iter().find(|s| s.id == id).cloned(), list.iter().find(|s| Some(s.id) == act).cloned()) else {
            return self.show(h, ws, id, None);
        };
        for a in &from.wins {
            self.move_win(a, To::Space(ws));
        }
        let mut next = list;
        if let Some(i) = next.iter_mut().find(|s| s.id == into.id) {
            i.wins.extend(from.wins.clone());
        }
        next.retain(|s| s.id != from.id);
        self.stages.insert(ws, next);
        self.publish(h);
    }

    fn ws_of_stage(&self, addr: &str) -> Option<i32> {
        self.stages.iter().find(|(_, l)| l.iter().any(|s| s.wins.iter().any(|a| a == addr))).map(|(w, _)| *w)
    }

    /// Bring stages in line with the windows that exist now
    pub fn reconcile(&mut self, h: &mut Host, opened: &str) {
        if !self.enabled {
            return;
        }
        let Some(win) = h.st.shell.get::<Windows>().cloned() else { return };
        let tops: &Vec<Client> = &win.clients;
        let alive: std::collections::HashSet<String> = tops.iter().map(|t| hx::key(&t.address)).collect();
        let mut next: BTreeMap<i32, Vec<St>> = BTreeMap::new();
        for (ws, list) in &self.stages {
            let l: Vec<St> = list.iter().map(|s| St { id: s.id, wins: s.wins.iter().filter(|a| alive.contains(*a)).cloned().collect() }).filter(|s| !s.wins.is_empty()).collect();
            next.insert(*ws, l);
        }
        let mut known: BTreeMap<String, i32> = BTreeMap::new();
        for (ws, list) in &next {
            for s in list {
                for a in &s.wins {
                    known.insert(a.clone(), *ws);
                }
            }
        }
        let mut act = self.active.clone();
        let mut switch_to: Option<(i32, i32)> = None; // (space, the stage it replaced)
        let opened = hx::key(opened);

        for t in tops {
            let k = hx::key(&t.address);
            let (wid, wname) = (t.workspace.id, t.workspace.name.as_str());
            let parked = wname == PARK;
            if wid < 0 && !parked {
                continue; // other special workspaces (minimized, scratchpads)
            }
            let home = known.get(&k).copied();
            if parked {
                if home.is_none() {
                    // Parked before a shell restart: give it a stage on the focused space
                    let fw = win.focused_monitor().map(|m| m.active.id).filter(|i| *i != 0).unwrap_or(1);
                    let id = self.next_id;
                    self.next_id += 1;
                    next.entry(fw).or_default().push(St { id, wins: vec![k] });
                }
                continue;
            }
            if home == Some(wid) {
                continue;
            }
            if let Some(hm) = home {
                // Moved to another space by hand: leave its old stage
                if let Some(l) = next.get_mut(&hm) {
                    for s in l.iter_mut() {
                        s.wins.retain(|a| *a != k);
                    }
                    l.retain(|s| !s.wins.is_empty());
                }
            }
            let cur_id = act.get(&wid).copied().flatten();
            let list = next.entry(wid).or_default();
            let cur_idx = list.iter().position(|s| Some(s.id) == cur_id);
            let same_app = cur_idx.is_some_and(|i| list[i].wins.iter().any(|a| class_of(&win, a) == t.class));
            if let Some(i) = cur_idx.filter(|_| same_app || k != opened) {
                list[i].wins.push(k.clone());
            } else {
                let id = self.next_id;
                self.next_id += 1;
                list.insert(0, St { id, wins: vec![k.clone()] });
                if let Some(c) = cur_id.filter(|_| cur_idx.is_some() && k == opened) {
                    switch_to = Some((wid, c));
                }
                act.insert(wid, Some(id));
            }
            known.insert(k, wid);
        }
        let keys: Vec<i32> = act.keys().copied().collect();
        for ws in keys {
            let cur = act[&ws];
            let l = next.get(&ws);
            if !l.is_some_and(|l| l.iter().any(|s| Some(s.id) == cur)) {
                act.insert(ws, l.and_then(|l| l.first()).map(|s| s.id));
            }
        }
        // A new app took over the space: park the stage it replaced
        let to_park: Vec<String> = switch_to.and_then(|(ws, id)| next.get(&ws).and_then(|l| l.iter().find(|s| s.id == id)).map(|s| s.wins.clone())).unwrap_or_default();
        self.stages = next;
        self.active = act;
        for a in to_park {
            self.move_win(&a, To::Park);
        }
        self.publish(h);
    }

    /// Turning it on groups each space's windows by app and keeps the focused one
    pub fn start(&mut self, h: &mut Host) {
        self.stages.clear();
        self.active.clear();
        let Some(win) = h.st.shell.get::<Windows>().cloned() else { return };
        let mut next: BTreeMap<i32, Vec<St>> = BTreeMap::new();
        let mut act: BTreeMap<i32, i32> = BTreeMap::new();
        let focused_ws = win.focused_monitor().map(|m| m.active.id).filter(|i| *i != 0).unwrap_or(1);
        for t in &win.clients {
            let parked = t.workspace.name == PARK;
            let wid = if parked { focused_ws } else { t.workspace.id };
            if wid < 0 {
                continue;
            }
            let list = next.entry(wid).or_default();
            let class = t.class.clone();
            let idx = list.iter().position(|s| s.wins.first().is_some_and(|a| class_of(&win, a) == class));
            let idx = idx.unwrap_or_else(|| {
                let id = self.next_id;
                self.next_id += 1;
                list.push(St { id, wins: vec![] });
                list.len() - 1
            });
            list[idx].wins.push(hx::key(&t.address));
            if win.active == t.address && !parked {
                act.insert(wid, list[idx].id);
            }
        }
        let wss: Vec<i32> = next.keys().copied().collect();
        for ws in wss {
            let a = *act.entry(ws).or_insert_with(|| next[&ws][0].id);
            // the shown stage first (a stable sort, as the QML's)
            next.get_mut(&ws).unwrap().sort_by_key(|s| if s.id == a { 0 } else { 1 });
            let stages = next[&ws].clone();
            for s in stages {
                for addr in s.wins {
                    let parked = win.clients.iter().find(|c| hx::key(&c.address) == addr).is_some_and(|c| c.workspace.name == PARK);
                    if s.id != a && !parked {
                        self.move_win(&addr, To::Park);
                    } else if s.id == a && parked {
                        self.move_win(&addr, To::Space(ws));
                    }
                }
            }
        }
        self.stages = next;
        self.active = act.into_iter().map(|(k, v)| (k, Some(v))).collect();
        self.publish(h);
    }

    /// Turning it off brings every parked window back to its space
    pub fn stop(&mut self, h: &mut Host) {
        let stages = std::mem::take(&mut self.stages);
        let active = std::mem::take(&mut self.active);
        for (ws, list) in &stages {
            for s in list {
                if Some(s.id) != active.get(ws).copied().flatten() {
                    for a in &s.wins {
                        self.move_win(a, To::Space(*ws));
                    }
                }
            }
        }
        if let Some(win) = h.st.shell.get::<Windows>().cloned() {
            let ws = win.focused_monitor().map(|m| m.active.id).filter(|i| *i != 0).unwrap_or(1);
            for t in &win.clients {
                let k = hx::key(&t.address);
                if t.workspace.name == PARK && !stages.values().any(|l| l.iter().any(|s| s.wins.contains(&k))) {
                    self.move_win(&k, To::Space(ws));
                }
            }
        }
        self.publish(h);
    }

    // ── wiring ───────────────────────────────────────────────────────────────
    pub fn begin(&mut self, h: &mut Host, io: &mut Io) {
        self.enabled = h.cfg()["stageManager"]["enabled"].as_bool().unwrap_or(false);
        if self.enabled {
            io.arm("stage.start", 600);
        }
    }
    pub fn config_changed(&mut self, h: &mut Host, io: &mut Io) {
        let on = h.cfg()["stageManager"]["enabled"].as_bool().unwrap_or(false);
        if on == self.enabled {
            return;
        }
        self.enabled = on;
        if on {
            io.arm("stage.start", 600);
        } else {
            self.stop(h);
        }
    }
    /// Window info (workspaces, classes) arrives a moment after the shell starts
    pub fn tick_start(&mut self, h: &mut Host, io: &mut Io) {
        if !self.enabled {
            return;
        }
        let ready = h.st.shell.get::<Windows>().is_some_and(|w| w.clients.iter().all(|c| !c.class.is_empty()));
        if ready {
            self.start(h);
        } else {
            io.arm("stage.start", 600);
        }
    }
    pub fn tick_refresh(&mut self, h: &mut Host) {
        let opened = std::mem::take(&mut self.pending_open);
        self.reconcile(h, &opened);
    }
    pub fn event(&mut self, h: &mut Host, io: &mut Io, name: &str, data: &str) {
        if !self.enabled {
            return;
        }
        match name {
            "openwindow" => {
                self.pending_open = data.split(',').next().unwrap_or("").to_string();
                io.arm("stage.refresh", 150);
            }
            "closewindow" => io.arm("stage.refresh", 150),
            "movewindow" | "movewindowv2" if !self.quiet() => io.arm("stage.refresh", 150),
            "activewindowv2" if !self.quiet() => {
                // Focused a parked window: bring its stage forward
                let addr = hx::key(data);
                let parked = h.st.shell.get::<Windows>().is_some_and(|w| w.clients.iter().any(|c| hx::key(&c.address) == addr && c.workspace.name == PARK));
                if parked {
                    if let Some(ws) = self.ws_of_stage(&addr) {
                        if let Some(id) = self.stages.get(&ws).and_then(|l| l.iter().find(|s| s.wins.contains(&addr))).map(|s| s.id) {
                            self.show(h, ws, id, Some(&addr));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
