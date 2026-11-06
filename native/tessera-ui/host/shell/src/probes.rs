//! What the system underneath is, and what is wrong with it: the QML services
//! Backends, Needs, ShellPriority, Conflicts, Health, Keybinds, HyprlandLink
//! and BugReport. Each looks at the system (through the same scripts, on
//! threads), keeps its answer in the shell's state for the pages and the menu
//! bar, and answers a few calls.
//!
//! State (set_state): "backends" {ready, found{…}, nmActive, netManager, wifiTitle,
//! wifiReason, wifiShort, hasWifiHardware, wifiLinkUp, wifiUnmanaged, ppd, powerShort,
//! powerReason, btState, btShort, btFix…}, plus the menu bar's "wifi" {elsewhere, short,
//! linkUp} and "power.ppd"; "needs"; "priority" {available, on, busy, error};
//! "conflicts" {found, fresh, checked}; "health" {checks, checking, checkedAt, fixing,
//! error, polkit}; "keybinds" {groups}; "hyprland.linked"; "bugreport".
//!
//! Calls: backends.recheck, needs.refresh / install / buildNative, priority.set [bool],
//! conflicts.check / announce / mute [ids] / unmute [id], health.check / fix [id] /
//! copyReport / setPolkit [state], keybinds.refresh, bugreport.start / gather / send [report]
//! / copy [report] / github [report]. `qs ipc call conflicts check | announce`.

use crate::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};

