//! Displays (Settings → Displays) and the display connect effect, replacing
//! shell/services/Displays.qml and shell/services/DisplayFx.qml.
//!
//! Displays: resolution, refresh rate, scale, rotation, mirroring and
//! arrangement applied live with `hyprctl eval hl.monitor(…)`; saved in
//! config displays.monitors (one tab-separated line per changed display) and
//! in <config dir>/monitors.lua (loaded by hyprland.lua at login); put back on
//! start, after a config reload and when a display is plugged in, only where
//! Hyprland differs; risky changes wait 15 s for Keep, then go back.
//!
//! DisplayFx: which way a connected/removed display is (the arrangement), the
//! "runs" the connect overlay shows (modules/osd/DisplayConnect.qml reads
//! bus "displayfx"), the connect sound and Spaces handover.
//!
//! Calls (prefix "displays"): set [name, changes], setMany [[{name,changes}]],
//! keep, revert, reset (QML resetAll), identify, refresh, preview [effect,
//! side] (also ipc target "displays" fn preview, as DisplayConnect.qml's
//! IpcHandler). State for other services: bus "displays" and "displayfx".

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

#[derive(Clone, Debug, PartialEq)]
pub struct Mon {
    pub name: String,
    pub description: String,
    pub label: String,
    pub width: i64,
    pub height: i64,
    pub refresh: f64,
    pub mode: String,
    pub x: i64,
    pub y: i64,
    pub scale: f64,
    pub transform: i64,
    pub enabled: bool,
    pub mirror: String,
    pub built_in: bool,
    pub modes: Vec<Value>,
}

/// A saved entry (name -> state), Displays.savedMap's values
#[derive(Clone, Debug, PartialEq)]
pub struct St {
    pub mode: String,
    pub x: i64,
    pub y: i64,
    pub scale: f64,
    pub transform: i64,
    pub enabled: bool,
    pub mirror: String,
}

pub fn rate_text(r: f64) -> String {
    format!("{:.2}", (r * 100.0).round() / 100.0)
}

/// JS's number to string (1 -> "1", 1.5 -> "1.5")
pub fn js_num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn is_builtin(name: &str) -> bool {
    name.starts_with("eDP") || name.starts_with("LVDS") || name.starts_with("DSI")
}

pub fn nice_name(m: &Value) -> String {
    let name = m["name"].as_str().unwrap_or("");
    if is_builtin(name) {
        return "Built-in Display".into();
    }
    let model = m["model"].as_str().unwrap_or("").trim();
    let make = m["make"].as_str().unwrap_or("").trim();
    if !model.is_empty() && model != "0x0000" {
        return if model.starts_with(make) || make.is_empty() { model.to_string() } else { format!("{make} {model}") };
    }
    name.to_string()
}

/// "1920x1080@60.00Hz" -> {w,h,r,text}; duplicates dropped
pub fn parse_modes(list: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for s in list.as_array().into_iter().flatten().filter_map(Value::as_str) {
        let Some(body) = s.strip_suffix("Hz") else { continue };
        let Some((wh, r)) = body.split_once('@') else { continue };
        let Some((w, h)) = wh.split_once('x') else { continue };
        let (Ok(w), Ok(h)) = (w.parse::<i64>(), h.parse::<i64>()) else { continue };
        if r.is_empty() || !r.chars().all(|c| c.is_ascii_digit() || c == '.') {
            continue;
        }
        let Ok(rate) = r.parse::<f64>() else { continue };
        let text = format!("{w}x{h}@{r}");
        if out.iter().any(|m| m["text"] == text) {
            continue;
        }
        out.push(json!({"w": w, "h": h, "r": rate, "text": text}));
    }
    out
}

fn mirror_name(list: &[Value], of: &Value) -> String {
    let key = match of {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    list.iter().find(|m| m["id"].to_string().trim_matches('"') == key).and_then(|m| m["name"].as_str()).map(String::from).unwrap_or(key)
}

pub fn parse(list: &[Value]) -> Vec<Mon> {
    list.iter()
        .filter(|m| {
            let n = m["name"].as_str().unwrap_or("");
            !(n.starts_with("HEADLESS") || n.starts_with("TESSERA-"))
        })
        .map(|m| {
            let name = m["name"].as_str().unwrap_or("").to_string();
            let refresh = m["refreshRate"].as_f64().unwrap_or(0.0);
            let (w, h) = (m["width"].as_i64().unwrap_or(0), m["height"].as_i64().unwrap_or(0));
            let of = &m["mirrorOf"];
            let mirror = if !of.is_null() && of.as_str() != Some("none") && of.as_str() != Some("") { mirror_name(list, of) } else { String::new() };
            Mon {
                description: m["description"].as_str().unwrap_or("").into(),
                label: nice_name(m),
                width: w,
                height: h,
                refresh,
                mode: format!("{w}x{h}@{}", rate_text(refresh)),
                x: m["x"].as_i64().unwrap_or(0),
                y: m["y"].as_i64().unwrap_or(0),
                scale: m["scale"].as_f64().unwrap_or(1.0),
                transform: m["transform"].as_i64().unwrap_or(0),
                enabled: !m["disabled"].as_bool().unwrap_or(false),
                mirror,
                built_in: is_builtin(&name),
                modes: parse_modes(&m["availableModes"]),
                name,
            }
        })
        .collect()
}

pub fn line_of(name: &str, s: &St) -> String {
    [name.to_string(), s.mode.clone(), s.x.to_string(), s.y.to_string(), js_num(s.scale), s.transform.to_string(), (if s.enabled { "1" } else { "0" }).into(), s.mirror.clone()].join("\t")
}

/// Config lines -> entries in order (Displays.savedMap)
pub fn saved_map(lines: &[String]) -> Vec<(String, St)> {
    let mut out: Vec<(String, St)> = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 8 || f[0].is_empty() {
            continue;
        }
        let st = St {
            mode: f[1].into(),
            x: f[2].parse().unwrap_or(0),
            y: f[3].parse().unwrap_or(0),
            scale: f[4].parse::<f64>().ok().filter(|s| *s != 0.0).unwrap_or(1.0),
            transform: f[5].parse().unwrap_or(0),
            enabled: f[6] != "0",
            mirror: f[7].into(),
        };
        if let Some(e) = out.iter_mut().find(|(n, _)| n == f[0]) {
            e.1 = st;
        } else {
            out.push((f[0].into(), st));
        }
    }
    out
}

