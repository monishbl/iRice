//! Displays (pages/displays.qml): the arrangement (drag to place, edges
//! snap together), then the chosen display's resolution, refresh rate,
//! scale, rotation, mirroring and brightness over DDC/CI, wireless displays,
//! and what happens when one connects. The displays are services/
//! Displays.qml's (feed.displays: hyprctl's monitors, parsed there), and
//! every change goes back to it (it applies, saves and asks to keep risky
//! ones).

use super::super::super::model::{Block, Ctx, Opt, Page, Row};
use super::{ask, feed, s};
use serde_json::{json, Value};

/// A display on the arrangement: its place and logical size
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

fn placed_of(m: &Value) -> Placed {
    let n = |k: &str| m[k].as_f64().unwrap_or(0.0);
    Placed { name: s(&m["name"]), x: n("x"), y: n("y"), w: n("lw").max(1.0), h: n("lh").max(1.0) }
}

/// The box around every display (canvas.box)
pub fn bounds(ps: &[Placed]) -> (f64, f64, f64, f64) {
    if ps.is_empty() {
        return (0.0, 0.0, 1.0, 1.0);
    }
    let x0 = ps.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let y0 = ps.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let x1 = ps.iter().map(|p| p.x + p.w).fold(f64::NEG_INFINITY, f64::max);
    let y1 = ps.iter().map(|p| p.y + p.h).fold(f64::NEG_INFINITY, f64::max);
    (x0, y0, x1 - x0, y1 - y0)
}

/// Where a dropped display goes (canvas.snap): touching the nearest edge
/// of another, top or bottom (left or right) edges lined up when close
pub fn snap(ps: &[Placed], name: &str, lx: f64, ly: f64) -> (f64, f64) {
    let Some(me) = ps.iter().find(|p| p.name == name) else { return (lx, ly) };
    let others: Vec<&Placed> = ps.iter().filter(|p| p.name != name).collect();
    if others.is_empty() {
        return (me.x, me.y);
    }
    let near = 120.0;
    let clamp = |v: f64, lo: f64, hi: f64| v.max(lo).min(hi);
    let align = |v: f64, a: f64, b: f64| if (v - a).abs() < near { a } else if (v - b).abs() < near { b } else { v };
    let overlaps = |x: f64, y: f64| others.iter().any(|o| x < o.x + o.w && x + me.w > o.x && y < o.y + o.h && y + me.h > o.y);
    let mut best: Option<(f64, f64, f64)> = None;
    for o in &others {
        let ys = align(clamp(ly, o.y - me.h + 1.0, o.y + o.h - 1.0), o.y, o.y + o.h - me.h);
        let xs = align(clamp(lx, o.x - me.w + 1.0, o.x + o.w - 1.0), o.x, o.x + o.w - me.w);
        for (cx, cy) in [(o.x + o.w, ys), (o.x - me.w, ys), (xs, o.y + o.h), (xs, o.y - me.h)] {
            if overlaps(cx, cy) {
                continue;
            }
            let d = (cx - lx).hypot(cy - ly);
            if best.map_or(true, |b| d < b.2) {
                best = Some((cx.round(), cy.round(), d));
            }
        }
    }
    best.map_or((me.x, me.y), |b| (b.0, b.1))
}

/// Every display that moves when `name` is dropped at (lx, ly) (canvas.drop):
/// snapped, then all shifted so the top-left stays at 0,0
pub fn drop_at(ps: &[Placed], name: &str, lx: f64, ly: f64) -> Vec<(String, f64, f64)> {
    let p = snap(ps, name, lx, ly);
    let pos: Vec<(String, f64, f64)> = ps.iter().map(|m| if m.name == name { (m.name.clone(), p.0, p.1) } else { (m.name.clone(), m.x, m.y) }).collect();
    let min_x = pos.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let min_y = pos.iter().map(|p| p.2).fold(f64::INFINITY, f64::min);
    ps.iter()
        .zip(pos)
        .filter_map(|(m, (n, x, y))| {
            let (nx, ny) = (x - min_x, y - min_y);
            (nx != m.x || ny != m.y).then_some((n, nx, ny))
        })
        .collect()
}

