//! Hyprland from the host's side: dispatches and `hyprctl eval` in order on
//! one worker (the QML's Hyprland.dispatch / Quickshell.execDetached ran one
//! after another too, and a move followed by a focus must stay in that
//! order), fresh lists of windows and displays, the config file and the
//! shell's state file (UiState.data, as the QML services write them).

use serde_json::Value;
use tessera_core::services::clients::Client;
use tessera_core::services::hypr;

fn quiet(argv: &[String]) {
    if argv.is_empty() {
        return;
    }
    let _ = std::process::Command::new(&argv[0]).args(&argv[1..]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

/// Hyprland.dispatch("hl.dsp…"), in order with every other (hypr::IN_ORDER)
pub fn dispatch(cmd: impl Into<String>) {
    hypr::dispatch(cmd);
}
/// `hyprctl eval <lua>` (Quickshell.execDetached), in order with the dispatches
pub fn eval(lua: impl Into<String>) {
    let lua = lua.into();
    hypr::IN_ORDER.run(move || quiet(&["hyprctl".into(), "eval".into(), lua]));
}
/// A command, after what was queued before it
pub fn run(argv: Vec<String>) {
    hypr::IN_ORDER.run(move || quiet(&argv));
}
/// A command on a thread of its own (it may take long)
pub fn spawn(argv: Vec<String>) {
    if argv.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let _ = std::process::Command::new(&argv[0]).args(&argv[1..]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    });
}

/// "0x…" for any address
pub fn addr(a: &str) -> String {
    if a.starts_with("0x") {
        a.to_string()
    } else {
        format!("0x{a}")
    }
}
/// An address as Hyprland's events say it: no "0x", lower case
pub fn key(a: &str) -> String {
    a.trim().trim_start_matches("0x").to_lowercase()
}
/// A Lua string literal
pub fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}
/// Apps.windowActionFor: `hl.dsp.window.<name>({ k = v, …, window = "address:0x…" })`;
/// the values are Lua already (strings quoted with `q`)
pub fn window_action(address: &str, name: &str, fields: &[(&str, String)]) {
    let mut parts: Vec<String> = fields.iter().map(|(k, v)| format!("{k} = {v}")).collect();
    parts.push(format!("window = {}", q(&format!("address:{}", addr(address)))));
    dispatch(format!("hl.dsp.window.{name}({{ {} }})", parts.join(", ")));
}
pub fn focus_window(address: &str) {
    dispatch(format!("hl.dsp.focus({{ window = {} }})", q(&format!("address:{}", addr(address)))));
}

/// What Hyprland lists right now (asked on a thread, never on the loop)
#[derive(Default, Clone, Debug)]
pub struct Fresh {
    pub clients: Vec<Client>,
    /// raw `j/monitors` and `j/workspaces` (reserved, description, model…)
    pub monitors: Vec<Value>,
    pub workspaces: Vec<Value>,
    /// the window in front ("0x…", "" none)
    pub active: String,
}

impl Fresh {
    pub fn client(&self, address: &str) -> Option<&Client> {
        let k = key(address);
        self.clients.iter().find(|c| key(&c.address) == k)
    }
    pub fn focused_ws(&self) -> i32 {
        self.monitors.iter().find(|m| m["focused"] == true).or(self.monitors.first()).and_then(|m| m["activeWorkspace"]["id"].as_i64()).unwrap_or(1) as i32
    }
    pub fn focused_monitor(&self) -> Option<&Value> {
        self.monitors.iter().find(|m| m["focused"] == true).or(self.monitors.first())
    }
}

fn get(path: &str) -> Option<Value> {
    serde_json::from_str(&hypr::request(path).ok()?).ok()
}

pub fn fetch() -> Fresh {
    let clients = get("j/clients").and_then(|v| serde_json::from_value::<Vec<Client>>(v).ok()).unwrap_or_default();
    let monitors = get("j/monitors").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let workspaces = get("j/workspaces").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let active = get("j/activewindow").and_then(|v| v["address"].as_str().map(String::from)).unwrap_or_default();
    Fresh { clients, monitors, workspaces, active }
}

// ── files ───────────────────────────────────────────────────────────────────

/// Tessera's config folder (Config.dir)
pub fn config_dir() -> std::path::PathBuf {
    tessera_core::theme::config_path().parent().map(|p| p.to_path_buf()).unwrap_or_default()
}

/// config.json: `section.key = value` (Config.options.section.key = …)
pub fn set_cfg(section: &str, key: &str, value: Value) {
    // (from the loop: queued, never waited on; read as set at once)
    let (s, k) = (section.to_string(), key.to_string());
    tessera_core::cfgfile::edit_config_later(move |m| {
        let sec = m.entry(s.clone()).or_insert_with(|| serde_json::json!({}));
        if !sec.is_object() {
            *sec = serde_json::json!({});
        }
        if sec[&k] == value {
            return false;
        }
        sec[&k] = value.clone();
        true
    });
}

pub fn state_path() -> std::path::PathBuf {
    tessera_core::util::state_dir().join("state.json")
}
/// state.json (UiState.data): one key
pub fn state_get(key: &str) -> Value {
    tessera_core::cfgfile::state_get(key)
}
/// state.json: `data.key = value` (under state-merge.py's lock)
pub fn state_set(key: &str, value: Value) {
    tessera_core::cfgfile::set_state(key, value);
}

/// A script of the shell's (shell/assets/scripts/…)
pub fn script(name: &str) -> Option<std::path::PathBuf> {
    tessera_core::services::apps::shell_file(&format!("shell/assets/scripts/{name}"))
}

// ── the glass plugin's commands ─────────────────────────────────────────────

/// `<plugin> <args>` on the plugin's command socket (it answers to
/// "tessera-glass", or "irice-glass" if built before the rename); false when
/// no plugin answers. Blocking: call it on a thread.
pub fn glass_command(args: &str) -> bool {
    for n in ["tessera-glass", "irice-glass"] {
        if hypr::request(&format!("{n} ping")).is_ok_and(|a| a.starts_with(n)) {
            return hypr::request(&format!("{n} {args}")).is_ok();
        }
    }
    false
}
/// Whether a glass plugin answers (blocking)
pub fn glass_available() -> bool {
    ["tessera-glass", "irice-glass"].iter().any(|n| hypr::request(&format!("{n} ping")).is_ok_and(|a| a.starts_with(n)))
}