enum Msg {
    Stack(String),
    Needs(String),
    Priority(i32),
    PriorityJob(i32, String),
    Conflicts(String),
    Checks(Option<Value>),
    Fixed(String, Option<Value>),
    Binds(String),
    Link(Option<i64>),
    Gathered(Option<Value>),
    Sent(Option<Value>),
    Captured(bool),
    Tick(&'static str),
}

// ── Backends.qml ────────────────────────────────────────────────────────────

/// `key=value` lines of system-stack.sh
pub fn parse_stack(text: &str) -> BTreeMap<String, String> {
    text.lines().filter_map(|l| l.split_once('=').filter(|(k, _)| !k.is_empty())).map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn net_name(m: &str) -> &'static str {
    match m {
        "networkmanager" => "NetworkManager",
        "iwd" => "iwd",
        "connman" => "ConnMan",
        "wpa_supplicant" => "wpa_supplicant",
        "systemd-networkd" => "systemd-networkd",
        _ => "",
    }
}

fn wifi_app_name(a: &str) -> String {
    match a {
        "iwgtk" | "iwgui" | "impala" | "iwctl" | "connmanctl" | "wpa_gui" | "wpa_cli" => a.to_string(),
        "cmst" => "CMST".into(),
        "connman-gtk" => "connman-gtk".into(),
        other => other.to_string(),
    }
}

/// Backends' computed texts, from what system-stack.sh found
pub fn backends(found: &BTreeMap<String, String>, ready: bool) -> Value {
    let g = |k: &str| found.get(k).map(String::as_str).unwrap_or("");
    let net = g("net");
    let nm_active = net == "networkmanager";
    let manager = net_name(net);
    let wifi_unmanaged = ready && !nm_active;
    let has_wifi = !g("wifi_iface").is_empty();
    let nm_installed = g("nm_installed") == "1";
    let wifi_app = g("wifi_app");
    let wifi_title = if !wifi_unmanaged { String::new() } else if net == "networkmanager" { "Restart Tessera for Wi-Fi".into() } else if !manager.is_empty() { format!("Wi-Fi is managed by {manager}") } else { "No network service is running".into() };
    let wifi_reason = if !wifi_unmanaged {
        String::new()
    } else if net == "networkmanager" {
        "NetworkManager started after Tessera did. Restart Tessera (Settings → About) to use its Wi-Fi menus.".into()
    } else if !manager.is_empty() {
        format!("Tessera's Wi-Fi menus need NetworkManager. Use {} to choose a network.", if wifi_app.is_empty() { format!("{manager}'s own tools") } else { wifi_app_name(wifi_app) })
    } else if nm_installed {
        "Tessera's Wi-Fi menus need NetworkManager, which is installed but not running.".into()
    } else {
        "Tessera's Wi-Fi menus need NetworkManager. Install networkmanager, or set up another network service.".into()
    };
    let wifi_short = if !wifi_unmanaged { String::new() } else if !manager.is_empty() && net != "networkmanager" { format!("Managed by {manager}") } else { "Needs NetworkManager".into() };
    // energy modes
    let ppd = !ready || g("ppd") == "1";
    let power_daemon = g("powerd");
    let power_name = match power_daemon {
        "tlp" => "TLP",
        "auto-cpufreq" => "auto-cpufreq",
        "tuned" => "TuneD",
        "system76-power" => "system76-power",
        o => o,
    };
    let power_short = if ppd { String::new() } else if !power_daemon.is_empty() { format!("Managed by {power_name}") } else { "Needs power-profiles-daemon".into() };
    let power_reason = if ppd {
        String::new()
    } else if !power_daemon.is_empty() {
        format!("{power_name} manages power on this computer, so Tessera doesn't switch energy modes. Change them in {power_name}'s own settings.")
    } else if g("ppd_installed") == "1" {
        "Energy modes need power-profiles-daemon, which is turned off for good (masked). sudo systemctl unmask power-profiles-daemon turns it back on.".into()
    } else {
        "Energy modes need power-profiles-daemon. Install power-profiles-daemon to use Low Power and Performance.".into()
    };
    // Bluetooth: ok | no-adapter | not-installed | stopped | blocked | hard-blocked
    let bt = if !ready {
        ""
    } else if g("bt_hard") == "1" {
        "hard-blocked"
    } else if g("bt_soft") == "1" {
        "blocked"
    } else if g("bt_hardware") != "1" {
        "no-adapter"
    } else if g("bt_installed") != "1" {
        "not-installed"
    } else if g("bt_running") != "1" {
        "stopped"
    } else {
        "ok"
    };
    let (bt_title, bt_short) = match bt {
        "no-adapter" => ("No Bluetooth adapter", "No adapter"),
        "not-installed" => ("Bluetooth isn't installed", "Not installed"),
        "stopped" => ("Bluetooth is turned off", "Service off"),
        "blocked" => ("Bluetooth is blocked", "Blocked"),
        "hard-blocked" => ("Bluetooth is switched off", "Switched off"),
        _ => ("", ""),
    };
    let bt_reason = match bt {
        "no-adapter" => "No Bluetooth adapter found. If this computer has one, check that it's turned on in its firmware settings.",
        "not-installed" => "Install bluez and bluez-utils, then start Bluetooth (sudo systemctl enable --now bluetooth).",
        "stopped" => "The Bluetooth service isn't running. Start it to use Bluetooth devices.",
        "blocked" => "Bluetooth is blocked by rfkill, as in flight mode. Unblock it to use Bluetooth devices.",
        "hard-blocked" => "Bluetooth is switched off with a key or switch on this computer, or in its firmware settings. Turn it on there.",
        _ => "",
    };
    let bt_fix = match bt {
        "stopped" => "start",
        "blocked" => "unblock",
        _ => "",
    };
    json!({
        "ready": ready, "found": found, "nmActive": nm_active, "netManager": net, "netManagerName": manager,
        "hasWifiHardware": has_wifi, "wifiLinkUp": g("wifi_up") == "1", "wifiUnmanaged": wifi_unmanaged,
        "wifiTitle": wifi_title, "wifiReason": wifi_reason, "wifiShort": wifi_short, "nmInstalled": nm_installed,
        "canStartNm": wifi_unmanaged && net == "none" && nm_installed,
        "wifiApp": wifi_app, "wifiAppTerminal": g("wifi_app_terminal") == "1", "wifiAppName": wifi_app_name(wifi_app),
        "ppd": ppd, "ppdInstalled": g("ppd_installed") == "1", "powerDaemon": power_daemon, "powerShort": power_short, "powerReason": power_reason,
        "btState": bt, "btTitle": bt_title, "btReason": bt_reason, "btShort": bt_short, "btFix": bt_fix,
        "btFixLabel": match bt_fix { "start" => "Start Bluetooth", "unblock" => "Unblock Bluetooth", _ => "" }
    })
}

// ── Needs.qml ───────────────────────────────────────────────────────────────

const NEEDS_KEYS: &[&str] = &["avahiInstalled", "avahi", "pipewire", "networkManager", "secretTool", "keyring", "goa", "goaMicrosoft", "goaSignin"];

fn needs_script() -> String {
    [
        "command -v avahi-daemon >/dev/null && echo avahiInstalled",
        "{ systemctl is-active --quiet avahi-daemon 2>/dev/null || { test -S /run/avahi-daemon/socket && pgrep -x avahi-daemon >/dev/null; }; } && echo avahi",
        "timeout 3 pw-cli info 0 >/dev/null 2>&1 && echo pipewire",
        "{ systemctl is-active --quiet NetworkManager 2>/dev/null || pgrep -x NetworkManager >/dev/null; } && echo networkManager",
        "command -v secret-tool >/dev/null && echo secretTool",
        "{ busctl --user list 2>/dev/null; busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ListActivatableNames 2>/dev/null; } | grep -q org.freedesktop.secrets && echo keyring",
        "{ test -x /usr/lib/goa-daemon || test -x /usr/libexec/goa-daemon; } && echo goa",
        "v=$(pacman -Q gnome-online-accounts 2>/dev/null | awk '{print $2}'); [ -n \"$v\" ] && [ \"$(vercmp \"$v\" 3.50)\" -ge 0 ] && echo goaMicrosoft",
        "test -x \"$1/native/build/goa-signin\" || command -v tessera-goa-signin >/dev/null && echo goaSignin",
        ":",
    ]
    .join("; ")
}

// ── Keybinds.qml ────────────────────────────────────────────────────────────

pub fn pretty_key(k: &str) -> String {
    match k {
        "Return" => "Enter".into(),
        "space" | "Space" => "Space".into(),
        "Tab" => "Tab".into(),
        "Escape" => "Esc".into(),
        "Print" => "Print Screen".into(),
        "Period" | "period" => ".".into(),
        "Comma" => ",".into(),
        "Slash" => "/".into(),
        "mouse:272" => "Left Click".into(),
        "mouse:273" => "Right Click".into(),
        "mouse:274" => "Middle Click".into(),
        "SUPER_L" | "SUPER_R" => String::new(),
        "mouse_down" => "Scroll Down".into(),
        "mouse_up" => "Scroll Up".into(),
        k if k.starts_with("XF86") => {
            // "XF86AudioMute" -> "Audio Mute"
            let mut out = String::new();
            let mut prev_lower = false;
            for c in k[4..].chars() {
                if prev_lower && c.is_uppercase() {
                    out.push(' ');
                }
                prev_lower = c.is_lowercase();
                out.push(c);
            }
            out
        }
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => k.to_string(),
    }
}

/// "Go to space 1" … "Go to space 9" → one row "Go to space 1–9" with keys "1–9"
pub fn merge_numbered(binds: Vec<(Vec<String>, String)>) -> Vec<(Vec<String>, String)> {
    let mut out: Vec<(Vec<String>, String)> = Vec::new();
    let mut seen: BTreeMap<String, (usize, i64, i64)> = BTreeMap::new();
    for (keys, desc) in binds {
        let num = desc.rsplit_once(' ').and_then(|(a, n)| n.parse::<i64>().ok().map(|n| (a.to_string(), n)));
        let last = keys.last().cloned().unwrap_or_default();
        let Some((base, n)) = num.filter(|(_, n)| last == n.to_string()) else {
            out.push((keys, desc));
            continue;
        };
        let key = format!("{base}|{}", keys[..keys.len() - 1].join("+"));
        if let Some((idx, min, max)) = seen.get_mut(&key) {
            *max = (*max).max(n);
            *min = (*min).min(n);
            out[*idx].1 = format!("{base} {min}–{max}");
            let mut k = keys[..keys.len() - 1].to_vec();
            k.push(format!("{min}–{max}"));
            out[*idx].0 = k;
            continue;
        }
        seen.insert(key, (out.len(), n, n));
        out.push((keys, desc));
    }
    out
}

/// hyprctl -j binds → [{name, binds: [{keys, description}]}]; only binds with a
/// description, "Group: Action" under that group
pub fn bind_groups(binds: &Value) -> Value {
    const MODS: [(i64, &str); 4] = [(64, "Super"), (4, "Ctrl"), (8, "Alt"), (1, "Shift")];
    let mut order: Vec<String> = Vec::new();
    let mut by_group: BTreeMap<String, Vec<(Vec<String>, String)>> = BTreeMap::new();
    for b in binds.as_array().cloned().unwrap_or_default() {
        let desc = b["description"].as_str().unwrap_or("");
        if desc.is_empty() {
            continue;
        }
        let (group, what) = match desc.find(": ") {
            Some(i) if i > 0 => (desc[..i].to_string(), desc[i + 2..].to_string()),
            _ => ("General".to_string(), desc.to_string()),
        };
        let mask = b["modmask"].as_i64().unwrap_or(0);
        let mut keys: Vec<String> = MODS.iter().filter(|(bit, _)| mask & bit != 0).map(|(_, n)| n.to_string()).collect();
        let key = pretty_key(b["key"].as_str().unwrap_or(""));
        if !key.is_empty() {
            keys.push(key);
        }
        if !by_group.contains_key(&group) {
            order.push(group.clone());
        }
        let list = by_group.entry(group).or_default();
        if !list.iter().any(|(_, d)| *d == what) {
            list.push((keys, what));
        }
    }
    Value::Array(order.into_iter().map(|g| json!({"name": g, "binds": merge_numbered(by_group.remove(&g).unwrap_or_default()).into_iter().map(|(k, d)| json!({"keys": k, "description": d})).collect::<Vec<_>>()})).collect())
}

// ── Conflicts.qml ───────────────────────────────────────────────────────────

/// "waybar", "waybar and mako", "waybar, mako and hypridle"
pub fn names(list: &[Value]) -> String {
    let mut n: Vec<String> = Vec::new();
    for c in list {
        let name = c["name"].as_str().unwrap_or("").to_string();
        if !n.contains(&name) {
            n.push(name);
        }
    }
    match n.len() {
        0 => String::new(),
        1 => n[0].clone(),
        k => format!("{} and {}", n[..k - 1].join(", "), n[k - 1]),
    }
}

#[derive(Default)]
pub struct Probes {
    // backends
    stack: BTreeMap<String, String>,
    stack_ready: bool,
    stack_busy: bool,
    stack_again: bool,
    // conflicts
    conflicts: Vec<Value>,
    conflicts_checked: bool,
    announced: bool,
    // health
    health: Value,
    checking: bool,
    checked_at: i64,
    fixing: String,
    health_error: String,
    polkit: String,
    // priority
    priority: (bool, bool, bool, String),
    // keybinds
    drift: bool,
    link_asking: bool,
    // bug report
    bug: serde_json::Map<String, Value>,
    bug_report: Value,
    link_timer: Option<slint::Timer>,
}

fn py(sub: &[&str], input: Option<String>, then: impl FnOnce(Option<Value>) + Send + 'static) {
    let mut cmd = vec!["python3".to_string(), "-I".to_string(), script("bug-report.py").to_string_lossy().into_owned()];
    cmd.extend(sub.iter().map(|s| s.to_string()));
    std::thread::spawn(move || {
        use std::io::Write;
        let child = std::process::Command::new(&cmd[0]).args(&cmd[1..]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn();
        let Ok(mut child) = child else { return then(None) };
        if let Some(mut i) = child.stdin.take() {
            let _ = i.write_all(input.unwrap_or_default().as_bytes());
        }
        let out = child.wait_with_output();
        then(out.ok().and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok()))
    });
}

impl Probes {
    /// Tools.qml's `available`, for the surfaces that hide what needs one
    /// (tools.hyprpicker: the menu bar's colour picker)
    fn publish_tools(&self, h: &mut Host) {
        let mut m = serde_json::Map::new();
        for t in ["hyprpicker", "wf-recorder", "gpu-screen-recorder", "tesseract", "grim", "slurp", "swappy", "cliphist", "easyeffects", "warp-cli", "songrec", "wtype", "checkupdates", "hyprpm", "mangohud", "qrencode", "adb", "scrcpy", "ddcutil", "hyprsunset", "yt-dlp"] {
            m.insert(t.into(), json!(tessera_core::util::on_path(t)));
        }
        h.set_state("tools", Value::Object(m));
    }