/// A display's resolutions, largest first, the first marked Native: (value, label)
pub fn resolutions(m: &Value) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<(String, String, f64)> = Vec::new();
    for md in m["modes"].as_array().into_iter().flatten() {
        let (w, h) = (md["w"].as_f64().unwrap_or(0.0), md["h"].as_f64().unwrap_or(0.0));
        let key = format!("{w}x{h}");
        if !seen.insert(key.clone()) {
            continue;
        }
        out.push((key, format!("{w} × {h}"), w * h));
    }
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    if let Some(f) = out.first_mut() {
        f.1.push_str("  (Native)");
    }
    out.into_iter().map(|x| (x.0, x.1)).collect()
}

/// The refresh rates at the current size, fastest first: (mode text, label)
pub fn rates(m: &Value) -> Vec<(String, String)> {
    let res = format!("{}x{}", m["width"].as_f64().unwrap_or(0.0), m["height"].as_f64().unwrap_or(0.0));
    let mut v: Vec<&Value> = m["modes"].as_array().into_iter().flatten().filter(|md| format!("{}x{}", md["w"].as_f64().unwrap_or(0.0), md["h"].as_f64().unwrap_or(0.0)) == res).collect();
    v.sort_by(|a, b| b["r"].as_f64().unwrap_or(0.0).total_cmp(&a["r"].as_f64().unwrap_or(0.0)));
    v.into_iter().map(|md| (s(&md["text"]), format!("{} Hz", (md["r"].as_f64().unwrap_or(0.0) * 100.0).round() / 100.0))).collect()
}

/// The listed mode closest to the current rate
pub fn rate(m: &Value) -> String {
    let (w, h, r) = (m["width"].as_f64().unwrap_or(0.0), m["height"].as_f64().unwrap_or(0.0), m["refresh"].as_f64().unwrap_or(0.0));
    let mut best = (String::new(), f64::INFINITY);
    for md in m["modes"].as_array().into_iter().flatten() {
        let d = (md["r"].as_f64().unwrap_or(0.0) - r).abs();
        if md["w"].as_f64() == Some(w) && md["h"].as_f64() == Some(h) && d < best.1 {
            best = (s(&md["text"]), d);
        }
    }
    best.0
}

/// The fastest mode at a size ("1920x1080")
pub fn fastest(m: &Value, size: &str) -> Option<String> {
    let mut v: Vec<&Value> = m["modes"].as_array().into_iter().flatten().filter(|md| format!("{}x{}", md["w"].as_f64().unwrap_or(0.0), md["h"].as_f64().unwrap_or(0.0)) == size).collect();
    v.sort_by(|a, b| b["r"].as_f64().unwrap_or(0.0).total_cmp(&a["r"].as_f64().unwrap_or(0.0)));
    v.first().map(|md| s(&md["text"]))
}

/// The scales offered (100% … 200%, and the current one): (value, label)
pub fn scales(m: &Value) -> Vec<(f64, String)> {
    let mut list: Vec<(f64, f64, f64)> = m["scales"].as_array().into_iter().flatten().map(|x| (x["v"].as_f64().unwrap_or(1.0), x["w"].as_f64().unwrap_or(0.0), x["h"].as_f64().unwrap_or(0.0))).collect();
    list.sort_by(|a, b| a.0.total_cmp(&b.0));
    list.dedup_by(|a, b| (a.0 - b.0).abs() < 0.01);
    list.into_iter().map(|(v, w, h)| (v, format!("{}%  ·  looks like {w} × {h}", (v * 100.0).round()))).collect()
}

/// "1" for a Value (a number as JS writes it)
fn num_text(v: &Value) -> String {
    v.as_f64().map_or(String::new(), |n| if n.fract() == 0.0 { format!("{}", n as i64) } else { format!("{n}") })
}

