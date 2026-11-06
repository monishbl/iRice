//! Windows & Spaces (pages/windowrules.qml, with appearance-windows.qml as
//! its part "Windows, Stages and Overview"). Window rules: pick an app, then
//! choose how its windows open; three views (the apps with rules, the app
//! picker, one app's rules). The rules are config.json's
//! windowRules.rules, one JSON string per app, as services/WindowRules.qml
//! keeps them: written here, the shell's service writes window-rules.lua
//! and applies them to Hyprland when the setting changes.

use super::super::super::model::{Ctx, Opt, Page, Row};
use super::{app_icon, feed, idx, place, sw};
use serde_json::{json, Value};

// ── The rules (WindowRules.qml's normalize, summary, set, remove, restore)

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rule {
    pub class: String,
    pub name: String,
    pub float: bool,
    pub tile: bool,
    pub workspace: i64,
    pub opaque: bool,
    pub start: String,
    pub pin: bool,
    pub width: i64,
    pub height: i64,
    pub self_maximize: bool,
}

/// JavaScript's truthiness, for a rule read from the file
fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Null => false,
        _ => true,
    }
}

/// Math.max(0, parseInt(v) || 0)
fn to_int(v: &Value) -> i64 {
    let n = match v {
        Value::Number(n) => n.as_f64().map(|f| f.trunc() as i64).unwrap_or(0),
        Value::String(s) => {
            let t = s.trim();
            let digits: String = t.chars().enumerate().take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+'))).map(|(_, c)| c).collect();
            digits.parse().unwrap_or(0)
        }
        _ => 0,
    };
    n.max(0)
}

pub fn normalize(r: &Value) -> Rule {
    let s = |k: &str| match &r[k] {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    };
    let class = s("class");
    let name = if truthy(&r["name"]) { s("name") } else { class.clone() };
    let float = truthy(&r["float"]);
    let pin = truthy(&r["pin"]);
    let start = s("start");
    Rule {
        name,
        float,
        tile: truthy(&r["tile"]) && !float && !pin,
        workspace: to_int(&r["workspace"]),
        opaque: truthy(&r["opaque"]),
        start: if start == "maximize" || start == "fullscreen" { start } else { String::new() },
        pin,
        width: to_int(&r["width"]),
        height: to_int(&r["height"]),
        self_maximize: truthy(&r["selfMaximize"]),
        class,
    }
}

impl Rule {
    pub fn new(class: &str, name: &str) -> Self {
        normalize(&json!({ "class": class, "name": name }))
    }
    pub fn to_value(&self) -> Value {
        json!({
            "class": self.class, "name": self.name, "float": self.float, "tile": self.tile,
            "workspace": self.workspace, "opaque": self.opaque, "start": self.start, "pin": self.pin,
            "width": self.width, "height": self.height, "selfMaximize": self.self_maximize
        })
    }
    /// Apply a change ({float: true, tile: false}…), as Object.assign
    pub fn patched(&self, patch: &Value) -> Rule {
        let mut v = self.to_value();
        if let (Some(o), Some(p)) = (v.as_object_mut(), patch.as_object()) {
            for (k, x) in p {
                o.insert(k.clone(), x.clone());
            }
        }
        normalize(&v)
    }
    /// "Floats · Space 3 · Opaque"
    pub fn summary(&self) -> String {
        let mut s: Vec<String> = Vec::new();
        if self.pin {
            s.push("All spaces".into());
        } else if self.float {
            s.push("Floats".into());
        } else if self.tile {
            s.push("Tiles".into());
        }
        if self.workspace > 0 {
            s.push(format!("Space {}", self.workspace));
        }
        if self.start == "maximize" {
            s.push("Maximized".into());
        } else if self.start == "fullscreen" {
            s.push("Fullscreen".into());
        }
        if self.opaque {
            s.push("Opaque".into());
        }
        if self.width > 0 && self.height > 0 {
            s.push(format!("{} × {}", self.width, self.height));
        }
        if self.self_maximize {
            s.push("Maximizes itself".into());
        }
        if s.is_empty() {
            "Nothing set".into()
        } else {
            s.join(" · ")
        }
    }
}

/// The rules in config.json's list of JSON strings (bad ones left out)
pub fn parse(list: &[String]) -> Vec<Rule> {
    list.iter().filter_map(|s| serde_json::from_str::<Value>(s).ok()).filter(|v| v.is_object() && truthy(&v["class"])).map(|v| normalize(&v)).collect()
}

