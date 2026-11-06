//! Tiling.qml: Tahoe-style tiling. A window goes to a half, quarter or third
//! of the screen, fills or centres, several arrange side by side, and Restore
//! returns it to its size before the first snap. Snapped windows float.
//! Picture in Picture for any window. The snap on a drag's drop (the glass
//! plugin's tessera-drag event) and the decorations' tile grid
//! (`tiling.snap [layout, address]`) come here; `qs ipc call tiling …`.

use super::hx::{self, Fresh};
use super::Io;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tessera_core::host::Host;
use tessera_core::services::clients::Client;

/// [x, y, w, h] as fractions of the usable area
pub fn layout(name: &str) -> Option<[f64; 4]> {
    Some(match name {
        "left" => [0.0, 0.0, 0.5, 1.0],
        "right" => [0.5, 0.0, 0.5, 1.0],
        "top" => [0.0, 0.0, 1.0, 0.5],
        "bottom" => [0.0, 0.5, 1.0, 0.5],
        "topLeft" => [0.0, 0.0, 0.5, 0.5],
        "topRight" => [0.5, 0.0, 0.5, 0.5],
        "bottomLeft" => [0.0, 0.5, 0.5, 0.5],
        "bottomRight" => [0.5, 0.5, 0.5, 0.5],
        "leftThird" => [0.0, 0.0, 1.0 / 3.0, 1.0],
        "centerThird" => [1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0],
        "rightThird" => [2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0],
        "leftTwoThirds" => [0.0, 0.0, 2.0 / 3.0, 1.0],
        "rightTwoThirds" => [1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0],
        "fill" => [0.0, 0.0, 1.0, 1.0],
        "center" => [0.15, 0.1, 0.7, 0.8],
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct R {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Settings → Windows & Spaces → Gaps: (between, at the screen's edges)
pub fn gaps(cfg: &Value) -> (f64, f64) {
    match cfg["appearance"]["gaps"].as_str().unwrap_or("compact") {
        "none" => (0.0, 0.0),
        "comfortable" => (5.0, 10.0),
        _ => (3.0, 4.0),
    }
}

/// Usable area of a monitor (raw `j/monitors`) in global coordinates
pub fn area(mon: Option<&Value>, gap_out: f64) -> Area {
    let m = mon.cloned().unwrap_or(Value::Null);
    let r: Vec<f64> = m["reserved"].as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default();
    let r = |i: usize| r.get(i).copied().unwrap_or(0.0);
    let scale = m["scale"].as_f64().filter(|s| *s > 0.0).unwrap_or(1.0);
    let x = m["x"].as_f64().unwrap_or(0.0) + r(0);
    let y = m["y"].as_f64().unwrap_or(0.0) + r(1);
    let w = m["width"].as_f64().unwrap_or(1920.0) / scale - r(0) - r(2);
    let h = m["height"].as_f64().unwrap_or(1080.0) / scale - r(1) - r(3);
    Area { x: x + gap_out, y: y + gap_out, w: w - 2.0 * gap_out, h: h - 2.0 * gap_out }
}

/// Rect for a layout; edges shared with another tile get half the inner gap
pub fn rect(name: &str, a: Area, gap_in: f64) -> Option<R> {
    let f = layout(name)?;
    let l = if f[0] > 0.0 { gap_in } else { 0.0 };
    let t = if f[1] > 0.0 { gap_in } else { 0.0 };
    let r = if f[0] + f[2] < 0.999 { gap_in } else { 0.0 };
    let b = if f[1] + f[3] < 0.999 { gap_in } else { 0.0 };
    if name == "center" {
        return Some(R { x: (a.x + a.w * f[0]).round() as i64, y: (a.y + a.h * f[1]).round() as i64, w: (a.w * f[2]).round() as i64, h: (a.h * f[3]).round() as i64 });
    }
    Some(R { x: (a.x + a.w * f[0] + l).round() as i64, y: (a.y + a.h * f[1] + t).round() as i64, w: (a.w * f[2] - l - r).round() as i64, h: (a.h * f[3] - t - b).round() as i64 })
}

#[derive(Clone, Debug)]
struct Prev {
    floating: bool,
    at: (i32, i32),
    size: (i32, i32),
}

#[derive(Default)]
pub struct Tiling {
    previous: HashMap<String, Prev>,
    pipped: HashSet<String>,
}

fn monitor_of<'a>(f: &'a Fresh, c: &Client) -> Option<&'a Value> {
    f.monitors.iter().find(|m| m["id"].as_i64() == Some(c.monitor as i64)).or_else(|| f.focused_monitor())
}

impl Tiling {
    fn place(&mut self, c: &Client, r: R) {
        let k = hx::key(&c.address);
        self.previous.entry(k).or_insert(Prev { floating: c.floating, at: c.at, size: c.size });
        if c.fullscreen > 0 {
            hx::window_action(&c.address, "fullscreen", &[("mode", hx::q("maximized")), ("action", hx::q("unset"))]);
        }
        if !c.floating {
            hx::window_action(&c.address, "float", &[("action", hx::q("enable"))]);
        }
        // resize keeps the centre, so move afterwards
        hx::window_action(&c.address, "resize", &[("x", r.w.to_string()), ("y", r.h.to_string())]);
        hx::window_action(&c.address, "move", &[("x", r.x.to_string()), ("y", r.y.to_string())]);
    }

    fn snap_one(&mut self, f: &Fresh, g: (f64, f64), name: &str, c: &Client) {
        if layout(name).is_none() {
            return;
        }
        let a = area(monitor_of(f, c), g.1);
        if name == "center" {
            // Keep the size, centre in the usable area
            let (w, h) = if c.floating { (c.size.0 as i64, c.size.1 as i64) } else { ((a.w * 0.7).round() as i64, (a.h * 0.8).round() as i64) };
            self.place(c, R { x: (a.x + (a.w - w as f64) / 2.0).round() as i64, y: (a.y + (a.h - h as f64) / 2.0).round() as i64, w, h });
            return;
        }
        if let Some(r) = rect(name, a, g.0) {
            self.place(c, r);
        }
    }

    /// Snap a window (the focused one when no address is given)
    pub fn snap(&mut self, io: &mut Io, name: &str, address: Option<String>) {
        if layout(name).is_none() {
            return;
        }
        let name = name.to_string();
        io.fetch_then(move |wm, h, _io, f| {
            let g = gaps(h.cfg());
            let target = address.unwrap_or_else(|| f.active.clone());
            if let Some(c) = f.client(&target).cloned() {
                wm.tiling.snap_one(&f, g, &name, &c);
            }
        });
    }

    /// Picture in Picture: small, bottom-right, on every space; again to put it back
    pub fn pip(&mut self, io: &mut Io) {
        io.fetch_then(move |wm, h, _io, f| {
            let g = gaps(h.cfg());
            let Some(c) = f.client(&f.active).cloned() else { return };
            let k = hx::key(&c.address);
            if wm.tiling.pipped.remove(&k) {
                hx::window_action(&c.address, "pin", &[("action", hx::q("disable"))]);
                wm.tiling.restore_one(&c);
                return;
            }
            let a = area(monitor_of(&f, &c), g.1);
            let w = 480.0;
            let ratio = (c.size.1.max(0) as f64 / (c.size.0.max(1) as f64)).clamp(0.3, 1.6);
            let hh = (w * ratio).round();
            wm.tiling.place(&c, R { x: (a.x + a.w - w - 14.0) as i64, y: (a.y + a.h - hh - 14.0) as i64, w: w as i64, h: hh as i64 });
            hx::window_action(&c.address, "pin", &[("action", hx::q("enable"))]);
            wm.tiling.pipped.insert(k);
        });
    }

    fn restore_one(&mut self, c: &Client) {
        let Some(p) = self.previous.remove(&hx::key(&c.address)) else { return };
        if !p.floating {
            hx::window_action(&c.address, "float", &[("action", hx::q("disable"))]);
        } else {
            hx::window_action(&c.address, "resize", &[("x", p.size.0.to_string()), ("y", p.size.1.to_string())]);
            hx::window_action(&c.address, "move", &[("x", p.at.0.to_string()), ("y", p.at.1.to_string())]);
        }
    }

    /// Back to the size and place before the first snap (or tiled again)
    pub fn restore(&mut self, io: &mut Io) {
        io.fetch_then(|wm, _h, _io, f| {
            if let Some(c) = f.client(&f.active).cloned() {
                wm.tiling.restore_one(&c);
            }
        });
    }

    /// Arrange the most recent windows: leftRight | topBottom | quarters
    pub fn arrange(&mut self, io: &mut Io, kind: &str) {
        let plan: &[&str] = match kind {
            "leftRight" => &["left", "right"],
            "topBottom" => &["top", "bottom"],
            "quarters" => &["topLeft", "topRight", "bottomLeft", "bottomRight"],
            _ => return,
        };
        io.fetch_then(move |wm, h, _io, f| {
            let g = gaps(h.cfg());
            let ws = f.client(&f.active).map(|c| c.workspace.id).unwrap_or_else(|| f.focused_ws());
            let mut list: Vec<&Client> = f.clients.iter().filter(|c| c.workspace.id == ws && !c.hidden).collect();
            let active = hx::key(&f.active);
            list.sort_by_key(|c| if hx::key(&c.address) == active { -1 } else { c.focus_history_id as i64 });
            let wins: Vec<Client> = list.into_iter().take(plan.len()).cloned().collect();
            for (i, c) in wins.iter().enumerate() {
                wm.tiling.snap_one(&f, g, plan[i], c);
            }
        });
    }

    /// Hyprland's events: a closed window's kept sizes go; a drop on a zone tiles
    pub fn event(&mut self, io: &mut Io, name: &str, data: &str) {
        if name == "closewindow" {
            let k = hx::key(data);
            self.previous.remove(&k);
            self.pipped.remove(&k);
            return;
        }
        if name != "tessera-drag" && name != "irice-drag" {
            return;
        }
        let p: Vec<&str> = data.split(',').collect();
        if p.len() >= 4 && p[0] == "drop" && p[2] != "none" && layout(p[2]).is_some() {
            self.snap(io, p[2], Some(p[3].to_string()));
        }
    }

    pub fn ipc(&mut self, io: &mut Io, func: &str, args: &[String]) -> Option<String> {
        match func {
            "snap" => self.snap(io, args.first().map(String::as_str).unwrap_or(""), None),
            "arrange" => self.arrange(io, args.first().map(String::as_str).unwrap_or("")),
            "restore" => self.restore(io),
            "pip" => self.pip(io),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_share_the_inner_gap() {
        let a = Area { x: 4.0, y: 4.0, w: 1000.0, h: 600.0 };
        let l = rect("left", a, 3.0).unwrap();
        let r = rect("right", a, 3.0).unwrap();
        assert_eq!((l.x, l.w), (4, 497));
        assert_eq!((r.x, r.w), (507, 497));
        let c = rect("center", a, 3.0).unwrap();
        assert_eq!((c.x, c.w), (154, 700));
        assert!(rect("nope", a, 3.0).is_none());
    }

    #[test]
    fn the_area_leaves_the_reserved_edge_and_the_gap() {
        let m = serde_json::json!({"x": 0, "y": 0, "width": 2000, "height": 1000, "scale": 2.0, "reserved": [0, 30, 0, 0]});
        let a = area(Some(&m), 4.0);
        assert_eq!((a.x, a.y, a.w, a.h), (4.0, 34.0, 992.0, 462.0));
    }
}