pub fn displays(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let d = &f["displays"];
    let mons: Vec<Value> = d["monitors"].as_array().cloned().unwrap_or_default();
    let selected = cx.local_s("displays.selected");
    let current = mons.iter().find(|m| s(&m["name"]) == selected).or_else(|| mons.iter().find(|m| m["enabled"].as_bool() == Some(true))).or(mons.first()).cloned();
    let placed: Vec<Value> = mons.iter().filter(|m| m["enabled"].as_bool() == Some(true) && s(&m["mirror"]).is_empty()).cloned().collect();
    let enabled_count = mons.iter().filter(|m| m["enabled"].as_bool() == Some(true)).count();
    let cur_name = current.as_ref().map(|m| s(&m["name"])).unwrap_or_default();
    let number = |m: &Value| num_text(&m["number"]);

    // ── Keep these settings? ──
    if d["confirming"].as_bool().unwrap_or(false) {
        let left = d["confirmLeft"].as_i64().unwrap_or(0);
        let id = cx.on(|cx, a| ask(cx, if a.i == 1 { "displaysKeep" } else { "displaysRevert" }, Value::Null));
        p.custom("devices", "confirm")
            .id(id)
            .title("Keep these display settings?")
            .text(format!("Going back to the previous settings in {left}{}", if left == 1 { " second." } else { " seconds." }));
    }

    if mons.is_empty() {
        p.text(if d["loaded"].as_bool().unwrap_or(false) { "No displays found. Hyprland didn't report any outputs." } else { "Looking for displays…" });
    } else {
        // ── Arrangement ──
        let ps: Vec<Placed> = placed.iter().map(placed_of).collect();
        let (bx, by, bw, bh) = bounds(&ps);
        let ps2 = ps.clone();
        let id = cx.on(move |cx, a| {
            let parts: Vec<&str> = a.s.split('|').collect();
            match parts.first().copied() {
                Some("select") => cx.local_set("displays.selected", parts.get(1).copied().unwrap_or("")),
                Some("drop") if parts.len() >= 4 => {
                    let (lx, ly) = (parts[2].parse::<f64>().unwrap_or(0.0), parts[3].parse::<f64>().unwrap_or(0.0));
                    let list: Vec<Value> = drop_at(&ps2, parts[1], lx, ly).into_iter().map(|(n, x, y)| json!({"name": n, "changes": {"x": x, "y": y}})).collect();
                    if !list.is_empty() {
                        ask(cx, "displaysSetMany", json!([list]));
                    }
                }
                Some("identify") => ask(cx, "displaysIdentify", Value::Null),
                _ => {}
            }
        });
        let mut b = Block::default();
        for m in &placed {
            let pl = placed_of(m);
            let mut r = Row::new(number(m)).detail(s(&m["label"]));
            r.id = pl.name.clone();
            r.on = pl.name == cur_name;
            r.value = if m["builtIn"].as_bool() == Some(true) { "builtIn".into() } else { String::new() };
            (r.from, r.to, r.step, r.num) = (pl.x as f32, pl.y as f32, pl.w as f32, pl.h as f32);
            b.rows.push(r);
        }
        let blk = p.custom("devices", "arrange");
        blk.rows = b.rows;
        blk.id(id)
            .data(vec![Opt::new("").num(bx as f32), Opt::new("").num(by as f32), Opt::new("").num(bw as f32), Opt::new("").num(bh as f32)])
            .title("Identify")
            .text("Shows each display's number on it for a moment")
            .footer(if placed.len() > 1 { "Drag displays to match how they sit on your desk; edges snap together. Click one to change it below." } else { "Connect another display to arrange them." });
        blk.on = placed.len() > 1;

        // ── Which display ──
        if mons.len() > 1 {
            let names: Vec<String> = mons.iter().map(|m| s(&m["name"])).collect();
            let id = cx.on(move |cx, a| {
                if let Some(n) = names.get(a.i.max(0) as usize) {
                    cx.local_set("displays.selected", n.clone());
                }
            });
            let opts: Vec<Opt> = mons.iter().map(|m| Opt::new(format!("{}  {}", number(m), s(&m["label"])))).collect();
            let at = mons.iter().position(|m| s(&m["name"]) == cur_name).unwrap_or(0);
            p.custom("devices", "picker").id(id).data(opts).num(at as f32);
        }
    }

    // ── The chosen display ──
    if let Some(m) = current.clone() {
        let name = s(&m["name"]);
        let on = m["enabled"].as_bool().unwrap_or(false);
        let mirror = s(&m["mirror"]);
        let only = enabled_count <= 1 && on;

        let n = name.clone();
        let use_it = cx.on(move |cx, _| ask(cx, "displaysSet", json!([n, if on { json!({"enabled": false}) } else { json!({"enabled": true, "mirror": ""}) }])));
        let mut rows = vec![Row::new("Use this display").detail(if only { "The only display that's on can't be turned off" } else { "" }).switch(on, use_it).enabled(!only)];

        // Mirror
        let others: Vec<&Value> = mons.iter().filter(|o| s(&o["name"]) != name && o["enabled"].as_bool() == Some(true) && s(&o["mirror"]).is_empty()).collect();
        let mut values = vec![String::new()];
        let mut labels = vec!["Off".to_string()];
        for o in &others {
            values.push(s(&o["name"]));
            labels.push(format!("{}  {}", number(o), s(&o["label"])));
        }
        let at = values.iter().position(|v| *v == mirror).map_or(-1, |i| i as i32);
        let n = name.clone();
        let vs = values.clone();
        let pick = cx.on(move |cx, a| {
            if let Some(v) = vs.get(a.i.max(0) as usize) {
                ask(cx, "displaysSet", json!([n, {"mirror": v}]));
            }
        });
        rows.push(Row::new("Mirror").visible(mons.len() > 1 && on).detail("Show the same picture as another display").choice(&labels, at, pick));

        let shown = on && mirror.is_empty();
        // Resolution
        let res = resolutions(&m);
        let cur_res = format!("{}x{}", m["width"].as_f64().unwrap_or(0.0), m["height"].as_f64().unwrap_or(0.0));
        let at = res.iter().position(|r| r.0 == cur_res).map_or(-1, |i| i as i32);
        let (n, mm, keys) = (name.clone(), m.clone(), res.iter().map(|r| r.0.clone()).collect::<Vec<_>>());
        let pick = cx.on(move |cx, a| {
            if let Some(mode) = keys.get(a.i.max(0) as usize).and_then(|k| fastest(&mm, k)) {
                ask(cx, "displaysSet", json!([n, {"mode": mode}]));
            }
        });
        rows.push(Row::new("Resolution").visible(shown).choice(&res.iter().map(|r| r.1.clone()).collect::<Vec<_>>(), at, pick));
        // Refresh rate
        let rs = rates(&m);
        let cur_rate = rate(&m);
        let at = rs.iter().position(|r| r.0 == cur_rate).map_or(-1, |i| i as i32);
        let lowered = d["lowered"].as_array().is_some_and(|l| l.iter().any(|x| s(x) == name));
        let (n, keys) = (name.clone(), rs.iter().map(|r| r.0.clone()).collect::<Vec<_>>());
        let pick = cx.on(move |cx, a| {
            if let Some(mode) = keys.get(a.i.max(0) as usize) {
                ask(cx, "displaysSet", json!([n, {"mode": mode}]));
            }
        });
        rows.push(Row::new("Refresh rate").visible(shown && !rs.is_empty()).detail(if lowered { "Cool-down has lowered it for now" } else { "" }).choice(&rs.iter().map(|r| r.1.clone()).collect::<Vec<_>>(), at, pick));
        // Scale
        let sc = scales(&m);
        let cur_scale = m["scale"].as_f64().unwrap_or(1.0);
        let at = sc.iter().position(|x| (x.0 - cur_scale).abs() < 0.01).or_else(|| sc.iter().position(|x| x.0 == 1.0)).map_or(-1, |i| i as i32);
        let (n, vals) = (name.clone(), sc.iter().map(|x| x.0).collect::<Vec<_>>());
        let pick = cx.on(move |cx, a| {
            if let Some(v) = vals.get(a.i.max(0) as usize) {
                ask(cx, "displaysSet", json!([n, {"scale": v}]));
            }
        });
        rows.push(Row::new("Scale").visible(shown).detail("Larger makes text and controls bigger; smaller fits more on screen").choice(&sc.iter().map(|x| x.1.clone()).collect::<Vec<_>>(), at, pick).width(260.0));
        // Rotation
        let t = m["transform"].as_i64().unwrap_or(0);
        let n = name.clone();
        let pick = cx.on(move |cx, a| ask(cx, "displaysSet", json!([n, {"transform": a.i as i64 + if t >= 4 { 4 } else { 0 }}])));
        rows.push(Row::new("Rotation").visible(shown).segmented(&["Standard", "90°", "180°", "270°"], (t % 4) as i32, pick).width(260.0));
        // Brightness over DDC/CI
        let ddc = &f["ddc"];
        let built_in = ["eDP", "LVDS", "DSI"].iter().any(|p| name.starts_with(p));
        let has_ddc = ddc["displays"].as_array().is_some_and(|l| l.iter().any(|x| s(x) == name));
        let installed = ddc["installed"].as_bool().unwrap_or(false);
        let value = ddc["values"][&name].as_f64().unwrap_or(0.5) as f32;
        let n = name.clone();
        let slide = cx.on(move |cx, a| ask(cx, "ddcSet", json!([n, a.f])));
        rows.push(Row::new("Brightness").visible(has_ddc && on).detail(format!("{}%, set on the display itself (DDC/CI)", (value * 100.0).round())).slider(value, 0.0, 1.0, 0.0, slide).width(220.0));
        rows.push(Row::new("Brightness").visible(!built_in && on && !installed).detail("To change this display's brightness from Tessera, install ddcutil (sudo pacman -S ddcutil), then log out and back in."));
        let problem = s(&ddc["problem"]);
        let scanning = ddc["scanning"].as_bool().unwrap_or(false);
        let again = cx.on(|cx, _| ask(cx, "ddcRescan", Value::Null));
        rows.push(Row::new("Brightness")
            .visible(!built_in && on && installed && !has_ddc)
            .tone(if problem.is_empty() { 0 } else { 2 })
            .detail(match problem.as_str() {
                "module" => "ddcutil can't reach displays because the i2c-dev module isn't loaded. Run sudo modprobe i2c-dev (to load it at every start: echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf), then check again.",
                "access" => "ddcutil isn't allowed to open /dev/i2c-*. Add yourself to the i2c group (sudo usermod -aG i2c $USER), log out and back in, then check again.",
                _ if !ddc["scanned"].as_bool().unwrap_or(false) || scanning => "Looking for DDC/CI support…",
                _ => "This display doesn't answer DDC/CI. Turn on DDC/CI in the display's own menu, then check again.",
            })
            .button("Check Again", again)
            .enabled(!scanning));
        // Flip
        let n = name.clone();
        let flip = cx.on(move |cx, _| ask(cx, "displaysSet", json!([n, {"transform": (t % 4) + if t >= 4 { 0 } else { 4 }}])));
        rows.push(Row::new("Flip").visible(shown).detail("Mirror image, for teleprompters and projectors").switch(t >= 4, flip));

        let g = p.group(format!("{} ({})", s(&m["label"]), name));
        g.footer(if on { s(&m["description"]) } else { "This display is off. Turn it on to change its resolution and arrangement.".into() });
        for r in rows {
            g.row(r);
        }
    }

    // ── Wireless displays ──
    let mi = &f["mirroring"];
    let available = mi["available"].as_bool().unwrap_or(false);
    let needs = &f["needs"];
    let footer = if !available {
        "Show this screen on a TV: Miracast TVs and sticks, Windows' Miracast over the network, and Chromecasts. Needs GNOME Network Displays.".to_string()
    } else if !s(&mi["error"]).is_empty() {
        s(&mi["error"])
    } else if needs["checked"].as_bool().unwrap_or(false) && !needs["networkManager"].as_bool().unwrap_or(true) {
        "Miracast TVs need NetworkManager (for Wi-Fi Direct), which isn't running here. Chromecasts and Miracast over the network still work.".into()
    } else if !mi["p2p"].as_bool().unwrap_or(true) {
        "This computer's Wi-Fi can't do Wi-Fi Direct, which Miracast TVs need. Chromecasts and network Miracast still work.".into()
    } else {
        "Turn on the TV's screen mirroring (Miracast, Screen Mirroring or Cast). The Screen Mirroring tile in Quick Settings does the same.".into()
    };
    let install_error = s(&mi["installError"]);
    let installing = mi["installing"].as_bool().unwrap_or(false);
    let install = cx.on(|cx, _| ask(cx, "mirrorInstall", Value::Null));
    let tvs: Vec<Value> = if available { mi["displays"].as_array().cloned().unwrap_or_default() } else { Vec::new() };
    let mut rows = vec![
        Row::new("Add a wireless display")
            .visible(!available)
            .detail(if install_error.is_empty() { "It asks for your password once.".to_string() } else { install_error.clone() })
            .tone(if install_error.is_empty() { 0 } else { 2 })
            .button(if installing { "Installing…" } else { "Install" }, install)
            .busy(installing)
            .enabled(!f["updatesRunning"].as_bool().unwrap_or(false)),
        Row::new("Looking for TVs…").visible(available && tvs.is_empty()).detail("Nearby TVs and sticks appear here"),
    ];
    let target = s(&mi["target"]);
    for tv in &tvs {
        let uuid = s(&tv["uuid"]);
        let here = target == uuid;
        let id = cx.on(move |cx, _| if here { ask(cx, "mirrorStop", Value::Null) } else { ask(cx, "mirrorStart", json!([uuid])) });
        let mut r = Row::new(s(&tv["name"]))
            .detail(if here { if s(&mi["state"]) == "streaming" { "Mirroring".to_string() } else { "Connecting…".into() } } else { s(&tv["protocol"]) })
            .buttons(vec![Opt::new(if here { "Stop" } else { "Mirror" }).icon(if here { "stop" } else { "screencast" })], id);
        r.icon = String::new();
        rows.push(r);
    }
    let g = p.group("Wireless displays");
    g.footer(footer);
    for r in rows {
        g.row(r);
    }

    // ── More options ──
    p.more(cx);
    {
        let values = ["off", "extend", "liquid", "warp"];
        let anim = cx.s("displays.connectAnimation");
        let pick = cx.pick("displays.connectAnimation", &values);
        let ds = cx.b("displays.directionalSound");
        let dt = cx.toggle("displays.directionalSound");
        let preview = cx.on(|cx, a| {
            let anim = cx.s("displays.connectAnimation");
            let anim = if anim == "off" { "extend".to_string() } else { anim };
            let side = ["left", "top", "right"].get(a.i.max(0) as usize).copied().unwrap_or("left");
            ask(cx, "fxPreview", json!([anim, side]));
        });
        p.group("When a display connects")
            .footer("The new display fills from the side it shares with its neighbour, which comes from the arrangement above; a display that leaves pulls back from that side. Directional sound plays the connect sound from that side (with Sound effects on).")
            .row(Row::new("Animation").segmented(&["Off", "Extend", "Liquid", "Warp"], values.iter().position(|v| *v == anim).unwrap_or(0) as i32, pick).width(300.0))
            .row(Row::new("Directional sound").detail("Heard from the side the display is on").switch(ds, dt))
            .row(Row::new("Preview").detail("On this display, as if one were plugged in on that side").buttons(vec![Opt::new("Left").icon("arrow-left"), Opt::new("Above").icon("arrow-up"), Opt::new("Right").icon("arrow-right")], preview));

        let warp = cx.b("workspaces.warpPointer");
        let wt = cx.toggle("workspaces.warpPointer");
        let cue = cx.s("workspaces.switchCue");
        let cue = if cue.is_empty() { "glow".to_string() } else { cue };
        let cues = ["off", "glow", "dim"];
        let cp = cx.pick("workspaces.switchCue", &cues);
        p.group("Switching displays")
            .footer("Each display has its own spaces, numbered from 1: Super+number goes to that space on the display you're on.")
            .row(Row::new("Move the pointer there").detail("When a shortcut takes you to another display").switch(warp, wt))
            .row(Row::new("Show which display you moved to")
                .detail(match cue.as_str() {
                    "dim" => "It dims briefly",
                    "off" => "Nothing shows",
                    _ => "A soft glow at its edges for a moment",
                })
                .segmented(&["Off", "Glow", "Dim"], cues.iter().position(|v| *v == cue).unwrap_or(1) as i32, cp)
                .width(200.0));

        let saved = cx.get("displays.monitors").as_array().map_or(0, Vec::len);
        let reset = cx.on(|cx, _| ask(cx, "displaysReset", Value::Null));
        p.group("")
            .footer("Settings are saved per display and come back when it's plugged in again.")
            .row(Row::new("Restore defaults").detail("Every display goes back to what Hyprland picks by itself").button("Restore Defaults", reset).enabled(saved > 0));
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pl(n: &str, x: f64, y: f64, w: f64, h: f64) -> Placed {
        Placed { name: n.into(), x, y, w, h }
    }

    #[test]
    fn a_dropped_display_snaps_to_an_edge() {
        let ps = vec![pl("A", 0.0, 0.0, 1920.0, 1080.0), pl("B", 1920.0, 0.0, 1280.0, 800.0)];
        // dropped below A, nearly lined up with its left edge
        assert_eq!(snap(&ps, "B", 60.0, 1100.0), (0.0, 1080.0));
        // dropped to the left
        assert_eq!(snap(&ps, "B", -1300.0, 40.0), (-1280.0, 0.0));
        // and the arrangement shifts back to 0,0
        let moved = drop_at(&ps, "B", -1300.0, 40.0);
        assert_eq!(moved, vec![("A".to_string(), 1280.0, 0.0), ("B".to_string(), 0.0, 0.0)]);
        assert_eq!(bounds(&ps), (0.0, 0.0, 3200.0, 1080.0));
    }

    #[test]
    fn modes_as_the_menus_list_them() {
        let m = json!({"width": 1920, "height": 1080, "refresh": 59.95, "modes": [
            {"w": 1920, "h": 1080, "r": 60.0, "text": "1920x1080@60.00"},
            {"w": 1920, "h": 1080, "r": 144.0, "text": "1920x1080@144.00"},
            {"w": 1280, "h": 720, "r": 60.0, "text": "1280x720@60.00"}],
            "scales": [{"v": 1, "w": 1920, "h": 1080}, {"v": 1.5, "w": 1280, "h": 720}, {"v": 1, "w": 1920, "h": 1080}]});
        let r = resolutions(&m);
        assert_eq!(r[0], ("1920x1080".to_string(), "1920 × 1080  (Native)".to_string()));
        assert_eq!(r.len(), 2);
        assert_eq!(rates(&m)[0], ("1920x1080@144.00".to_string(), "144 Hz".to_string()));
        assert_eq!(rate(&m), "1920x1080@60.00");
        assert_eq!(fastest(&m, "1920x1080").as_deref(), Some("1920x1080@144.00"));
        let s = scales(&m);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].1, "150%  ·  looks like 1280 × 720");
    }
}