/// The list as config.json keeps it
pub fn save(rules: &[Rule]) -> Value {
    Value::Array(rules.iter().map(|r| Value::from(r.to_value().to_string())).collect())
}

/// WindowRules.set: change an app's rule (made when it has none)
pub fn set(rules: &[Rule], class: &str, name: &str, patch: &Value) -> Vec<Rule> {
    let mut list = rules.to_vec();
    match list.iter().position(|r| r.class == class) {
        Some(i) => list[i] = list[i].patched(patch),
        None => list.push(Rule::new(class, name).patched(patch)),
    }
    list
}

/// WindowRules.remove: the list without it, and the index it had
pub fn remove(rules: &[Rule], class: &str) -> (Vec<Rule>, i64) {
    let i = rules.iter().position(|r| r.class == class).map_or(-1, |i| i as i64);
    (rules.iter().filter(|r| r.class != class).cloned().collect(), i)
}

/// WindowRules.restore: put it back where it was
pub fn restore(rules: &[Rule], rule: &Rule, index: i64) -> Vec<Rule> {
    let mut list: Vec<Rule> = rules.iter().filter(|r| r.class != rule.class).cloned().collect();
    let at = index.clamp(0, list.len() as i64) as usize;
    list.insert(at, rule.clone());
    list
}

/// The size fields: both empty (no size), or whole numbers in range
pub fn size_of(w: &str, h: &str) -> Result<(i64, i64), &'static str> {
    let (w, h) = (w.trim(), h.trim());
    if w.is_empty() && h.is_empty() {
        return Ok((0, 0));
    }
    if w.is_empty() || h.is_empty() {
        return Err("Enter both a width and a height, or leave both empty.");
    }
    match (w.parse::<i64>(), h.parse::<i64>()) {
        (Ok(w), Ok(h)) if (200..=7680).contains(&w) && (150..=4320).contains(&h) => Ok((w, h)),
        _ => Err("Enter whole numbers: a width from 200 to 7680 and a height from 150 to 4320 pixels."),
    }
}

fn rules_of(cx: &Ctx) -> Vec<Rule> {
    parse(&cx.list("windowRules.rules"))
}

fn save_rules(cx: &mut Ctx, rules: &[Rule]) {
    cx.set("windowRules.rules", save(rules));
}

fn change(cx: &mut Ctx, patch: Value) {
    let (class, name) = (cx.local_s("wr.class"), cx.local_s("wr.name"));
    let list = set(&rules_of(cx), &class, &name, &patch);
    save_rules(cx, &list);
}

fn edit(cx: &mut Ctx, class: &str, name: &str) {
    cx.local_set("wr.class", class);
    cx.local_set("wr.name", name);
    cx.local_set("wr.view", "edit");
    cx.local.remove("wr.sizeError");
}

/// An app in the lists: its class, name and icon class
#[derive(Clone, Debug, PartialEq)]
pub struct App {
    pub cls: String,
    pub name: String,
}

/// An app row (AppRow): icon, name, detail, chevron; the whole row opens it
fn app_row(cx: &mut Ctx, cls: &str, name: &str, detail: String, first: bool, last: bool) -> Row {
    let (c, n) = (cls.to_string(), name.to_string());
    let id = cx.on(move |cx, _| edit(cx, &c, &n));
    let icon = app_icon(cx, cls);
    let mut r = Row::custom("notify", "item").id(id);
    r.label = name.to_string();
    r.detail = detail;
    r.text = "chevron".into();
    r.index = place(first, last);
    r.from = 48.0;
    r.to = 64.0;
    if let Some(i) = icon {
        r.num = 1.0;
        r.image = Some(i);
    }
    r
}

/// Does an app match the picker's search?
pub fn matches(a: &App, q: &str) -> bool {
    q.is_empty() || a.name.to_lowercase().contains(q) || a.cls.to_lowercase().contains(q)
}

