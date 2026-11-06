//! WindowRules.qml: per-app window rules (Settings → Window Rules): float,
//! open on a space, always opaque, start maximized or fullscreen, show on all
//! spaces, default size, matched by window class; stored in
//! windowRules.rules (one JSON string per app). Applied three ways: written
//! to <config>/window-rules.lua (hyprland.lua loads it when Hyprland starts),
//! live with `hyprctl eval` (each app's rule keeps its handle in a Lua table
//! so a changed rule replaces the old one; when a rule loses an effect and
//! the hook is in the Hyprland config, the config is reloaded instead), and
//! again after a config reload. Also the default layout (new windows tiled or
//! floating) and "ignore a window's own maximize request".

use super::hx;
use super::Io;
use serde_json::Value;
use std::collections::BTreeMap;
use tessera_core::host::Host;
use tessera_core::services::hypr;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rule {
    pub class: String,
    pub name: String,
    pub float: bool,
    pub tile: bool,
    pub workspace: u32,
    pub opaque: bool,
    /// "" | "maximize" | "fullscreen"
    pub start: String,
    pub pin: bool,
    pub width: u32,
    pub height: u32,
    pub self_maximize: bool,
}

fn to_int(v: &Value) -> u32 {
    let n = match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    };
    if n.is_finite() && n > 0.0 { n as u32 } else { 0 }
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

pub fn normalize(r: &Value) -> Rule {
    let class = r["class"].as_str().unwrap_or("").to_string();
    let name = r["name"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| class.clone());
    let (float, pin) = (truthy(&r["float"]), truthy(&r["pin"]));
    Rule {
        name,
        float,
        tile: truthy(&r["tile"]) && !float && !pin,
        workspace: to_int(&r["workspace"]),
        opaque: truthy(&r["opaque"]),
        start: match r["start"].as_str() {
            Some(s @ ("maximize" | "fullscreen")) => s.to_string(),
            _ => String::new(),
        },
        pin,
        width: to_int(&r["width"]),
        height: to_int(&r["height"]),
        self_maximize: truthy(&r["selfMaximize"]),
        class,
    }
}

/// The rules in the settings
pub fn rules_of(cfg: &Value) -> Vec<Rule> {
    cfg["windowRules"]["rules"]
        .as_array()
        .map(|a| a.iter().filter_map(|s| s.as_str()).filter_map(|s| serde_json::from_str::<Value>(s).ok()).filter(|r| r.is_object() && truthy(&r["class"])).map(|r| normalize(&r)).collect())
        .unwrap_or_default()
}

pub fn has_effects(r: &Rule) -> bool {
    r.float || r.tile || r.workspace > 0 || r.opaque || !r.start.is_empty() || r.pin || (r.width > 0 && r.height > 0) || r.self_maximize
}