pub fn spec(name: &str, s: &St) -> String {
    if !s.enabled {
        return format!("hl.monitor({{ output = {}, disabled = true }})", lua_str(name));
    }
    let mode = if s.mode.is_empty() { "preferred" } else { &s.mode };
    let mut f = format!("output = {}, mode = {}, position = {}, scale = {}", lua_str(name), lua_str(mode), lua_str(&format!("{}x{}", s.x, s.y)), js_num(s.scale));
    if s.transform != 0 {
        f += &format!(", transform = {}", s.transform);
    }
    if !s.mirror.is_empty() {
        f += &format!(", mirror = {}", lua_str(&s.mirror));
    }
    format!("hl.monitor({{ {f} }})")
}

pub fn file_text(map: &[(String, St)]) -> String {
    let mut s = String::from("-- Generated by Tessera from Settings → Displays; changes here are overwritten.\n-- Loaded from hyprland.lua: pcall(dofile, TESSERA_USER_DIR .. \"/monitors.lua\")\n");
    for (k, v) in map {
        s += &spec(k, v);
        s.push('\n');
    }
    s
}

/// "1920x1080@60" -> (1920, 1080, Some(60))
fn mode_parts(m: &str) -> Option<(i64, i64, Option<f64>)> {
    let (w, rest) = m.split_once('x')?;
    let w: i64 = w.parse().ok()?;
    let h_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let h: i64 = rest[..h_end].parse().ok()?;
    let r = rest[h_end..].strip_prefix('@').and_then(|r| {
        let e = r.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(r.len());
        r[..e].parse::<f64>().ok()
    });
    Some((w, h, r))
}

pub fn same(a: &St, b: &St) -> bool {
    if a.enabled != b.enabled {
        return false;
    }
    if !a.enabled {
        return true;
    }
    let mode_same = if a.mode == "preferred" || b.mode == "preferred" {
        true
    } else {
        match (mode_parts(&a.mode), mode_parts(&b.mode)) {
            (Some((aw, ah, ar)), Some((bw, bh, br))) => aw == bw && ah == bh && (ar.is_none() || br.is_none() || (ar.unwrap() - br.unwrap()).abs() < 0.5),
            _ => a.mode == b.mode,
        }
    };
    mode_same && a.x == b.x && a.y == b.y && (a.scale - b.scale).abs() < 0.01 && a.transform == b.transform && a.mirror == b.mirror
}

fn mode_same(a: &str, b: &str) -> bool {
    let mk = |m: &str| St { enabled: true, mode: m.into(), x: 0, y: 0, scale: 1.0, transform: 0, mirror: String::new() };
    same(&mk(a), &mk(b))
}

/// Logical size: what windows see (scaled, turned for 90°/270°)
pub fn logical_size(m: &Mon, s: Option<(&str, f64, i64)>) -> (i64, i64) {
    let (mode, scale, transform) = s.map(|(a, b, c)| (a.to_string(), b, c)).unwrap_or((m.mode.clone(), m.scale, m.transform));
    let (mut w, mut h) = mode_parts(&mode).map(|(w, h, _)| (w, h)).unwrap_or((m.width, m.height));
    if transform % 2 == 1 {
        std::mem::swap(&mut w, &mut h);
    }
    let sc = if scale == 0.0 { 1.0 } else { scale };
    (round(w as f64 / sc), round(h as f64 / sc))
}

fn st_json(s: &St) -> Value {
    json!({"mode": s.mode, "x": s.x, "y": s.y, "scale": s.scale, "transform": s.transform, "enabled": s.enabled, "mirror": s.mirror})
}

// ── DisplayFx's geometry ──────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rel {
    pub edge: i64,
    pub focus: f64,
    pub other_edge: i64,
    pub other_focus: f64,
}

pub fn rect_of(m: &Value) -> Rect {
    let s = m["scale"].as_f64().filter(|s| *s != 0.0).unwrap_or(1.0);
    let w = round(m["width"].as_f64().unwrap_or(0.0) / s) as f64;
    let h = round(m["height"].as_f64().unwrap_or(0.0) / s) as f64;
    let (x, y) = (m["x"].as_f64().unwrap_or(0.0), m["y"].as_f64().unwrap_or(0.0));
    if m["transform"].as_i64().unwrap_or(0) % 2 != 0 {
        Rect { x, y, w: h, h: w }
    } else {
        Rect { x, y, w, h }
    }
}