    fn publish_stack(&self, h: &mut Host) {
        let b = backends(&self.stack, self.stack_ready);
        h.set_state("wifi", json!({"elsewhere": b["wifiUnmanaged"] == true && b["hasWifiHardware"] == true, "short": b["wifiShort"], "linkUp": b["wifiLinkUp"]}));
        h.set_state("power.ppd", b["ppd"].clone());
        h.set_state("backends", b);
    }

    fn recheck(&mut self, h: &mut Host) {
        if self.stack_busy {
            self.stack_again = true;
            return;
        }
        self.stack_busy = true;
        let send = h.sender("Probes");
        let script = script("system-stack.sh").to_string_lossy().into_owned();
        run_then(vec!["bash".into(), script], move |_, out| send(Box::new(Msg::Stack(out))));
    }

    fn check_conflicts(&mut self, h: &mut Host) {
        if !on_path("python3") {
            return;
        }
        let send = h.sender("Probes");
        run_then(vec!["python3".into(), "-I".into(), script("conflicts.py").to_string_lossy().into_owned()], move |_, out| send(Box::new(Msg::Conflicts(out))));
    }

    fn muted(&self) -> Vec<String> {
        ui_get("conflictsMuted").as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
    }

    fn fresh(&self) -> Vec<Value> {
        let muted = self.muted();
        self.conflicts.iter().filter(|c| !muted.iter().any(|m| c["id"].as_str() == Some(m))).cloned().collect()
    }

