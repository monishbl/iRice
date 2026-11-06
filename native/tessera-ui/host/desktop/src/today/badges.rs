//! Badges.qml: the dock's badges. Unread counts apps report over D-Bus (the
//! Unity LauncherEntry API: Telegram, Discord, Thunderbird, Signal…), read
//! from dbus-monitor's text (the QML's fallback; the Events service of the
//! shell host may call `badges.update [app, count, visible]` instead), and a
//! dot for apps whose window asks for attention (urgent: the dock reads
//! clients::Windows.urgent itself; the classes are said here too).
//! Said as the state field `badges`: { counts: { entry id: n }, urgent:
//! [window classes] }.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use tessera_core::host::Host;
use tessera_core::services::clients::Windows;

#[derive(Default)]
pub struct Badges {
    pub counts: BTreeMap<String, i64>,
    started: bool,
}

/// One LauncherEntry update from the monitor
pub struct Update {
    pub app: String,
    pub count: Option<i64>,
    pub visible: Option<bool>,
}

impl Badges {
    /// Badges.flush: apply an update
    pub fn apply(&mut self, u: Update) -> bool {
        let id = u.app.trim_start_matches("application://").trim_end_matches(".desktop").to_string();
        let prev = self.counts.get(&id).copied().unwrap_or(0);
        let visible = u.visible.unwrap_or(prev > 0 || u.count.unwrap_or(0) > 0);
        let n = u.count.unwrap_or(prev);
        let before = self.counts.clone();
        if visible && n > 0 {
            self.counts.insert(id, n);
        } else {
            self.counts.remove(&id);
        }
        before != self.counts
    }

    pub fn start(&mut self, h: &mut Host) {
        h.st.shell.want("clients");
        if self.started || !tessera_core::util::on_path("dbus-monitor") {
            return;
        }
        self.started = true;
        let send = h.sender(super::NAME);
        std::thread::spawn(move || monitor(send));
    }

    pub fn call(&mut self, args: &Value) -> bool {
        let a = super::common::args_of(args);
        let app = a.first().and_then(Value::as_str).unwrap_or("").to_string();
        if app.is_empty() {
            return false;
        }
        self.apply(Update { app, count: a.get(1).and_then(Value::as_i64), visible: a.get(2).and_then(Value::as_bool) })
    }

    pub fn publish(&self, h: &mut Host) {
        let urgent: Vec<String> = h
            .st
            .shell
            .get::<Windows>()
            .map(|w| {
                let mut v: Vec<String> = w.clients.iter().filter(|c| w.urgent.contains(&c.address)).map(|c| c.class.clone()).collect();
                v.sort();
                v.dedup();
                v
            })
            .unwrap_or_default();
        h.set_state("badges", json!({"counts": self.counts, "urgent": urgent}));
    }
}

/// dbus-monitor on the LauncherEntry signal, its text read as the QML does:
///   signal ... interface=com.canonical.Unity.LauncherEntry; member=Update
///      string "application://org.telegram.desktop.desktop"
///      array [ dict entry( string "count"  variant int64 3 ) ... ]
fn monitor(send: impl Fn(Box<dyn std::any::Any + Send>)) {
    use std::io::{BufRead, BufReader};
    let Ok(mut child) = std::process::Command::new("dbus-monitor")
        .args(["--session", "type='signal',interface='com.canonical.Unity.LauncherEntry',member='Update'"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return;
    };
    let Some(out) = child.stdout.take() else { return };
    let (mut app, mut key) = (String::new(), String::new());
    let mut pending: Option<(Option<i64>, Option<bool>)> = None;
    let flush = |app: &str, p: &mut Option<(Option<i64>, Option<bool>)>| {
        if let (false, Some((count, visible))) = (app.is_empty(), p.take()) {
            send(Box::new(super::Msg::Badge(Update { app: app.to_string(), count, visible })));
        }
    };
    for line in BufReader::new(out).lines().map_while(Result::ok) {
        let t = line.trim();
        if t.starts_with("signal ") {
            flush(&app, &mut pending);
            app.clear();
            pending = if t.contains("LauncherEntry") { Some((None, None)) } else { None };
            continue;
        }
        let Some(p) = pending.as_mut() else { continue };
        if let Some(s) = t.strip_prefix("string \"").and_then(|r| r.strip_suffix('"')) {
            if app.is_empty() {
                app = s.to_string();
            } else {
                key = s.to_string();
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("variant ") {
            let val = rest.split_whitespace().nth(1).unwrap_or("");
            if key == "count" {
                p.0 = Some(val.parse().unwrap_or(0));
            } else if key == "count-visible" {
                p.1 = Some(val == "true");
            }
            key.clear();
            continue;
        }
        if t == "]" {
            flush(&app, &mut pending);
        }
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_come_and_go() {
        let mut b = Badges::default();
        assert!(b.apply(Update { app: "application://org.telegram.desktop.desktop".into(), count: Some(3), visible: None }));
        assert_eq!(b.counts["org.telegram.desktop"], 3);
        assert!(b.apply(Update { app: "application://org.telegram.desktop.desktop".into(), count: None, visible: Some(false) }));
        assert!(b.counts.is_empty());
        assert!(!b.apply(Update { app: "application://x.desktop".into(), count: Some(0), visible: None }));
    }
}