/// Where `b` is, seen from `a`; None when they don't touch (or overlap)
pub fn relation(a: Rect, b: Rect) -> Option<Rel> {
    let tol = 8.0;
    let edge = if b.x >= a.x + a.w - tol {
        2
    } else if b.x + b.w <= a.x + tol {
        0
    } else if b.y >= a.y + a.h - tol {
        3
    } else if b.y + b.h <= a.y + tol {
        1
    } else {
        return None;
    };
    let vertical = edge == 0 || edge == 2;
    let lo = if vertical { a.y.max(b.y) } else { a.x.max(b.x) };
    let hi = if vertical { (a.y + a.h).min(b.y + b.h) } else { (a.x + a.w).min(b.x + b.w) };
    let mid = if hi > lo { (lo + hi) / 2.0 } else if vertical { b.y + b.h / 2.0 } else { b.x + b.w / 2.0 };
    let clamp = |v: f64| v.clamp(0.0, 1.0);
    Some(Rel {
        edge,
        focus: clamp(if vertical { (mid - a.y) / a.h } else { (mid - a.x) / a.w }),
        other_edge: (edge + 2) % 4,
        other_focus: clamp(if vertical { (mid - b.y) / b.h } else { (mid - b.x) / b.w }),
    })
}

fn distance(a: Rect, b: Rect) -> f64 {
    let dx = (a.x + a.w / 2.0) - (b.x + b.w / 2.0);
    let dy = (a.y + a.h / 2.0) - (b.y + b.h / 2.0);
    dx.hypot(dy)
}

/// The displays touching `r` (nearest first), each with its relation
pub fn neighbours(r: Rect, others: &[(String, Rect)]) -> Vec<(String, Rel, f64)> {
    let mut v: Vec<(String, Rel, f64)> = others.iter().filter_map(|(n, o)| relation(*o, r).map(|rel| (n.clone(), rel, distance(*o, r)))).collect();
    v.sort_by(|p, q| p.2.partial_cmp(&q.2).unwrap_or(std::cmp::Ordering::Equal));
    v
}

pub fn lead_for(fx: &str) -> i64 {
    match fx {
        "liquid" => 620,
        "extend" => 120,
        _ => 0,
    }
}

// ── The service ───────────────────────────────────────────────────────────

enum M {
    Raw(Raw),
    /// `hyprctl -j monitors all`
    Read(Value),
    /// `hyprctl -j monitors` (DisplayFx's)
    FxRead(Value),
    Settle(u64),
    FxSettle(u64),
    Countdown(u64),
    IdentifyEnd(u64),
    RestoreDelay(u64),
    Quiet(u64),
    HandoverLimit(u64),
    PreviewNext(u64, String, i64, String),
    Armed,
}

struct Pending {
    name: String,
    state: St,
    had_saved: bool,
}

#[derive(Default)]
pub struct Displays {
    monitors: Vec<Mon>,
    loaded: bool,
    reading: bool,
    read_again: bool,
    restore_pending: bool,
    saved: Vec<String>,
    pending_revert: Option<Vec<Pending>>,
    confirm_left: i64,
    countdown_gen: u64,
    settle_gen: u64,
    restore_gen: u64,
    identifying: bool,
    identify_gen: u64,
    // DisplayFx
    armed: bool,
    geometry: Vec<(String, Rect)>,
    added: Vec<String>,
    removed: Vec<String>,
    runs: Vec<Value>,
    serial: i64,
    hotplug_at: u64,
    waiting: Vec<String>,
    queued: Option<Vec<Value>>,
    burst_at: u64,
    fx_reading: bool,
    fx_read_again: bool,
    fx_settle_gen: u64,
    fx_settle_running: bool,
    removed_last: bool,
    quiet_since: u64,
    last_tick: u64,
    calm: i64,
    quiet_gen: u64,
    handover_gen: u64,
    preview_gen: u64,
    started: bool,
}