fn lua_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}
fn escape(cls: &str) -> String {
    let mut o = String::new();
    for c in cls.chars() {
        if ".*+?^${}()|[]\\".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}
fn class_pattern(cls: &str) -> String {
    format!("^({})$", escape(cls))
}

/// The hl.window_rule spec for a rule, or "nil" when it does nothing
pub fn spec(r: Option<&Rule>) -> String {
    let Some(r) = r else { return "nil".into() };
    let without_self = Rule { self_maximize: false, ..r.clone() };
    if !has_effects(r) || (r.self_maximize && !has_effects(&without_self)) {
        return "nil".into();
    }
    let mut f = vec![format!("name = {}", lua_str(&format!("tessera-window-rules:{}", r.class))), format!("match = {{ class = {} }}", lua_str(&class_pattern(&r.class)))];
    if r.float || r.pin {
        f.push("float = true".into()); // (pinning only works on floating windows)
    } else if r.tile {
        f.push("tile = true".into());
    }
    if r.workspace > 0 {
        f.push(format!("workspace = {}", lua_str(&r.workspace.to_string())));
    }
    if r.opaque {
        f.push("opacity = \"1.0 override 1.0 override\"".into());
    }
    if r.start == "fullscreen" {
        f.push("fullscreen = true".into());
    } else if r.start == "maximize" {
        f.push("maximize = true".into());
    }
    if r.pin {
        f.push("pin = true".into());
    }
    if r.width > 0 && r.height > 0 {
        f.push(format!("size = {}", lua_str(&format!("{} {}", r.width, r.height))));
    }
    format!("{{ {} }}", f.join(", "))
}

/// Replaces an app's rule: disables the one it had (if any), adds the new
/// (and, once, the rules made while Tessera was called iRice, under the old names)
const PRELUDE: &str = "if IRICE_WINDOW_RULES then for _, r in pairs(IRICE_WINDOW_RULES) do pcall(function() r:set_enabled(false) end) end IRICE_WINDOW_RULES = nil end\n\
for _, k in ipairs({ \"IRICE_DEFAULT_LAYOUT\", \"IRICE_NO_SELF_MAXIMIZE\" }) do local r = rawget(_G, k) if r then pcall(function() r:set_enabled(false) end) end _G[k] = nil end\n\
TESSERA_WINDOW_RULES = TESSERA_WINDOW_RULES or {}\n\
local function tessera_rule(key, spec)\n\
    local old = TESSERA_WINDOW_RULES[key]\n\
    if old then pcall(function() old:set_enabled(false) end) end\n\
    TESSERA_WINDOW_RULES[key] = spec and hl.window_rule(spec) or nil\n\
end\n";

/// New windows tiled or floating: one rule floats every window, except apps
/// whose rule says "tile" (left out of its match)
fn layout_spec(list: &[Rule]) -> String {
    let tiles: Vec<String> = list.iter().filter(|r| r.tile).map(|r| escape(&r.class)).collect();
    let m = if tiles.is_empty() { ".*".to_string() } else { format!("negative:^({})$", tiles.join("|")) };
    format!("{{ name = \"tessera-default-layout\", match = {{ class = {} }}, float = true }}", lua_str(&m))
}
/// Apps asking to be maximized covered every other window as they opened:
/// those requests are ignored, except for apps whose rule allows it
fn maximize_spec(list: &[Rule]) -> String {
    let allowed: Vec<String> = list.iter().filter(|r| r.self_maximize).map(|r| escape(&r.class)).collect();
    let m = if allowed.is_empty() { ".*".to_string() } else { format!("negative:^({})$", allowed.join("|")) };
    format!("{{ name = \"tessera-no-self-maximize\", match = {{ class = {} }}, suppress_event = \"maximize\" }}", lua_str(&m))
}
pub fn layout_lua(list: &[Rule], layout: &str) -> String {
    format!(
        "if TESSERA_DEFAULT_LAYOUT then pcall(function() TESSERA_DEFAULT_LAYOUT:set_enabled(false) end) end\nTESSERA_DEFAULT_LAYOUT = {}\nif TESSERA_NO_SELF_MAXIMIZE then pcall(function() TESSERA_NO_SELF_MAXIMIZE:set_enabled(false) end) end\nTESSERA_NO_SELF_MAXIMIZE = hl.window_rule({})\n",
        if layout == "floating" { format!("hl.window_rule({})", layout_spec(list)) } else { "nil".into() },
        maximize_spec(list)
    )
}

pub fn file_text(list: &[Rule], layout: &str) -> String {
    // (Loaded only when the config loads, when no earlier rule exists, so the handle table starts empty)
    let mut s = format!("-- Generated by Tessera from Settings → Window Rules; changes here are overwritten.\n-- Loaded from hyprland.lua: pcall(dofile, cfg .. \"/tessera/window-rules.lua\")\nTESSERA_WINDOW_RULES = {{}}\n{PRELUDE}{}", layout_lua(list, layout));
    for r in list {
        if has_effects(r) {
            s += &format!("tessera_rule({}, {})\n", lua_str(&r.class), spec(Some(r)));
        }
    }
    s
}

/// Each effect the old rule had is still there, unchanged
fn additive(a: Option<&Rule>, b: Option<&Rule>) -> bool {
    let Some(a) = a else { return true };
    let Some(b) = b else { return !has_effects(a) };
    let fx = |r: &Rule| -> [String; 6] {
        [
            if r.float || r.pin { "1".into() } else { String::new() },
            if r.tile { "1".into() } else { String::new() },
            if r.workspace > 0 { r.workspace.to_string() } else { String::new() },
            if r.opaque { "1".into() } else { String::new() },
            r.start.clone(),
            if r.pin { "1".into() } else { String::new() },
        ]
    };
    let size = |r: &Rule| if r.width > 0 && r.height > 0 { format!("{}x{}", r.width, r.height) } else { String::new() };
    let (x, y) = (fx(a), fx(b));
    for i in 0..x.len() {
        if !x[i].is_empty() && x[i] != y[i] {
            return false;
        }
    }
    let (sa, sb) = (size(a), size(b));
    sa.is_empty() || sa == sb
}

#[derive(Default)]
pub struct WindowRules {
    applied: BTreeMap<String, Rule>,
    written: BTreeMap<String, Rule>,
    started: bool,
    reset_table: bool,
    reload_seen: bool,
    hooked: bool,
    hook_checked: bool,
    writing: bool,
    layout_applied: String,
    last_layout: String,
    last_rules: Vec<Rule>,
    pub error: String,
}

fn layout_of(cfg: &Value) -> String {
    if cfg.is_null() {
        return String::new();
    }
    cfg["windowRules"]["defaultLayout"].as_str().unwrap_or("tiled").to_string()
}

impl WindowRules {
    fn file() -> std::path::PathBuf {
        hx::config_dir().join("window-rules.lua")
    }

    pub fn begin(&mut self, h: &Host, io: &mut Io) {
        self.last_rules = rules_of(h.cfg());
        self.check_hook(io);
        self.config_changed(h, io);
    }

    fn check_hook(&mut self, io: &Io) {
        let hypr_dir = std::env::var("XDG_CONFIG_HOME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{}/.config", std::env::var("HOME").unwrap_or_default())) + "/hypr";
        io.thread_then(
            move || {
                std::process::Command::new("sh")
                    .args(["-c", "grep -qsF window-rules.lua \"$1/hyprland.lua\" \"$1/custom.lua\"", "sh", &hypr_dir])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            },
            |wm, _h, io, found| {
                let r = &mut wm.rules;
                r.hooked = found || tessera_core::env::var("TTY").as_deref() == Some("1");
                r.hook_checked = true;
                if r.reload_seen {
                    r.reload_seen = false;
                    if r.hooked {
                        r.applied = r.written.clone(); // the reload loaded the file as last written
                    } else {
                        // A reload drops everything added with eval: add it all again, into a fresh handle table
                        r.applied.clear();
                        r.reset_table = true;
                    }
                }
                io.arm("rules.sync", 400);
            },
        );
    }

    pub fn config_changed(&mut self, h: &Host, io: &mut Io) {
        let cfg = h.cfg();
        if cfg.is_null() {
            return;
        }
        let rules = rules_of(cfg);
        let layout = layout_of(cfg);
        if rules != self.last_rules {
            self.last_rules = rules;
            io.arm("rules.sync", 400);
            self.apply_layout(h);
        }
        if layout != self.last_layout {
            self.apply_layout(h);
            io.arm("rules.sync", 400);
            if !self.last_layout.is_empty() {
                self.rearrange(h, &layout);
            }
            self.last_layout = layout;
        }
    }

    pub fn apply_layout(&mut self, h: &Host) {
        let layout = layout_of(h.cfg());
        if layout.is_empty() {
            return;
        }
        let lua = layout_lua(&rules_of(h.cfg()), &layout);
        if lua == self.layout_applied {
            return;
        }
        self.layout_applied = lua.clone();
        hx::eval(lua);
    }

    /// Switching the layout also re-arranges what's open on the spaces you can see
    fn rearrange(&mut self, h: &Host, layout: &str) {
        let rules = rules_of(h.cfg());
        let floating = layout == "floating";
        std::thread::spawn(move || {
            let get = |p: &str| hypr::request(p).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v.as_array().cloned());
            let (Some(clients), Some(mons)) = (get("j/clients"), get("j/monitors")) else { return };
            let rule_of = |cls: &str| rules.iter().find(|r| r.class == cls);
            let sel = |c: &Value| format!("address:{}", hx::addr(c["address"].as_str().unwrap_or("")));
            let mut lua = String::new();
            for m in &mons {
                let ws = m["activeWorkspace"]["id"].as_i64().unwrap_or(0);
                if ws <= 0 {
                    continue;
                }
                let mine: Vec<&Value> = clients.iter().filter(|c| c["workspace"]["id"].as_i64() == Some(ws) && c["mapped"] != false && c["fullscreen"].as_i64().unwrap_or(0) == 0 && c["pinned"] != true).collect();
                let scale = m["scale"].as_f64().filter(|s| *s != 0.0).unwrap_or(1.0);
                let (mw, mh) = (m["width"].as_f64().unwrap_or(0.0) / scale, m["height"].as_f64().unwrap_or(0.0) / scale);
                if floating {
                    // Float the tiled ones, cascading from the centre at about two thirds of the screen
                    let (w, hh) = ((mw * 0.62).round(), (mh * 0.68).round());
                    let mut i = 0;
                    for c in mine {
                        if c["floating"] == true || rule_of(c["class"].as_str().unwrap_or("")).is_some_and(|r| r.tile) {
                            continue;
                        }
                        let x = (m["x"].as_f64().unwrap_or(0.0) + (mw - w) / 2.0 + ((i % 6) * 32) as f64).round();
                        let y = (m["y"].as_f64().unwrap_or(0.0) + (mh - hh) / 2.0 + ((i % 6) * 32) as f64).round();
                        let s = sel(c);
                        lua += &format!("hl.dispatch(hl.dsp.window.float({{ action = \"enable\", window = \"{s}\" }}))\nhl.dispatch(hl.dsp.window.resize({{ x = {w}, y = {hh}, window = \"{s}\" }}))\nhl.dispatch(hl.dsp.window.move({{ x = {x}, y = {y}, window = \"{s}\" }}))\n");
                        i += 1;
                    }
                } else {
                    // Tile the floating ones, except apps set to float and the small ones (dialogs, pickers…)
                    for c in mine {
                        let r = rule_of(c["class"].as_str().unwrap_or(""));
                        let (sw, sh) = (c["size"][0].as_i64().unwrap_or(0), c["size"][1].as_i64().unwrap_or(0));
                        if c["floating"] != true || r.is_some_and(|r| r.float || r.pin) || sw < 480 || sh < 360 {
                            continue;
                        }
                        lua += &format!("hl.dispatch(hl.dsp.window.float({{ action = \"disable\", window = \"{}\" }}))\n", sel(c));
                    }
                }
            }
            if !lua.is_empty() {
                hx::eval(lua);
            }
        });
    }

    /// One write for a burst of edits
    pub fn tick_sync(&mut self, h: &Host, io: &mut Io) {
        let cfg = h.cfg();
        if cfg.is_null() || cfg.get("windowRules").is_none() || !self.hook_checked {
            return;
        }
        if self.writing {
            io.arm("rules.sync", 400);
            return;
        }
        self.writing = true;
        let list = rules_of(cfg);
        let text = file_text(&list, &layout_of(cfg));
        let next: BTreeMap<String, Rule> = list.into_iter().map(|r| (r.class.clone(), r)).collect();
        let path = Self::file();
        io.thread_then(move || tessera_core::util::write_atomic(&path, text.as_bytes()).is_ok(), move |wm, _h, _io, ok| {
            let r = &mut wm.rules;
            r.writing = false;
            if !ok {
                r.error = format!("Couldn't save {}. Check that the folder is writable.", WindowRules::file().display());
                return;
            }
            r.error.clear();
            r.written = next.clone();
            r.apply(next);
        });
    }

    fn apply(&mut self, next: BTreeMap<String, Rule>) {
        // On shell start with the hook in place, Hyprland loaded the file itself; nothing to add
        if !self.started {
            self.started = true;
            if self.hooked {
                self.applied = next;
                return;
            }
        }
        let mut changed: Vec<String> = Vec::new();
        for (k, r) in &next {
            if spec(self.applied.get(k)) != spec(Some(r)) {
                changed.push(k.clone());
            }
        }
        for (k, r) in &self.applied {
            if !next.contains_key(k) && spec(Some(r)) != "nil" {
                changed.push(k.clone());
            }
        }
        if changed.is_empty() {
            return;
        }
        let exact = changed.iter().all(|k| additive(self.applied.get(k), next.get(k)));
        if !exact && self.hooked {
            // Something was turned off: reload, and the file brings exactly the current rules back
            hx::run(vec!["sh".into(), "-c".into(), "hyprctl reload config-only >/dev/null".into()]);
            self.applied = next;
            return;
        }
        let mut lua = String::new();
        if self.reset_table {
            lua += "TESSERA_WINDOW_RULES = {}\n";
        }
        self.reset_table = false;
        lua += PRELUDE;
        for k in &changed {
            lua += &format!("tessera_rule({}, {})\n", lua_str(k), spec(next.get(k)));
        }
        hx::eval(lua);
        self.applied = next;
    }

    pub fn event(&mut self, h: &Host, io: &mut Io, name: &str) {
        if name != "configreloaded" {
            return;
        }
        self.reload_seen = true;
        self.layout_applied.clear(); // a reload drops rules added with eval
        self.apply_layout(h);
        self.check_hook(io); // the hook may have just been added
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(v: Value) -> Rule {
        normalize(&v)
    }

    #[test]
    fn a_rule_becomes_a_lua_spec() {
        let r = rule(json!({"class": "foot", "float": true, "workspace": "3", "opaque": true, "width": 800, "height": 600}));
        let s = spec(Some(&r));
        assert!(s.contains("float = true") && s.contains("workspace = \"3\"") && s.contains("size = \"800 600\"") && s.contains("^(foot)$"));
        assert_eq!(spec(Some(&rule(json!({"class": "x"})))), "nil");
        assert_eq!(spec(None), "nil");
        // a rule that only lets the app maximize itself adds no per-app rule
        assert_eq!(spec(Some(&rule(json!({"class": "x", "selfMaximize": true})))), "nil");
    }

    #[test]
    fn tile_needs_a_free_layout_choice() {
        assert!(!rule(json!({"class": "x", "tile": true, "float": true})).tile);
        assert!(!rule(json!({"class": "x", "tile": true, "pin": true})).tile);
        assert_eq!(rule(json!({"class": "x", "start": "bogus"})).start, "");
    }

    #[test]
    fn turning_an_effect_off_is_not_additive() {
        let a = rule(json!({"class": "x", "float": true}));
        let b = rule(json!({"class": "x", "float": true, "opaque": true}));
        assert!(additive(Some(&a), Some(&b)));
        assert!(!additive(Some(&b), Some(&a)));
        assert!(additive(None, Some(&a)) && !additive(Some(&a), None));
    }

    #[test]
    fn the_layout_rule_leaves_tiling_apps_out() {
        let l = vec![rule(json!({"class": "a.b", "tile": true}))];
        assert!(layout_lua(&l, "floating").contains("negative:^(a\\\\.b)$"));
        assert!(layout_lua(&l, "tiled").contains("TESSERA_DEFAULT_LAYOUT = nil"));
    }
}