/// Apps with windows open now, one per class (by name)
pub fn open_apps(classes: &[String], name_of: impl Fn(&str) -> Option<String>) -> Vec<App> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<App> = Vec::new();
    for c in classes {
        if c.is_empty() || c == "org.quickshell" || !seen.insert(c.clone()) {
            continue;
        }
        out.push(App { cls: c.clone(), name: name_of(c).unwrap_or_else(|| c.clone()) });
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

pub fn windowrules(cx: &mut Ctx, p: &mut Page) {
    cx.want("clients");
    cx.want("apps");
    let view = match cx.local_s("wr.view").as_str() {
        "pick" => "pick",
        "edit" => "edit",
        _ => "list",
    };
    let rules = rules_of(cx);
    let floating = cx.s("windowRules.defaultLayout") == "floating";

    // New windows: tiled or floating, for every app
    if view == "list" {
        let layouts = ["tiled", "floating"];
        let layout = cx.s("windowRules.defaultLayout");
        let lp = cx.pick("windowRules.defaultLayout", &layouts);
        p.group("")
            .footer(if floating {
                "Every app opens as a free window; dialogs too. To tile an app anyway, add a rule for it and turn on Tile."
            } else {
                "Apps share the screen side by side; dialogs and pickers still float. To float an app anyway, add a rule for it and turn on Float."
            })
            .row(Row::new("New windows open").detail("Changing it also re-arranges the windows on the spaces you can see").segmented(&["Tiled", "Floating"], idx(&layouts, &layout), lp).width(220.0));
    }

    // Errors (the shell couldn't write the rules file)
    let error = feed(cx)["rulesError"].as_str().unwrap_or("").to_string();
    if !error.is_empty() {
        p.group("").row(Row::new("Rules weren't saved").detail(error));
    }

    match view {
        "list" => list_view(cx, p, &rules),
        "pick" => pick_view(cx, p, &rules),
        _ => edit_view(cx, p, &rules, floating),
    }

    // ── Parts: other pages shown here ──
    p.part(cx, "Windows, Stages and Overview", windows);
}

fn list_view(cx: &mut Ctx, p: &mut Page, rules: &[Rule]) {
    // Undo for the last removed rule (8 s)
    let removed = cx.local.get("wr.removed").cloned().unwrap_or(Value::Null);
    let at = cx.local.get("wr.removedAt").and_then(Value::as_i64).unwrap_or(0);
    if removed.is_object() && crate::util::now_ms() - at < 8000 {
        let rule = normalize(&removed["rule"]);
        let index = removed["index"].as_i64().unwrap_or(0);
        let r2 = rule.clone();
        let undo = cx.on(move |cx, _| {
            let list = restore(&rules_of(cx), &r2, index);
            save_rules(cx, &list);
            cx.local.remove("wr.removed");
        });
        p.group("").row(Row::new(format!("Removed the rule for {}", rule.name)).buttons(vec![Opt::new("Undo").icon("arrow-counter-clockwise")], undo));
    }

    let choose = cx.on(|cx, _| {
        cx.local_set("wr.view", "pick");
        cx.local_set("wr.query", "");
    });
    if rules.is_empty() {
        p.custom("notify", "empty").id(choose).data(vec![Opt::new("").icon("app-window")]);
        return;
    }
    p.custom("notify", "listhead").id(choose).text("Apps open the way you set here. Rules match the window class.");
    let n = rules.len();
    let rows: Vec<Row> = rules.iter().enumerate().map(|(i, r)| app_row(cx, &r.class, &r.name, r.summary(), i == 0, i + 1 == n)).collect();
    let g = p.group("");
    g.footer("Changes apply to windows opened after the change.");
    for r in rows {
        g.row(r);
    }
}

fn pick_view(cx: &mut Ctx, p: &mut Page, rules: &[Rule]) {
    let query = cx.local_s("wr.query").trim().to_lowercase();
    let shell = cx.shell;
    let apps = shell.get::<crate::services::apps::Apps>();
    let classes: Vec<String> = shell.get::<crate::services::clients::Windows>().map(|w| w.clients.iter().filter(|c| c.mapped).map(|c| c.class.clone()).collect()).unwrap_or_default();
    let open = open_apps(&classes, |c| apps.and_then(|a| a.entry_for(c)).map(|e| e.name.clone()));
    let mut installed: Vec<App> = Vec::new();
    if let Some(a) = apps {
        let mut seen = std::collections::HashSet::new();
        for e in &a.entries {
            let cls = if e.wm_class.is_empty() { e.id.clone() } else { e.wm_class.clone() };
            if e.no_display || cls.is_empty() || open.iter().any(|o| o.cls == cls) || !seen.insert(cls.clone()) {
                continue;
            }
            installed.push(App { cls, name: e.name.clone() });
        }
    }
    installed.sort_by_key(|a| a.name.to_lowercase());
    let open_shown: Vec<App> = open.iter().filter(|a| matches(a, &query)).cloned().collect();
    let matched: Vec<App> = installed.iter().filter(|a| matches(a, &query)).cloned().collect();
    const LIMIT: usize = 80;
    let shown: Vec<App> = matched.iter().take(LIMIT).cloned().collect();
    let typed = cx.local_s("wr.query").trim().to_string();

    // Back, and the search (Enter: the first result, or the text as a class)
    let first = open_shown.first().or(shown.first()).cloned();
    let search = cx.on(move |cx, a| match a.i {
        0 => cx.local_set("wr.view", "list"),
        1 => cx.local_set("wr.query", a.s.clone()),
        _ => {
            if let Some(f) = &first {
                edit(cx, &f.cls, &f.name);
            } else if !a.s.trim().is_empty() {
                let t = a.s.trim().to_string();
                edit(cx, &t, &t);
            }
        }
    });
    p.custom("notify", "search").id(search).text("Search apps, or type a window class");

    if apps.map_or(true, |a| a.entries.is_empty()) && open.is_empty() {
        p.custom("notify", "note").text("Loading apps…").num(3.0);
    }
    if !query.is_empty() && open_shown.is_empty() && matched.is_empty() {
        p.group("").row(Row::new(format!("No apps match “{typed}”")).detail(format!("Check the spelling, or press Enter to use “{typed}” as the window class. Open the app and it appears under Open now.")));
    }
    let detail = |a: &App| rules.iter().find(|r| r.class == a.cls).map_or(a.cls.clone(), Rule::summary);
    let n = open_shown.len();
    let rows: Vec<Row> = open_shown.iter().enumerate().map(|(i, a)| app_row(cx, &a.cls, &a.name, detail(a), i == 0, i + 1 == n)).collect();
    let g = p.group("Open now");
    for r in rows {
        g.row(r);
    }
    let n = shown.len();
    let rows: Vec<Row> = shown.iter().enumerate().map(|(i, a)| app_row(cx, &a.cls, &a.name, detail(a), i == 0, i + 1 == n)).collect();
    let g = p.group("Applications");
    if matched.len() > LIMIT {
        g.footer(format!("Showing {LIMIT} of {}. Type to narrow the list.", matched.len()));
    }
    for r in rows {
        g.row(r);
    }
}

fn edit_view(cx: &mut Ctx, p: &mut Page, rules: &[Rule], floating: bool) {
    let class = cx.local_s("wr.class");
    let name = cx.local_s("wr.name");
    let saved = rules.iter().find(|r| r.class == class).cloned();
    let rule = saved.clone().unwrap_or_else(|| Rule::new(&class, &name));

    // Back, the app, Remove rule
    let head = cx.on(|cx, a| {
        if a.i == 0 {
            cx.local_set("wr.view", "list");
            return;
        }
        let class = cx.local_s("wr.class");
        let rules = rules_of(cx);
        cx.local_set("wr.view", "list");
        let Some(r) = rules.iter().find(|r| r.class == class).cloned() else { return };
        let (list, index) = remove(&rules, &class);
        save_rules(cx, &list);
        cx.local_set("wr.removed", json!({ "rule": r.to_value(), "index": index }));
        cx.local_set("wr.removedAt", crate::util::now_ms());
        let key = format!("wr.undoTick.{}", crate::util::now_ms());
        cx.spawn(&key, || {
            std::thread::sleep(std::time::Duration::from_millis(8100));
            Value::Null
        });
    });
    let icon = app_icon(cx, &class);
    let b = p.custom("notify", "edithead");
    b.id(head).title(name.clone()).text(format!("Window class {class}"));
    b.on = saved.is_some();
    if let Some(i) = icon {
        b.num(1.0).image(i);
    }

    // When it opens (the opposite of what new windows do)
    let r = rule.clone();
    let float = cx.on(move |cx, _| change(cx, if r.pin { json!({ "float": false, "pin": false }) } else { json!({ "float": !r.float, "tile": false }) }));
    let tile_on = rule.tile;
    let tile = cx.on(move |cx, _| change(cx, json!({ "tile": !tile_on, "float": false, "pin": false })));
    let ws = rule.workspace;
    let space = cx.on(move |cx, a| {
        if a.i < 0 && ws > 0 {
            change(cx, json!({ "workspace": ws - 1 }));
        } else if a.i > 0 && ws < 10 {
            change(cx, json!({ "workspace": ws + 1 }));
        }
    });
    let starts = ["", "maximize", "fullscreen"];
    let start = cx.on(move |cx, a| {
        if let Some(v) = starts.get(a.i.max(0) as usize) {
            change(cx, json!({ "start": v }));
        }
    });
    let sm = rule.self_maximize;
    let selfmax = cx.on(move |cx, _| change(cx, json!({ "selfMaximize": !sm })));
    let pin_on = rule.pin;
    let pin = cx.on(move |cx, _| change(cx, json!({ "pin": !pin_on })));
    let mut stepper = Row::new("Open on space")
        .detail(if ws > 0 { format!("Opens on space {ws} and takes you there") } else { "Opens on the space you're on".into() })
        .ctl("notify", "stepper", space)
        .value(if ws > 0 { format!("Space {ws}") } else { "Any".into() });
    stepper.opts = vec![Opt::new("").off(ws <= 0), Opt::new("").off(ws >= 10)];
    p.group("When it opens")
        .row(Row::new("Float").visible(!floating).detail("Opens as a free window instead of tiling").switch(rule.float || rule.pin, float))
        .row(Row::new("Tile").visible(floating).detail("Tiles with the others, though new windows float").switch(rule.tile, tile))
        .row(stepper)
        .row(Row::new("Start as").segmented(&["Normal", "Maximized", "Fullscreen"], idx(&starts, &rule.start), start).width(280.0))
        .row(Row::new("Let the app maximize itself").detail("Apps that ask to be maximized when they open (or from their own button) normally aren't, so they don't cover your other windows. Super+F works either way.").switch(rule.self_maximize, selfmax))
        .row(Row::new("Show on all spaces").detail("Floats and stays in view when you switch spaces").switch(rule.pin, pin));

    let op = rule.opaque;
    let opaque = cx.on(move |cx, _| change(cx, json!({ "opaque": !op })));
    p.group("Look").row(Row::new("Always opaque").detail("Ignores window transparency (Appearance), like video and games").switch(rule.opaque, opaque));

    // Size: when it floats; both empty for the app's own
    let error = cx.local_s("wr.sizeError");
    let size = cx.on(|cx, a| {
        let (w, h) = a.s.split_once('\n').unwrap_or((a.s.as_str(), ""));
        match size_of(w, h) {
            Ok((w, h)) => {
                cx.local.remove("wr.sizeError");
                change(cx, json!({ "width": w, "height": h }));
            }
            Err(e) => cx.local_set("wr.sizeError", e),
        }
    });
    let mut size_row = Row::new("Default size").detail(if error.is_empty() { "Width × height, in pixels" } else { "" }).ctl("notify", "size", size);
    size_row.opts = vec![
        Opt::new(if rule.width > 0 { rule.width.to_string() } else { String::new() }),
        Opt::new(if rule.height > 0 { rule.height.to_string() } else { String::new() }),
    ];
    // The app's open window, for "Use current size"
    let shell = cx.shell;
    let current = shell.get::<crate::services::clients::Windows>().and_then(|w| w.clients.iter().find(|c| c.class == class && c.size.0 > 0).map(|c| c.size));
    let use_size = cx.on(move |cx, _| {
        if let Some((w, h)) = current {
            cx.local.remove("wr.sizeError");
            change(cx, json!({ "width": (w as i64).max(200), "height": (h as i64).max(150) }));
        }
    });
    let g = p.group("Size");
    g.footer("Used when the window floats. Leave both empty for the app's own size.").row(size_row);
    if let Some((w, h)) = current {
        g.row(Row::new("Current window").detail(format!("{w} × {h}")).button("Use current size", use_size));
    }
    if !error.is_empty() {
        p.custom("notify", "note").text(error).num(2.0).above(6.0);
    }
    p.custom("notify", "note").text(format!("Changes apply to {name} windows opened from now on.")).num(0.0);
}

/// Windows, Stages and Overview (appearance-windows.qml)
pub fn windows(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx).clone();
    let plugin = f["decorationsPlugin"].as_bool().unwrap_or(false);
    let controls = f["pluginControls"].as_bool().unwrap_or(false);
    let glass = f["glassPlugin"].as_bool().unwrap_or(false);
    let deco = cx.s("appearance.decorations");
    let gaps = ["none", "compact", "comfortable"];
    let gap = cx.s("appearance.gaps");
    let gp = cx.pick("appearance.gaps", &gaps);
    let decos = ["none", "controls", "titlebars"];
    let dp = cx.pick("appearance.decorations", &decos);
    p.group("Windows")
        .footer(if deco == "titlebars" && !plugin {
            "Title bars need the hyprbars plugin, which isn't installed."
        } else if deco == "controls" {
            if controls {
                "Hover the handle at the top of any window for close, minimize and zoom; rest on the green button to tile. Drawn by the compositor, so it moves with the window."
            } else {
                "Hover the handle at the top of the focused window for close, minimize and zoom. With the compositor glass plugin it's drawn on every window and never lags behind."
            }
        } else if deco == "titlebars" {
            "macOS-style title bars with traffic lights. Double-click a title bar to zoom."
        } else {
            "Borderless windows; use the Window menu for window actions."
        })
        .row(Row::new("Gaps").detail("Space round windows: between them and at the screen's edges").segmented(&["None", "Compact", "Comfortable"], idx(&gaps, &gap), gp).width(300.0))
        .row(Row::new("Decorations").segmented(&["None", "Window controls", "Title bars"], idx(&decos, &deco), dp).width(300.0));

    // (one choice: off, or on with or without the recent stages)
    let on = cx.b("stageManager.enabled");
    let strip = cx.b("stageManager.strip");
    let now = if !on { "off" } else if strip { "recents" } else { "on" };
    let stages = ["off", "on", "recents"];
    let sp = cx.on(move |cx, a| {
        let v = stages.get(a.i.max(0) as usize).copied().unwrap_or("off");
        if v != "off" {
            cx.set("stageManager.strip", v == "recents");
        }
        cx.set("stageManager.enabled", v != "off");
    });
    p.group("Stages")
        .footer("Keeps one group of windows in front and the others as thumbnails on the left. A new app opens in its own stage; Shift-click a thumbnail to add its windows to the stage in front. Also in Quick Settings.")
        .row(Row::new("Stages")
            .detail(if !on { "" } else if strip { "Recent stages wait on the left" } else { "Windows get the full width; switch stages with the Dock or All Windows" })
            .segmented(&["Off", "On", "On with recents"], idx(&stages, now), sp)
            .width(300.0));

    let styles = ["grid", "macos"];
    let style = cx.s("overview.style");
    let op = cx.pick("overview.style", &styles);
    p.group("Overview")
        .footer("Window previews apply to the Overview, All Windows and the Stages strip. Live previews update up to 30 times a second and use more power while those are open; nothing is captured while they're closed.")
        .row(Row::new("Style")
            .detail(if style == "macos" { "Your spaces in a strip on top, a search in the middle" } else { "Your spaces as large previews, with a search on top" })
            .segmented(&["Grid", "Strip"], idx(&styles, &style), op)
            .width(260.0));

    // ── More options ──
    p.more(cx);
    {
        let opac = ["off", "subtle", "more", "custom"];
        let o = cx.s("appearance.windowOpacity");
        let opk = cx.pick("appearance.windowOpacity", &opac);
        let mut rows = vec![Row::new("Window transparency")
            .detail(if o == "off" { "Windows are fully opaque" } else { "Video, games and fullscreen stay opaque" })
            .segmented(&["Off", "Subtle", "More", "Custom"], idx(&opac, &o), opk)
            .width(300.0)];
        if o == "custom" {
            for (key, label) in [("appearance.windowOpacityActive", "Focused window"), ("appearance.windowOpacityInactive", "Other windows")] {
                let v = cx.f(key);
                let n = cx.num(key, 0.01);
                rows.push(Row::new(label).detail(format!("{}% opaque", (v * 100.0).round())).slider(v, 0.6, 1.0, 0.0, n).width(220.0));
            }
        }
        let tabs = ["hyprland", "finder"];
        let t = cx.s("appearance.windowTabs");
        let tp = cx.pick("appearance.windowTabs", &tabs);
        rows.push(
            Row::new("Window tabs")
                .detail(if t == "finder" { format!("Tabs with icons, drawn by Tessera{}", if glass { "" } else { " (needs the glass plugin to know the front tab)" }) } else { "Hyprland's tab bar, restyled; smoothest".into() })
                .segmented(&["Hyprland", "Tessera"], idx(&tabs, &t), tp)
                .width(300.0),
        );
        let install = cx.on(|cx, _| cx.ask("notify", "installHyprbars", Value::Null));
        rows.push(Row::new("hyprbars plugin").visible(deco == "titlebars" && !plugin).detail("Builds the official plugin with hyprpm in a terminal.").button("Install…", install).primary());
        let g = p.group("Windows");
        for r in rows {
            g.row(r);
        }

        let disp = ["rows", "each"];
        let d = cx.s("overview.displays");
        let dpk = cx.pick("overview.displays", &disp);
        let walls = sw(cx, "overview.wallpapers", "Show the wallpaper in spaces", "Behind the windows in each space's preview (Overview and All Windows)");
        let prev = ["snapshot", "hybrid", "live"];
        let pv = cx.s("overview.previews");
        let ppk = cx.pick("overview.previews", &prev);
        p.group("Overview")
            .row(Row::new("With several displays")
                .detail(if d == "each" { "The Overview opens on every screen, each showing its own spaces" } else { "A row for each display, in the order they're arranged" })
                .segmented(&["A row per display", "Each its own"], idx(&disp, &d), dpk)
                .width(260.0))
            .row(walls)
            .row(Row::new("Window previews")
                .detail(match pv.as_str() {
                    "live" => "Every window keeps updating, up to 30 frames a second",
                    "hybrid" => "Windows on the current space keep updating (up to 30 frames a second); the rest are pictures",
                    _ => "A picture of each window, taken when the Overview opens",
                })
                .segmented(&["Snapshot", "Hybrid", "Live"], idx(&prev, &pv), ppk)
                .width(260.0));
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_read_as_the_shell_reads_them() {
        let list = vec![
            r#"{"class":"firefox","float":1,"workspace":"3","pin":false,"tile":true}"#.to_string(),
            "not json".to_string(),
            r#"{"name":"no class"}"#.to_string(),
            r#"{"class":"mpv","start":"weird","width":-5,"opaque":true}"#.to_string(),
        ];
        let r = parse(&list);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].name, "firefox");
        assert!(r[0].float && !r[0].tile);
        assert_eq!(r[0].workspace, 3);
        assert_eq!(r[1].start, "");
        assert_eq!(r[1].width, 0);
        assert_eq!(r[0].summary(), "Floats · Space 3");
        assert_eq!(r[1].summary(), "Opaque");
        assert_eq!(Rule::new("x", "").summary(), "Nothing set");
    }

    #[test]
    fn saved_as_json_strings_in_the_shells_order() {
        let r = Rule::new("foot", "Foot");
        let s = save(&[r]);
        assert_eq!(s[0].as_str().unwrap(), r#"{"class":"foot","name":"Foot","float":false,"tile":false,"workspace":0,"opaque":false,"start":"","pin":false,"width":0,"height":0,"selfMaximize":false}"#);
    }

    #[test]
    fn set_remove_restore() {
        let l = set(&[], "a", "A", &json!({"float": true}));
        let l = set(&l, "b", "B", &json!({"pin": true}));
        assert!(l[0].float && l[1].pin);
        let l = set(&l, "a", "A", &json!({"workspace": 2}));
        assert_eq!((l[0].workspace, l[0].float), (2, true));
        let (rest, i) = remove(&l, "a");
        assert_eq!((rest.len(), i), (1, 0));
        let back = restore(&rest, &l[0], i);
        assert_eq!(back[0].class, "a");
        assert_eq!(remove(&l, "zz").1, -1);
    }

    #[test]
    fn sizes_are_checked() {
        assert_eq!(size_of("", " "), Ok((0, 0)));
        assert!(size_of("800", "").is_err());
        assert_eq!(size_of("800", "600"), Ok((800, 600)));
        assert!(size_of("100", "600").is_err());
        assert!(size_of("800.5", "600").is_err());
    }

    #[test]
    fn the_picker_lists_each_class_once() {
        let open = open_apps(&["foot".into(), "org.quickshell".into(), "foot".into(), "".into(), "Alacritty".into()], |c| (c == "foot").then(|| "Foot".to_string()));
        assert_eq!(open.iter().map(|a| a.cls.as_str()).collect::<Vec<_>>(), ["Alacritty", "foot"]);
        assert!(matches(&open[1], "foo") && !matches(&open[1], "zz") && matches(&open[1], ""));
    }
}