impl Displays {
    fn sandbox() -> bool {
        tessera_core::env::sandboxed()
    }
    fn ignored(name: &str) -> bool {
        name.is_empty() || name.starts_with("HEADLESS") || (name.starts_with("TESSERA-") && !Self::sandbox())
    }
    fn cfg_lines(h: &Host) -> Vec<String> {
        h.cfg()["displays"]["monitors"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()
    }
    fn style(h: &Host) -> String {
        if h.cfg().is_null() {
            return "off".into();
        }
        h.cfg()["displays"]["connectAnimation"].as_str().unwrap_or("extend").to_string()
    }
    fn still(h: &Host) -> bool {
        h.cfg()["appearance"]["reduceMotion"] == true
    }
    fn motion_scale() -> f64 {
        crate::bus::field("cooldown", "motionScale").as_f64().unwrap_or(1.0)
    }
    fn kept_rate(name: &str) -> Option<f64> {
        crate::bus::get("cooldown")["savedRates"][name].as_f64()
    }

    fn refresh(&mut self, h: &mut Host) {
        if self.reading {
            self.read_again = true;
            return;
        }
        self.reading = true;
        let send = h.sender("Displays");
        std::thread::spawn(move || send(Box::new(M::Read(hyprctl_json(&["monitors", "all"])))));
    }

    fn publish(&self) {
        let monitors: Vec<Value> = self
            .monitors
            .iter()
            .map(|m| {
                let (lw, lh) = logical_size(m, None);
                let scales: Vec<Value> = [1.0, 1.25, 1.5, 1.75, 2.0, (m.scale * 100.0).round() / 100.0]
                    .iter()
                    .map(|v| {
                        let (w, h) = logical_size(m, Some((&m.mode, *v, m.transform)));
                        json!({"v": v, "w": w, "h": h})
                    })
                    .collect();
                json!({
                    "name": m.name, "description": m.description, "label": m.label, "width": m.width, "height": m.height,
                    "refresh": m.refresh, "mode": m.mode, "x": m.x, "y": m.y, "scale": m.scale, "transform": m.transform,
                    "enabled": m.enabled, "mirror": m.mirror, "builtIn": m.built_in, "modes": m.modes,
                    "lw": lw, "lh": lh, "number": self.number_of(&m.name), "scales": scales
                })
            })
            .collect();
        let lowered: Vec<String> = crate::bus::get("cooldown")["savedRates"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        crate::bus::put(
            "displays",
            json!({
                "loaded": self.loaded, "confirming": self.pending_revert.is_some(), "confirmLeft": self.confirm_left,
                "identifying": self.identifying, "monitors": monitors, "saved": self.saved.len(), "lowered": lowered
            }),
        );
    }

    fn publish_fx(&self) {
        crate::bus::put("displayfx", json!({"runs": self.runs, "hotplugAt": self.hotplug_at, "settling": self.settling()}));
    }

    fn settling(&self) -> bool {
        self.hotplug_at > 0 && now_ms().saturating_sub(self.hotplug_at) < 4000
    }

    /// Numbers left to right, then top to bottom (as in the arrangement)
    fn number_of(&self, name: &str) -> usize {
        let mut order: Vec<&Mon> = self.monitors.iter().collect();
        order.sort_by(|a, b| a.x.cmp(&b.x).then(a.y.cmp(&b.y)));
        order.iter().position(|m| m.name == name).map(|i| i + 1).unwrap_or(0)
    }

    fn state_of(m: &Mon) -> St {
        let mut mode = m.mode.clone();
        if let Some(kept) = Self::kept_rate(&m.name) {
            mode = format!("{}x{}@{}", m.width, m.height, rate_text(kept));
        }
        St { mode, x: m.x, y: m.y, scale: m.scale, transform: m.transform, enabled: m.enabled, mirror: m.mirror.clone() }
    }

    fn save(&mut self, h: &mut Host, map: &[(String, St)]) {
        self.saved = map.iter().map(|(k, v)| line_of(k, v)).collect();
        crate::util::config_set("displays.monitors", json!(self.saved));
        let file = tessera_core::theme::config_path().with_file_name("monitors.lua");
        let text = file_text(map);
        std::thread::spawn(move || {
            let _ = crate::util::write_atomic(&file, &text);
        });
        let _ = h;
    }

    fn eval_all(&mut self, h: &mut Host, entries: &[(String, St)]) {
        if entries.is_empty() {
            return;
        }
        hyprctl_eval(&entries.iter().map(|(n, s)| spec(n, s)).collect::<Vec<_>>().join("\n"));
        self.settle_gen += 1;
        after(h.sender("Displays"), 700, M::Settle(self.settle_gen));
    }

    fn restore(&mut self, h: &mut Host) {
        let saved = saved_map(&self.saved);
        let mut todo: Vec<(String, St)> = Vec::new();
        for m in &self.monitors {
            let Some((_, s)) = saved.iter().find(|(n, _)| *n == m.name) else { continue };
            let mut now = Self::state_of(m);
            if Self::kept_rate(&m.name).is_some() {
                now.mode = s.mode.clone();
            }
            if same(&now, s) {
                continue;
            }
            let mut want = s.clone();
            if m.enabled && s.enabled && mode_same(&now.mode, &s.mode) {
                want.mode = m.mode.clone();
            }
            todo.push((m.name.clone(), want));
        }
        self.eval_all(h, &todo);
    }

    pub fn set_many(&mut self, h: &mut Host, list: &[Value]) {
        let mut saved = saved_map(&self.saved);
        let mut entries: Vec<(String, St)> = Vec::new();
        let mut before: Vec<Pending> = Vec::new();
        let mut risky = false;
        for item in list {
            let name = item["name"].as_str().unwrap_or("").to_string();
            let changes = &item["changes"];
            let Some(m) = self.monitors.iter().find(|x| x.name == name) else { continue };
            let old = Self::state_of(m);
            let had = saved.iter().find(|(n, _)| *n == name).map(|(_, s)| s.clone());
            let mut next = had.clone().unwrap_or_else(|| old.clone());
            if let Some(v) = changes["mode"].as_str() {
                next.mode = v.into();
            }
            if let Some(v) = changes["x"].as_f64() {
                next.x = round(v);
            }
            if let Some(v) = changes["y"].as_f64() {
                next.y = round(v);
            }
            if let Some(v) = changes["scale"].as_f64() {
                next.scale = v;
            }
            if let Some(v) = changes["transform"].as_i64() {
                next.transform = v;
            }
            if let Some(v) = changes["enabled"].as_bool() {
                next.enabled = v;
            }
            if let Some(v) = changes["mirror"].as_str() {
                next.mirror = v.into();
            }
            // Positions always come from what's on screen now unless changed
            if changes.get("x").is_none() {
                next.x = old.x;
                next.y = old.y;
            }
            if next.enabled && (next.mode.is_empty() || next.mode.starts_with("0x0")) {
                next.mode = "preferred".into();
            }
            if next.enabled && !old.enabled && had.is_none() && changes.get("x").is_none() {
                next.x = self
                    .monitors
                    .iter()
                    .filter(|o| o.enabled && o.mirror.is_empty() && o.name != name)
                    .map(|o| o.x + logical_size(o, None).0)
                    .fold(0, i64::max);
                next.y = 0;
            }
            if same(&old, &next) {
                continue;
            }
            let old_j = st_json(&old);
            for k in ["mode", "scale", "transform", "enabled", "mirror"] {
                if !changes[k].is_null() && changes[k] != old_j[k] {
                    // (numbers compare by value: 1 and 1.0)
                    let eq = changes[k].as_f64().zip(old_j[k].as_f64()).is_some_and(|(a, b)| a == b);
                    if !eq {
                        risky = true;
                    }
                }
            }
            before.push(Pending { name: name.clone(), state: had.clone().unwrap_or(old), had_saved: had.is_some() });
            if let Some(e) = saved.iter_mut().find(|(n, _)| *n == name) {
                e.1 = next.clone();
            } else {
                saved.push((name.clone(), next.clone()));
            }
            entries.push((name, next));
        }
        if entries.is_empty() {
            return;
        }
        // A second change while confirming keeps the oldest state to go back to
        if risky && self.pending_revert.is_none() {
            self.pending_revert = Some(before);
            self.confirm_left = 15;
            self.countdown_gen += 1;
            after(h.sender("Displays"), 1000, M::Countdown(self.countdown_gen));
        }
        self.save(h, &saved);
        self.eval_all(h, &entries);
        self.publish();
    }

    fn keep(&mut self) {
        self.countdown_gen += 1;
        self.pending_revert = None;
        self.publish();
    }

    fn revert(&mut self, h: &mut Host) {
        self.countdown_gen += 1;
        let back = self.pending_revert.take().unwrap_or_default();
        let mut saved = saved_map(&self.saved);
        for b in &back {
            if b.had_saved {
                if let Some(e) = saved.iter_mut().find(|(n, _)| *n == b.name) {
                    e.1 = b.state.clone();
                } else {
                    saved.push((b.name.clone(), b.state.clone()));
                }
            } else {
                saved.retain(|(n, _)| *n != b.name);
            }
        }
        self.save(h, &saved);
        let entries: Vec<(String, St)> = back.into_iter().map(|b| (b.name, b.state)).collect();
        self.eval_all(h, &entries);
        self.publish();
    }

    /// Back to what Hyprland picks by itself
    fn reset_all(&mut self, h: &mut Host) {
        self.saved.clear();
        crate::util::config_set("displays.monitors", json!([]));
        let file = tessera_core::theme::config_path().with_file_name("monitors.lua");
        std::thread::spawn(move || {
            let _ = std::fs::remove_file(&file);
            crate::util::run_async(&["hyprctl", "reload", "config-only"]);
        });
        self.settle_gen += 1;
        after(h.sender("Displays"), 700, M::Settle(self.settle_gen));
    }

    fn identify(&mut self, h: &mut Host) {
        self.identifying = true;
        self.identify_gen += 1;
        after(h.sender("Displays"), 2500, M::IdentifyEnd(self.identify_gen));
        self.publish();
    }

    // ── DisplayFx ──

    fn show(&mut self, list: Vec<Value>, style: &str) {
        let keep: Vec<Value> = self.runs.iter().filter(|r| r["pending"] != true && !list.iter().any(|x| x["screen"] == r["screen"])).cloned().collect();
        self.runs = keep;
        for mut x in list {
            self.serial += 1;
            let mut r = json!({"id": self.serial, "style": style, "reverse": 0, "focus": 0.5, "pending": false});
            if let Value::Object(o) = &mut x {
                for (k, v) in std::mem::take(o) {
                    r[k] = v;
                }
            }
            self.runs.push(r);
        }
        self.publish_fx();
    }

    /// Overlay finished (DisplayConnect tells: displays.finish [id])
    fn finish(&mut self, h: &mut Host, id: i64) {
        let r = self.runs.iter().find(|x| x["id"] == id).cloned();
        self.runs.retain(|x| x["id"] != id);
        if let Some(r) = r {
            if r["role"] == 0 {
                self.handover(h, r["screen"].as_str().unwrap_or(""));
            }
        }
        self.publish_fx();
    }

    fn handover(&mut self, h: &mut Host, screen: &str) {
        if !self.waiting.iter().any(|n| n == screen) {
            return;
        }
        self.waiting.retain(|n| n != screen);
        h.ask("spaces.arrive", json!([screen]));
    }

    fn fx_event(&mut self, h: &mut Host, e: &Raw) {
        let n = e.name.as_str();
        if n != "monitoraddedv2" && n != "monitorremovedv2" {
            return;
        }
        let name = e.data.split(',').nth(1).unwrap_or("").to_string();
        if Self::ignored(&name) {
            return;
        }
        self.hotplug_at = now_ms();
        self.removed_last = n == "monitorremovedv2";
        if n == "monitoraddedv2" {
            self.added.push(name.clone());
        } else {
            self.removed.push(name.clone());
        }
        let style = Self::style(h);
        if style != "off" && self.armed {
            let mut names: Vec<String> = h.st.outputs_now().into_iter().map(|(_, n)| n).filter(|s| *s != name).collect();
            if n == "monitoraddedv2" {
                names.push(name.clone());
            }
            let still = Self::still(h);
            let want: Vec<String> = names.into_iter().filter(|s| !Self::ignored(s) && (*s == name || !still) && !self.runs.iter().any(|r| r["screen"] == *s)).collect();
            for s in want {
                self.serial += 1;
                self.runs.push(json!({"id": self.serial, "screen": s, "role": if s == name {0} else {1}, "edge": 0, "focus": 0.5, "reverse": 0, "style": style, "pending": true}));
            }
            self.publish_fx();
        }
        // The first event is read at once; the rest of a burst once it's quiet
        // for a moment, at most 600 ms on
        let quiet = !self.fx_settle_running && !self.fx_reading && now_ms().saturating_sub(self.burst_at) > 600;
        if quiet {
            self.burst_at = now_ms();
        }
        let interval = if quiet { 0 } else { (600 - now_ms().saturating_sub(self.burst_at) as i64).clamp(0, 150) as u64 };
        self.fx_settle_gen += 1;
        self.fx_settle_running = true;
        after(h.sender("Displays"), interval, M::FxSettle(self.fx_settle_gen));
    }

    fn fx_read(&mut self, h: &mut Host) {
        if self.fx_reading {
            self.fx_read_again = true;
            return;
        }
        self.fx_reading = true;
        let send = h.sender("Displays");
        std::thread::spawn(move || send(Box::new(M::FxRead(hyprctl_json(&["monitors"])))));
    }

    fn happen(&mut self, h: &mut Host, now: Vec<(String, Rect)>) {
        let before = std::mem::replace(&mut self.geometry, now.clone());
        let has = |l: &[(String, Rect)], n: &str| l.iter().any(|(k, _)| k == n);
        let uniq = |v: &[String]| {
            let mut o: Vec<String> = Vec::new();
            for x in v {
                if !o.contains(x) {
                    o.push(x.clone());
                }
            }
            o
        };
        let arrived: Vec<String> = uniq(&self.added).into_iter().filter(|n| has(&now, n) && !has(&before, n)).collect();
        let left: Vec<String> = uniq(&self.removed).into_iter().filter(|n| !has(&now, n) && has(&before, n)).collect();
        // (an event that came while this was being read waits for the next read)
        let more = self.fx_settle_running || self.fx_read_again;
        self.added = if more { self.added.iter().filter(|n| !arrived.contains(n) && !has(&now, n)).cloned().collect() } else { vec![] };
        self.removed = if more { self.removed.iter().filter(|n| !left.contains(n) && has(&now, n)).cloned().collect() } else { vec![] };
        let style = Self::style(h);
        if (arrived.is_empty() && left.is_empty()) || !self.armed {
            if !more {
                self.runs.retain(|r| r["pending"] != true);
                self.publish_fx();
            }
            return;
        }
        let mut list: Vec<Value> = Vec::new();
        let mut side = "";
        let side_of = |edge: i64| if edge == 0 { "left" } else if edge == 2 { "right" } else { "" };
        let old: Vec<(String, Rect)> = now.iter().filter(|(n, _)| !arrived.contains(n)).cloned().collect();
        for n in &arrived {
            let r = now.iter().find(|(k, _)| k == n).map(|(_, r)| *r).unwrap();
            let near = neighbours(r, &old);
            let first = near.first();
            list.push(json!({
                "screen": n, "role": 0, "edge": first.map(|f| f.1.other_edge).unwrap_or(0),
                "focus": first.map(|f| f.1.other_focus).unwrap_or(0.5), "lead": if first.is_some() { lead_for(&style) } else { 0 }
            }));
            for x in &near {
                list.push(json!({"screen": x.0, "role": 1, "edge": x.1.edge, "focus": x.1.focus, "reverse": 0}));
            }
            if let Some(f) = first {
                if side.is_empty() {
                    side = side_of(f.1.edge);
                }
            }
        }
        for n in &left {
            let r = before.iter().find(|(k, _)| k == n).map(|(_, r)| *r).unwrap();
            let near = neighbours(r, &now);
            for x in &near {
                list.push(json!({"screen": x.0, "role": 1, "edge": x.1.edge, "focus": x.1.focus, "reverse": 1}));
            }
            if let Some(f) = near.first() {
                if side.is_empty() {
                    side = side_of(f.1.edge);
                }
            }
        }
        if style != "off" {
            let l = if Self::still(h) { list.iter().filter(|r| r["role"] == 0).cloned().collect() } else { list.clone() };
            self.when_quiet(h, l);
        } else {
            self.runs.retain(|r| r["pending"] != true);
        }
        let screens: Vec<String> = h.st.outputs_now().into_iter().map(|(_, n)| n).collect();
        for n in &arrived {
            if style != "off" && list.iter().any(|r| r["screen"] == *n && r["role"] == 0) && screens.contains(n) {
                self.waiting.push(n.clone());
            } else {
                h.ask("spaces.arrive", json!([n]));
            }
        }
        if !self.waiting.is_empty() {
            self.handover_gen += 1;
            after(h.sender("Displays"), 3500, M::HandoverLimit(self.handover_gen));
        }
        if h.cfg()["displays"]["directionalSound"].as_bool().unwrap_or(true) && !h.cfg().is_null() {
            self.sound(h, !arrived.is_empty(), side);
        }
        self.publish_fx();
    }

    /// The connect / disconnect sound, panned toward the display (your own
    /// sound file, if you picked one, plays as it is)
    fn sound(&self, h: &mut Host, connected: bool, side: &str) {
        let base = if connected { "displays" } else { "displaysOff" };
        let own = h.cfg()["sounds"]["displaysFile"].as_str().is_some_and(|s| !s.is_empty());
        let name = if !side.is_empty() && !own { format!("{base}{}", if side == "left" { "Left" } else { "Right" }) } else { base.to_string() };
        h.ask("sound.play", json!([name]));
    }

    /// A screen that was just added makes the GUI thread wait on the first
    /// frame of each window; the effect starts once that has passed
    fn when_quiet(&mut self, h: &mut Host, list: Vec<Value>) {
        let q = match self.queued.take() {
            Some(q) => {
                let keep: Vec<Value> = q.into_iter().filter(|r| !list.iter().any(|x| x["screen"] == r["screen"])).collect();
                keep.into_iter().chain(list).collect()
            }
            None => list,
        };
        self.queued = Some(q);
        self.quiet_since = now_ms();
        self.last_tick = now_ms();
        self.calm = 0;
        self.quiet_gen += 1;
        after(h.sender("Displays"), 16, M::Quiet(self.quiet_gen));
    }

    fn quiet_tick(&mut self, h: &mut Host) {
        let t = now_ms();
        self.calm = if t - self.last_tick < 30 { self.calm + 1 } else { 0 };
        self.last_tick = t;
        let sync_wait = if Self::sandbox() { 0 } else if self.removed_last { 900 } else { 1800 };
        if (self.calm >= 3 || t - self.quiet_since > 600) && t.saturating_sub(self.hotplug_at) >= sync_wait {
            let list = self.queued.take().unwrap_or_default();
            let style = Self::style(h);
            self.show(list, &style);
        } else {
            after(h.sender("Displays"), 16, M::Quiet(self.quiet_gen));
        }
    }

    /// Settings' Preview buttons and `qs ipc call displays preview liquid left`
    fn preview(&mut self, h: &mut Host, effect: &str, side: &str) {
        let style = Self::style(h);
        let fx = if ["extend", "liquid", "warp"].contains(&effect) { effect.to_string() } else if style != "off" { style } else { "extend".into() };
        let edge: i64 = match side {
            "left" => 0,
            "top" => 1,
            "bottom" => 3,
            _ => 2,
        };
        let focused = h.st.shell.get::<tessera_core::services::hypr::Snapshot>().map(|s| s.focused.clone()).filter(|f| !f.is_empty());
        let screen = focused.or_else(|| h.st.outputs_now().into_iter().next().map(|(_, n)| n)).unwrap_or_default();
        if screen.is_empty() {
            return;
        }
        self.runs.retain(|r| r["screen"] != screen);
        self.serial += 1;
        self.runs.push(json!({"id": self.serial, "screen": screen, "role": 1, "edge": edge, "focus": 0.5, "reverse": 0, "style": fx, "pending": false}));
        self.preview_gen += 1;
        let ms = ((match fx.as_str() { "liquid" => 660.0, "extend" => 260.0, _ => 650.0 }) * Self::motion_scale()).round() as u64;
        after(h.sender("Displays"), ms, M::PreviewNext(self.preview_gen, screen, edge, fx));
        h.ask("sound.play", json!([if edge == 0 { "displaysLeft" } else if edge == 2 { "displaysRight" } else { "displays" }, true]));
        self.publish_fx();
    }
}

impl Service for Displays {
    fn name(&self) -> &'static str {
        "Displays"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["displays", "displayfx"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["displays"]
    }
    fn start(&mut self, h: &mut Host) {
        self.started = true;
        self.saved = Self::cfg_lines(h);
        spawn_hypr_events(&["configreloaded", "monitoraddedv2", "monitorremovedv2"], h.sender("Displays"));
        // (events arrive as Raw; wrap them)
        after(h.sender("Displays"), 5000, M::Armed);
        self.refresh(h);
        self.fx_read(h);
    }
    fn config_changed(&mut self, h: &mut Host) {
        let lines = Self::cfg_lines(h);
        if lines != self.saved {
            self.saved = lines;
        }
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
        match call.split_once('.').map(|x| x.1).unwrap_or("") {
            "set" => {
                let name = a(0).as_str().unwrap_or("").to_string();
                self.set_many(h, &[json!({"name": name, "changes": a(1)})]);
            }
            "setMany" => {
                let l = a(0).as_array().cloned().unwrap_or_default();
                self.set_many(h, &l);
            }
            "keep" => self.keep(),
            "revert" => self.revert(h),
            "reset" | "resetAll" => self.reset_all(h),
            "identify" => self.identify(h),
            "refresh" => self.refresh(h),
            "preview" => self.preview(h, a(0).as_str().unwrap_or(""), a(1).as_str().unwrap_or("left")),
            "finish" => self.finish(h, a(0).as_i64().unwrap_or(0)),
            other => eprintln!("tessera-ui: displays: no call {other}"),
        }
    }
    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, args: &[String]) -> Option<String> {
        if func == "preview" {
            self.preview(h, args.first().map(String::as_str).unwrap_or(""), args.get(1).map(String::as_str).unwrap_or(""));
        }
        None
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Raw>() {
            Ok(e) => {
                match e.name.as_str() {
                    "configreloaded" | "monitoraddedv2" => {
                        self.restore_pending = true;
                        self.restore_gen += 1;
                        after(h.sender("Displays"), 500, M::RestoreDelay(self.restore_gen));
                    }
                    "monitorremovedv2" => {
                        self.restore_gen += 1;
                        after(h.sender("Displays"), 500, M::RestoreDelay(self.restore_gen));
                    }
                    _ => {}
                }
                self.fx_event(h, &e);
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Raw(_) => {}
            M::Read(v) => {
                self.reading = false;
                if let Some(list) = v.as_array() {
                    self.monitors = parse(list);
                    let first = !self.loaded;
                    self.loaded = true;
                    if self.restore_pending {
                        self.restore_pending = false;
                        self.restore(h);
                    } else if first {
                        self.restore(h);
                    }
                    self.publish();
                }
                if self.read_again {
                    self.read_again = false;
                    self.refresh(h);
                }
            }
            M::FxRead(v) => {
                self.fx_reading = false;
                let list = v.as_array().cloned().unwrap_or_default();
                let now: Vec<(String, Rect)> = list
                    .iter()
                    .filter(|m| !Self::ignored(m["name"].as_str().unwrap_or("")) && m["disabled"] != true)
                    .map(|m| (m["name"].as_str().unwrap_or("").to_string(), rect_of(m)))
                    .collect();
                self.happen(h, now);
                if self.fx_read_again {
                    self.fx_read_again = false;
                    self.fx_read(h);
                }
            }
            M::Settle(g) => {
                if g == self.settle_gen {
                    self.refresh(h);
                }
            }
            M::FxSettle(g) => {
                if g == self.fx_settle_gen {
                    self.fx_settle_running = false;
                    self.fx_read(h);
                }
            }
            M::Countdown(g) => {
                if g == self.countdown_gen && self.pending_revert.is_some() {
                    self.confirm_left -= 1;
                    if self.confirm_left <= 0 {
                        self.revert(h);
                    } else {
                        after(h.sender("Displays"), 1000, M::Countdown(g));
                        self.publish();
                    }
                }
            }
            M::IdentifyEnd(g) => {
                if g == self.identify_gen {
                    self.identifying = false;
                    self.publish();
                }
            }
            M::RestoreDelay(g) => {
                if g == self.restore_gen {
                    self.refresh(h);
                    self.fx_read(h);
                }
            }
            M::Quiet(g) => {
                if g == self.quiet_gen && self.queued.is_some() {
                    self.quiet_tick(h);
                }
            }
            M::HandoverLimit(g) => {
                if g == self.handover_gen {
                    for n in self.waiting.clone() {
                        self.handover(h, &n);
                    }
                }
            }
            M::PreviewNext(g, screen, edge, fx) => {
                if g == self.preview_gen {
                    self.runs.retain(|r| r["screen"] != screen);
                    self.serial += 1;
                    self.runs.push(json!({"id": self.serial, "screen": screen, "role": 0, "edge": edge, "focus": 0.5, "reverse": 0, "style": fx, "pending": false}));
                    self.publish_fx();
                }
            }
            M::Armed => self.armed = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_names() {
        let m = parse_modes(&json!(["1920x1080@60.00Hz", "1920x1080@60.00Hz", "2560x1440@144.00Hz", "bad"]));
        assert_eq!(m.len(), 2);
        assert_eq!(m[1]["text"], "2560x1440@144.00");
        assert_eq!(rate_text(59.9512), "59.95");
        assert_eq!(nice_name(&json!({"name": "eDP-1"})), "Built-in Display");
        assert_eq!(nice_name(&json!({"name": "DP-1", "make": "Dell", "model": "U2720Q"})), "Dell U2720Q");
        assert_eq!(nice_name(&json!({"name": "DP-1", "make": "Dell", "model": "Dell U27"})), "Dell U27");
        assert_eq!(nice_name(&json!({"name": "DP-2", "model": "0x0000"})), "DP-2");
    }

    #[test]
    fn saved_lines_and_lua() {
        let s = St { mode: "1920x1080@60.00".into(), x: 0, y: 0, scale: 1.5, transform: 1, enabled: true, mirror: String::new() };
        let l = line_of("DP-1", &s);
        assert_eq!(l, "DP-1\t1920x1080@60.00\t0\t0\t1.5\t1\t1\t");
        let m = saved_map(&[l]);
        assert_eq!(m[0].1, s);
        assert_eq!(spec("DP-1", &s), "hl.monitor({ output = \"DP-1\", mode = \"1920x1080@60.00\", position = \"0x0\", scale = 1.5, transform = 1 })");
        let off = St { enabled: false, ..s.clone() };
        assert_eq!(spec("DP-2", &off), "hl.monitor({ output = \"DP-2\", disabled = true })");
        assert!(same(&St { mode: "1920x1080@60".into(), ..s.clone() }, &St { mode: "1920x1080@60.01".into(), ..s.clone() }));
        assert!(!same(&s, &St { scale: 2.0, ..s.clone() }));
    }

    #[test]
    fn arrangement() {
        let a = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 };
        let b = Rect { x: 1920.0, y: 0.0, w: 1920.0, h: 1080.0 };
        let r = relation(a, b).unwrap();
        assert_eq!((r.edge, r.other_edge, r.focus, r.other_focus), (2, 0, 0.5, 0.5));
        assert!(relation(a, Rect { x: 100.0, y: 100.0, w: 50.0, h: 50.0 }).is_none());
        let n = neighbours(b, &[("A".into(), a)]);
        assert_eq!(n[0].0, "A");
        assert_eq!(lead_for("liquid"), 620);
    }

    #[test]
    fn logical() {
        let m = Mon { name: "x".into(), description: "".into(), label: "".into(), width: 2560, height: 1440, refresh: 60.0, mode: "2560x1440@60.00".into(), x: 0, y: 0, scale: 2.0, transform: 1, enabled: true, mirror: "".into(), built_in: false, modes: vec![] };
        assert_eq!(logical_size(&m, None), (720, 1280));
    }
}