    /// The first look posts ONE island alert naming them, with Review (System
    /// health) and Don't show again (remembered per thing in state.json)
    fn announce(&mut self, h: &mut Host) {
        let list = self.fresh();
        if list.is_empty() {
            return;
        }
        let running: Vec<&Value> = list.iter().filter(|c| !c["id"].as_str().unwrap_or("").starts_with("unit:")).collect();
        let title = names(&list);
        let mut chars = title.chars();
        let title = chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default();
        let ids: Vec<Value> = list.iter().map(|c| c["id"].clone()).collect();
        let sub = if list.len() == 1 { list[0]["short"].as_str().unwrap_or("").to_string() } else if !running.is_empty() { "Also running, doing what Tessera does".into() } else { "Start with your session, doing what Tessera does".into() };
        h.ask("island.post", json!([{"kind": "custom", "icon": "warning", "title": title, "subtitle": sub, "color": "#FF9F0A", "action": "Review",
            "onAction": {"call": "panels.openSettings", "args": ["about/health"]},
            "more": [{"text": "Don't show again", "act": {"call": "conflicts.mute", "args": [ids]}}]}, 12000]));
    }

    fn publish_conflicts(&self, h: &mut Host) {
        h.set_state("conflicts", json!({"found": self.conflicts, "fresh": self.fresh(), "checked": self.conflicts_checked, "muted": self.muted()}));
    }

    fn publish_health(&self, h: &mut Host) {
        h.set_state("health", json!({"checks": self.health, "checking": self.checking, "checkedAt": self.checked_at, "fixing": self.fixing, "error": self.health_error, "polkit": self.polkit}));
    }

    fn health_args(&self, h: &Host) -> Vec<String> {
        let mut a = vec!["--polkit".to_string(), self.polkit.clone()];
        if h.state("google.connected").as_bool().unwrap_or(false) {
            a.push("--google".into());
        }
        a
    }

    fn health_check(&mut self, h: &mut Host) {
        if self.checking {
            return;
        }
        h.ask("tools.recheck", json!([])); // (something may have been installed since)
        self.checking = true;
        self.health_error.clear();
        self.recheck(h);
        self.check_conflicts(h);
        let mut cmd = vec!["python3".to_string(), "-I".into(), script("health-check.py").to_string_lossy().into_owned()];
        cmd.extend(self.health_args(h));
        let send = h.sender("Probes");
        run_then(cmd, move |_, out| send(Box::new(Msg::Checks(serde_json::from_str(&out).ok()))));
        self.publish_health(h);
    }

    fn priority_check(&self, h: &mut Host) {
        let send = h.sender("Probes");
        run_then(vec!["sh".into(), "-c".into(), "test -d /etc/ananicy.d || exit 2; test -f /etc/ananicy.d/tessera.rules || test -f /etc/ananicy.d/irice.rules".into()], move |code, _| send(Box::new(Msg::Priority(code))));
    }

    fn publish_priority(&self, h: &mut Host) {
        h.set_state("priority", json!({"available": self.priority.0, "on": self.priority.1, "busy": self.priority.2, "error": self.priority.3}));
    }

    fn bug_publish(&self, h: &mut Host) {
        h.set_state("bugreport", Value::Object(self.bug.clone()));
    }

    fn bug_set(&mut self, k: &str, v: Value) {
        self.bug.insert(k.into(), v);
    }
}

impl Service for Probes {
    fn name(&self) -> &'static str {
        "Probes"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["backends", "needs", "priority", "conflicts", "health", "keybinds", "bugreport", "tools", "shellpriority", "uistate"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["conflicts"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("hypr");
        self.polkit = if sandboxed() { "off".into() } else { "no".into() };
        self.health = json!([]);
        self.recheck(h);
        self.publish_tools(h);
        self.priority_check(h);
        // Needs: after the desktop is up (Startup.later)
        h.ask("startup.later", json!(["Probes", "needs"]));
        // Conflicts: 10 s after the shell starts (once the session's autostart has settled)
        let send = h.sender("Probes");
        slint::Timer::single_shot(std::time::Duration::from_secs(10), move || send(Box::new(Msg::Tick("conflicts"))));
        // Keybinds: read live from Hyprland
        self.read_binds(h);
        // HyprlandLink: after a quiet spell, ask Hyprland which space is active
        let send = h.sender("Probes");
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(20), move || send(Box::new(Msg::Tick("link"))));
        self.link_timer = Some(t);
        h.set_state("hyprland.linked", json!(true));
        self.bug_set("state", json!("idle"));
        self.publish_priority(h);
        self.publish_conflicts(h);
        self.publish_health(h);
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        // (other services: Startup's `{"startup": tag}`)
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                if v["startup"] == "needs" {
                    let send = h.sender("Probes");
                    run_then(vec!["sh".into(), "-c".into(), needs_script(), "sh".into(), shell_dir().to_string_lossy().into_owned()], move |_, out| send(Box::new(Msg::Needs(out))));
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Stack(out) => {
                self.stack = parse_stack(&out);
                self.stack_ready = true;
                self.stack_busy = false;
                self.publish_stack(h);
                if self.stack_again {
                    self.stack_again = false;
                    self.recheck(h);
                }
            }
            Msg::Needs(out) => {
                let has = |k: &str| out.lines().any(|l| l == k);
                let mut v = serde_json::Map::new();
                for k in NEEDS_KEYS {
                    v.insert((*k).into(), json!(has(k)));
                }
                v.insert("checked".into(), json!(true));
                h.set_state("needs", Value::Object(v));
            }
            Msg::Priority(code) => {
                self.priority.0 = code != 2;
                self.priority.1 = code == 0;
                self.publish_priority(h);
            }
            Msg::PriorityJob(code, err) => {
                self.priority.2 = false;
                self.priority.3 = if code == 64 {
                    "Tessera's system helper is from before this setting. Run Software Update once, then try again.".into()
                } else if code != 0 && code != 126 {
                    if code == 127 { "Not allowed (an administrator's password is needed).".into() } else { let e = err.replace("tessera-root: ", "").replace("irice-root: ", ""); if e.trim().is_empty() { "That didn't change. Update Tessera, then try again.".into() } else { e.trim().to_string() } }
                } else {
                    self.priority.3.clone()
                };
                self.publish_priority(h);
                self.priority_check(h);
            }
            Msg::Conflicts(out) => {
                self.conflicts = serde_json::from_str::<Value>(&out).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
                self.conflicts_checked = true;
                self.publish_conflicts(h);
                if !self.announced {
                    self.announced = true;
                    self.announce(h);
                }
            }
            Msg::Checks(v) => {
                self.checking = false;
                match v {
                    Some(c) => {
                        self.health = c;
                        self.checked_at = now_ms();
                    }
                    None => self.health_error = "The health check didn't answer properly. Check that python3 is installed.".into(),
                }
                self.publish_health(h);
            }
            Msg::Fixed(id, r) => {
                if let Some(r) = r.filter(|r| r["ok"] != true) {
                    self.health_error = r["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("That didn't work. Try the command shown instead.").to_string();
                }
                self.fixing.clear();
                self.checking = false;
                // (a fix ran; Backends looks again, and then the check runs to see whether it worked)
                self.recheck(h);
                h.ask("needs.refresh", json!([]));
                h.ask("health.check", json!([]));
                let _ = id;
                self.publish_health(h);
            }
            Msg::Binds(out) => {
                if let Ok(v) = serde_json::from_str::<Value>(&out) {
                    h.set_state("keybinds", json!({"groups": bind_groups(&v)}));
                }
            }
            Msg::Link(id) => {
                let ours = h.st.shell.get::<tessera_core::services::hypr::Snapshot>().and_then(|s| s.active.get(&s.focused).copied()).map(i64::from);
                let Some(real) = id else { return }; // (no answer: Hyprland is busy or going; nothing to compare)
                if ours == Some(real) {
                    // (in step again: events came, or the state was read anew)
                    self.drift = false;
                    h.set_state("hyprland.linked", json!(true));
                    return;
                }
                // Hyprland.refreshMonitors / refreshWorkspaces / refreshToplevels
                tessera_core::services::hypr::refresh();
                if !self.drift {
                    self.drift = true; // (read again; a second drift means the feed is gone)
                    eprintln!("HyprlandLink: space {real} was missed: reading the state again");
                    return;
                }
                // The event feed is gone (Hyprland drops a listener that falls behind): connect it
                // again, in place; the QML shell replaced itself (repair-shell.sh) here
                eprintln!("HyprlandLink: the event socket drifted again: reconnecting it");
                h.set_state("hyprland.linked", json!(false));
                tessera_core::services::hypr::reconnect();
                self.drift = false;
            }
            Msg::Gathered(v) => {
                self.bug_set("gathering", json!(false));
                self.bug_set("log", v.as_ref().map_or(json!(""), |v| v["log"].clone()));
                self.bug_set("sysinfo", v.as_ref().map_or(json!({}), |v| v["sysinfo"].clone()));
                self.bug_publish(h);
            }
            Msg::Sent(r) => {
                match r {
                    Some(r) if r["ok"] == true => {
                        self.bug_set("state", json!("sent"));
                        self.bug_set("issueUrl", r["issue_url"].clone());
                        self.bug_set("lastSent", json!(now_ms()));
                    }
                    other => {
                        self.bug_set("state", json!("error"));
                        self.bug_set("error", other.and_then(|r| r["error"].as_str().map(String::from)).map_or(json!("Couldn't send the report. Try again, or copy it or open it on GitHub instead."), |e| json!(e)));
                    }
                }
                self.bug_publish(h);
            }
            Msg::Captured(ok) => {
                self.bug_set("hasShot", json!(ok));
                self.bug_set("shotVersion", json!(self.bug.get("shotVersion").and_then(Value::as_i64).unwrap_or(0) + 1));
                h.ask("panels.openSettings", json!(["about/bugreport"]));
                self.bug_publish(h);
                self.bug_gather(h);
            }
            Msg::Tick(what) => match what {
                "conflicts" => self.check_conflicts(h),
                "recheck" => self.recheck(h),
                "link" => {
                    // (only after a quiet spell: events came meanwhile, nothing to check)
                    if tessera_core::services::hypr::quiet_ms() <= 30_000 {
                        self.drift = false;
                        return;
                    }
                    let send = h.sender("Probes");
                    std::thread::spawn(move || {
                        let id = capture(&["hyprctl", "-j", "activeworkspace"]).and_then(|o| serde_json::from_str::<Value>(&o).ok()).and_then(|v| v["id"].as_i64());
                        send(Box::new(Msg::Link(id)));
                    });
                }
                _ => {}
            },
        }
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let (prefix, func) = call.split_once('.').unwrap_or((call, ""));
        let a = if args.is_array() { args.clone() } else { json!([]) };
        match (prefix, func) {
            ("backends", "recheck") => self.recheck(h),
            // the System health fixes, by the names Backends and Needs had for them
            ("backends", "fix") => self.call(h, "health.fix", &json!([a[0].clone()])),
            ("backends", "startNm") => self.call(h, "health.fix", &json!(["networkmanager"])),
            ("needs", "turnOnAvahi") => {
                if h.state("needs.avahiInstalled").as_bool().unwrap_or(false) {
                    self.call(h, "health.fix", &json!(["avahi"]));
                } else {
                    h.ask("needs.install", json!([["avahi"]]));
                }
            }
            // Backends.fixBluetooth: start the service (a health fix), or rfkill unblock and look again
            ("backends", "fixBluetooth") => match backends(&self.stack, self.stack_ready)["btFix"].as_str().unwrap_or("") {
                "start" => self.call(h, "health.fix", &json!(["bluetooth"])),
                "unblock" => {
                    let send = h.sender("Probes");
                    run_then(vec!["rfkill".into(), "unblock".into(), "bluetooth".into()], move |_, _| send(Box::new(Msg::Tick("recheck"))));
                }
                _ => {}
            },
            // the Wi-Fi tool of a computer without NetworkManager: in a terminal, or on its own
            ("backends", "openWifiApp") => {
                let app = self.stack.get("wifi_app").cloned().unwrap_or_default();
                if !app.is_empty() {
                    h.ask("panels.closeOpen", json!([]));
                    if self.stack.get("wifi_app_terminal").map(String::as_str) == Some("1") {
                        launch_terminal(&[app.as_str()], false);
                    } else {
                        launch_run(&[app.as_str()], "run");
                    }
                }
            }
            ("needs", "refresh") => h.ask("startup.later", json!(["Probes", "needs"])),
            // Software Update's job, then a fresh look
            ("needs", "install") => {
                let pk: Vec<String> = a[0].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                if !pk.is_empty() {
                    h.ask("updates.update", json!(["--install", pk.join(",")]));
                }
            }
            ("needs", "buildNative") => run_owned(vec![script("../native/build.sh").to_string_lossy().into_owned()]),
            // UiState.data.<key> = value, for the other crates (System's caffeine: [key, value])
            ("uistate", "set") => {
                if let Some(k) = a[0].as_str() {
                    ui_set(k, a[1].clone());
                }
            }
            // Tools.qml's probe is a PATH look-up every time here (nothing is kept): nothing to redo
            ("tools", "recheck") => self.publish_tools(h),
            // (Settings -> About's switch asks it by the QML service's name)
            ("priority" | "shellpriority", "set") => {
                if self.priority.2 {
                    return;
                }
                self.priority.3.clear();
                if !on_path("pkexec") {
                    self.priority.3 = "Changing this needs polkit (pkexec), which isn't installed. Install polkit, then try again.".into();
                    self.publish_priority(h);
                    return;
                }
                self.priority.2 = true;
                let on = a[0].as_bool().unwrap_or(false);
                let send = h.sender("Probes");
                let cmd = vec!["sh".into(), "-c".into(), "h=/usr/lib/tessera/tessera-root; [ -x \"$h\" ] || h=/usr/lib/irice/irice-root; exec pkexec \"$h\" \"$@\" 2>&1".into(), "sh".into(), "priority".into(), (if on { "on" } else { "off" }).into()];
                run_then(cmd, move |code, out| send(Box::new(Msg::PriorityJob(code, out))));
                self.publish_priority(h);
            }
            ("conflicts", "check") => {
                self.announced = false;
                self.check_conflicts(h);
            }
            ("conflicts", "announce") => self.announce(h),
            ("conflicts", "mute") => {
                let mut m = self.muted();
                for id in a[0].as_array().cloned().unwrap_or_default() {
                    if let Some(id) = id.as_str() {
                        if !m.iter().any(|x| x == id) {
                            m.push(id.to_string());
                        }
                    }
                }
                ui_set("conflictsMuted", json!(m));
                self.conflicts_muted_now(h, m);
            }
            ("conflicts", "unmute") => {
                let m: Vec<String> = self.muted().into_iter().filter(|x| Some(x.as_str()) != a[0].as_str()).collect();
                ui_set("conflictsMuted", json!(m));
                self.conflicts_muted_now(h, m);
            }
            ("health", "check") => self.health_check(h),
            ("health", "fix") => {
                let id = a[0].as_str().unwrap_or("").to_string();
                if !self.fixing.is_empty() || id.is_empty() {
                    return;
                }
                self.fixing = id.clone();
                let send = h.sender("Probes");
                run_then(vec!["python3".into(), "-I".into(), script("health-check.py").to_string_lossy().into_owned(), "fix".into(), id.clone()], move |_, out| send(Box::new(Msg::Fixed(id, serde_json::from_str(&out).ok()))));
                self.publish_health(h);
            }
            // plain text, scrubbed like a bug report, to the clipboard
            ("health", "copyReport") => {
                let mut full = vec!["sh".to_string(), "-c".into(), "python3 -I \"$0\" text \"$@\" | wl-copy".into(), script("health-check.py").to_string_lossy().into_owned()];
                full.extend(self.health_args(h));
                run_then(full, |_, _| {});
                h.ask("island.post", json!([{"kind": "custom", "icon": "copy", "title": "Health report copied", "subtitle": "Paste it into an issue or a message", "color": "#0A84FF"}, 3000]));
            }
            // set by the password prompt: whether its agent registered with polkit
            ("health", "setPolkit") => {
                self.polkit = a[0].as_str().unwrap_or("no").to_string();
                self.publish_health(h);
            }
            ("keybinds", "refresh") => self.read_binds(h),
            ("bugreport", "start") => {
                self.bug_set("state", json!("idle"));
                self.bug_set("error", json!(""));
                self.bug_set("issueUrl", json!(""));
                let mon = Self::focused_monitor(h);
                let dir = runtime_dir().join("tessera-bug-report");
                let shot = dir.join("screen.png");
                self.bug_set("shot", json!(shot.to_string_lossy()));
                self.bug_publish(h);
                let send = h.sender("Probes");
                run_then(
                    vec!["sh".into(), "-c".into(), "mkdir -p \"$1\" && chmod 700 \"$1\" && grim ${3:+-o \"$3\"} \"$2\"".into(), "sh".into(), dir.to_string_lossy().into_owned(), shot.to_string_lossy().into_owned(), mon],
                    move |code, _| send(Box::new(Msg::Captured(code == 0))),
                );
            }
            ("bugreport", "gather") => self.bug_gather(h),
            // the page's fields (title, happened, steps, expected; includeShot/Log/Info), kept here like BugReport.qml's properties
            ("bugreport", "set") => {
                if let Some(k) = a[0].as_str().filter(|k| ["title", "happened", "steps", "expected", "includeShot", "includeLog", "includeInfo"].contains(k)) {
                    self.bug_set(k, a[1].clone());
                    if k.starts_with("include") {
                        self.bug_publish(h);
                    }
                }
            }
            // mark up the screenshot in the screenshot editor
            ("bugreport", "editShot") => {
                if self.bug.get("hasShot") == Some(&json!(true)) {
                    if let Some(f) = self.bug.get("shot").and_then(Value::as_str) {
                        h.ask("screen.editShot", json!([f]));
                    }
                }
            }
            ("bugreport", "openGithub") => self.call(h, "bugreport.github", args),
            ("bugreport", "send") => {
                let last = self.bug.get("lastSent").and_then(Value::as_i64).unwrap_or(0);
                if self.bug.get("state") == Some(&json!("sending")) || now_ms() - last < 60_000 {
                    return;
                }
                let r = if a[0].is_object() { a[0].clone() } else { self.report_now() };
                if r["title"].as_str().unwrap_or("").trim().is_empty() || r["happened"].as_str().unwrap_or("").trim().is_empty() {
                    self.bug_set("state", json!("error"));
                    self.bug_set("error", json!(if r["title"].as_str().unwrap_or("").trim().is_empty() { "Add a title, then send again." } else { "Say what happened, then send again." }));
                    self.bug_publish(h);
                    return;
                }
                self.bug_set("state", json!("sending"));
                self.bug_set("error", json!(""));
                self.bug_publish(h);
                let send = h.sender("Probes");
                py(&["send"], Some(format!("{r}\n")), move |v| send(Box::new(Msg::Sent(v))));
            }
            // the fallbacks: the report as text on the clipboard, or a prefilled GitHub issue
            ("bugreport", "copy") => {
                let r = if a[0].is_object() { a[0].clone() } else { self.report_now() };
                let script = script("bug-report.py").to_string_lossy().into_owned();
                run_then(vec!["sh".into(), "-c".into(), format!("printf '%s\\n' \"$1\" | python3 -I \"$0\" text | wl-copy"), script, r.to_string()], |_, _| {});
                h.ask("island.post", json!([{"kind": "custom", "icon": "copy", "title": "Report copied", "subtitle": "Paste it into an issue or a message", "color": "#0A84FF"}, 3000]));
            }
            ("bugreport", "github") => {
                let r = if a[0].is_object() { a[0].clone() } else { self.report_now() };
                let script = script("bug-report.py").to_string_lossy().into_owned();
                run_then(vec!["sh".into(), "-c".into(), "printf '%s\\n' \"$1\" | python3 -I \"$0\" github-url".into(), script, r.to_string()], |_, out| {
                    if out.trim().starts_with("https://") {
                        launch_open(out.trim());
                    }
                });
            }
            (p, f) => eprintln!("tessera-ui: host: no {p}.{f}"),
        }
    }
    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, _args: &[String]) -> Option<String> {
        match func {
            "check" => self.call(h, "conflicts.check", &json!([])),
            "announce" => self.announce(h),
            _ => {}
        }
        None
    }
}

impl Probes {
    fn conflicts_muted_now(&self, h: &mut Host, _m: Vec<String>) {
        self.publish_conflicts(h);
    }

    fn read_binds(&self, h: &mut Host) {
        let send = h.sender("Probes");
        run_then(vec!["hyprctl".into(), "-j".into(), "binds".into()], move |_, out| send(Box::new(Msg::Binds(out))));
    }

    fn focused_monitor(h: &Host) -> String {
        h.st.shell.get::<tessera_core::services::hypr::Snapshot>().map(|s| s.focused.clone()).unwrap_or_default()
    }

    /// BugReport.report(): what the fields say now
    fn report_now(&self) -> Value {
        let g = |k: &str| self.bug.get(k).cloned().unwrap_or(Value::Null);
        let s = |k: &str| json!(g(k).as_str().unwrap_or(""));
        let b = |k: &str, d: bool| g(k).as_bool().unwrap_or(d);
        json!({
            "title": s("title"), "happened": s("happened"), "steps": s("steps"), "expected": s("expected"),
            "includeShot": b("includeShot", true) && b("hasShot", false), "includeLog": b("includeLog", true), "includeInfo": b("includeInfo", true),
            "shot": s("shot"), "log": s("log"), "sysinfo": g("sysinfo")
        })
    }

    fn bug_gather(&mut self, h: &mut Host) {
        self.bug_set("gathering", json!(true));
        self.bug_publish(h);
        let send = h.sender("Probes");
        py(&["gather", &shell_dir().to_string_lossy()], None, move |v| send(Box::new(Msg::Gathered(v))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stack_script_prints_key_value_lines() {
        let m = parse_stack("net=iwd\nwifi_iface=wlan0\nppd=0\n\nnoise\n=x\n");
        assert_eq!(m["net"], "iwd");
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn wifi_managed_elsewhere_says_who() {
        let b = backends(&parse_stack("net=iwd\nwifi_iface=wlan0\nwifi_app=iwgtk\nppd=1\n"), true);
        assert_eq!(b["wifiUnmanaged"], true);
        assert_eq!(b["wifiTitle"], "Wi-Fi is managed by iwd");
        assert_eq!(b["wifiShort"], "Managed by iwd");
        assert!(b["wifiReason"].as_str().unwrap().contains("iwgtk"));
        let ok = backends(&parse_stack("net=networkmanager\nwifi_iface=wlan0\n"), true);
        assert_eq!(ok["wifiUnmanaged"], false);
        assert_eq!(ok["wifiTitle"], "");
        // not looked at yet: nothing is said
        let early = backends(&BTreeMap::new(), false);
        assert_eq!(early["ppd"], true);
        assert_eq!(early["wifiUnmanaged"], false);
    }

    #[test]
    fn energy_modes_and_bluetooth_say_what_is_missing() {
        let b = backends(&parse_stack("ppd=0\npowerd=tlp\nbt_hardware=1\nbt_installed=1\nbt_running=0\n"), true);
        assert_eq!(b["powerShort"], "Managed by TLP");
        assert_eq!(b["btState"], "stopped");
        assert_eq!(b["btFix"], "start");
        assert_eq!(b["btFixLabel"], "Start Bluetooth");
        assert_eq!(backends(&parse_stack("ppd=0\n"), true)["powerShort"], "Needs power-profiles-daemon");
        assert_eq!(backends(&parse_stack("bt_hard=1\n"), true)["btState"], "hard-blocked");
        assert_eq!(backends(&parse_stack("bt_hardware=1\nbt_installed=1\nbt_running=1\n"), true)["btState"], "ok");
    }

    #[test]
    fn key_names() {
        assert_eq!(pretty_key("Return"), "Enter");
        assert_eq!(pretty_key("a"), "A");
        assert_eq!(pretty_key("XF86AudioMute"), "Audio Mute");
        assert_eq!(pretty_key("SUPER_L"), "");
        assert_eq!(pretty_key("mouse:272"), "Left Click");
    }

    #[test]
    fn numbered_binds_become_one_row() {
        let binds: Vec<(Vec<String>, String)> = (1..=3).map(|n| (vec!["Super".to_string(), n.to_string()], format!("Go to space {n}"))).chain([(vec!["Super".to_string(), "L".to_string()], "Lock".to_string())]).collect();
        let m = merge_numbered(binds);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].1, "Go to space 1–3");
        assert_eq!(m[0].0, vec!["Super", "1–3"]);
    }

    #[test]
    fn binds_group_by_their_description() {
        let v = json!([
            {"description": "Windows: Close", "modmask": 64, "key": "Q"},
            {"description": "Windows: Close", "modmask": 64, "key": "W"},
            {"description": "Lock", "modmask": 65, "key": "L"},
            {"description": "", "modmask": 64, "key": "X"}
        ]);
        let g = bind_groups(&v);
        assert_eq!(g[0]["name"], "Windows");
        assert_eq!(g[0]["binds"].as_array().unwrap().len(), 1); // (the same action once)
        assert_eq!(g[0]["binds"][0]["keys"], json!(["Super", "Q"]));
        assert_eq!(g[1]["name"], "General");
        assert_eq!(g[1]["binds"][0]["keys"], json!(["Super", "Shift", "L"]));
    }

    #[test]
    fn conflict_names_read_as_a_list() {
        let c = |n: &str| json!({"name": n});
        assert_eq!(names(&[c("waybar")]), "waybar");
        assert_eq!(names(&[c("waybar"), c("mako")]), "waybar and mako");
        assert_eq!(names(&[c("waybar"), c("mako"), c("hypridle"), c("mako")]), "waybar, mako and hypridle");
        assert_eq!(names(&[]), "");
    }
}
