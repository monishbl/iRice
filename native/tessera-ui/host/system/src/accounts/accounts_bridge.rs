//! modules/settings/RustBridgeAccounts.qml: Settings drawn in Rust, the
//! "accounts" page group's side: Accounts (Google, iCloud, Microsoft 365,
//! their drives), Calendar, AI & Sidebar, Default Apps. While Settings is
//! open (`live`) it tells the pages what only the shell knows (`feed`, the
//! pages' cx.feed["accounts"]) and does what they ask (`act`:
//! cx.ask("accounts", what, args)).
//!
//! How it is reached: the settings window asks `shell::ask("settings",
//! ["act", "accounts", what, …args])`; host-shell's settings router should
//! forward that as `h.ask("accounts.<what>", json!([…args]))` (this service
//! also accepts the bare call "accounts" with args [what, …args]). Other
//! calls: accounts.live [bool] (Settings opened/closed; also read from the
//! state "settingsOpen" when host-shell sets it; the feed is sent while
//! live, and live starts true) and accounts.updatesFinished [job] (the
//! Updates service tells every service that installs through it).
//!
//! Reads from other groups (bus keys, owners): "needs" (host-shell:
//! checked, secretTool, keyring, goa, goaMicrosoft, goaSignin, installing,
//! building, buildError), "calendar" (desktop: sources, lastError,
//! defaultAlert), "ai" (desktop: providers, models [{key,name}], hasKey
//! {modelKey: bool}, sessions, pinned), "updates" (sysinfo: running).
//! Asks it makes: launch.open [url], launch.run [argv], needs.install [list],
//! needs.buildNative, calendar.setSource [url, {enabled}], calendar.removeSource
//! [url], calendar.addSource [name, url], calendar.setDefaultAlert [n],
//! ai.setModel [key], ai.accountsSave [raw, oldKey, credential],
//! ai.removeModel [key] (keeps the removed record for an undo),
//! ai.restoreRemoved, ai.forgetRemoved, ai.deleteAllChats.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Poll,
    AiUndoDone,
}

pub struct AccountsBridge {
    live: bool,
    sent: String,
    ai_undo: String, // the text shown ("" none)
    undo_t: slint::Timer,
    poll: slint::Timer,
    tx: Option<Tx>,
}

impl Default for AccountsBridge {
    fn default() -> Self {
        Self { live: true, sent: String::new(), ai_undo: String::new(), undo_t: slint::Timer::default(), poll: slint::Timer::default(), tx: None }
    }
}

/// The provider → environment variable table (AiKeys.envNames); only the
/// variable's presence is looked at, never its value
const ENV_NAMES: &[(&str, &str)] = &[
    ("anthropic", "ANTHROPIC_API_KEY"),
    ("gemini", "GEMINI_API_KEY"),
    ("aistudio", "GEMINI_API_KEY"),
    ("vertex", "VERTEX_API_KEY"),
    ("vertex-oauth", "GOOGLE_OAUTH_ACCESS_TOKEN"),
    ("openai", "OPENAI_API_KEY"),
    ("openrouter", "OPENROUTER_API_KEY"),
];

/// Providers that have a key in the environment (envKeys())
pub fn env_keys(has: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out: Vec<String> = ENV_NAMES.iter().filter(|(_, v)| has(v)).map(|(p, _)| p.to_string()).collect();
    if has("GOOGLE_API_KEY") {
        out.push("aistudio".into());
        out.push("vertex".into());
    }
    out
}

const TOOLS: &[&str] = &[
    "kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "gnome-terminal", "kgx", "ptyxis", "xfce4-terminal", "xterm", "zen-browser", "firefox",
    "google-chrome-stable", "chromium", "brave", "code", "codium", "zed", "cursor", "windsurf", "gnome-text-editor", "kate", "mousepad", "pavucontrol",
    "pavucontrol-qt", "gnome-system-monitor", "plasma-systemmonitor", "btop", "htop",
];

fn map_rows(v: &Value, f: impl Fn(&Value) -> Value) -> Value {
    Value::Array(v.as_array().map(|a| a.iter().map(&f).collect()).unwrap_or_default())
}

fn s_(v: &Value, k: &str) -> Value {
    json!(v[k].as_str().unwrap_or(""))
}

impl AccountsBridge {
    fn feed(&self) -> Value {
        let g = crate::bus::get("google");
        let c = crate::bus::get("cloudaccounts");
        let d = crate::bus::get("clouddrives");
        let n = crate::bus::get("needs");
        let cal = crate::bus::get("calendar");
        let ai = crate::bus::get("ai");
        let acct = &g["account"];
        let contacts = c["contacts"].as_array().cloned().unwrap_or_default();
        let sessions = ai["sessions"].as_i64().unwrap_or(0);
        json!({
            "signingIn": g["signingIn"].as_bool().unwrap_or(false),
            "google": {
                "connected": g["connected"].as_bool().unwrap_or(false),
                "configured": g["configured"].as_bool().unwrap_or(false),
                "busy": g["busy"].as_bool().unwrap_or(false),
                "operation": g["operation"].as_str().unwrap_or(""),
                "pendingAccount": g["pending"]["account"].as_str().unwrap_or(""),
                "lastError": g["lastError"].as_str().unwrap_or(""),
                "outbox": g["outbox"].as_i64().unwrap_or(0),
                "signin": acct["signin"] == true,
                "goaSettings": acct["goaSettings"].as_bool().unwrap_or(false) || acct["goaSettings"].as_str().is_some_and(|s| !s.is_empty()),
                "goaAccounts": Value::Array(acct["goaAccounts"].as_array().map(|a| a.iter().filter(|x| !x["used"].as_bool().unwrap_or(false)).map(|x| json!({"id": x["id"], "name": x["name"].as_str().unwrap_or("")})).collect()).unwrap_or_default()),
                "accounts": map_rows(&g["accounts"], |a| json!({"id": a["id"], "identity": a["identity"].as_str().unwrap_or(""), "syncedAt": a["syncedAt"].as_f64().unwrap_or(0.0)})),
                "calendars": map_rows(&g["calendars"], |x| json!({"account": s_(x, "account"), "id": s_(x, "id"), "title": s_(x, "title"), "primary": x["primary"].as_bool().unwrap_or(false), "writable": x["writable"].as_bool().unwrap_or(false)})),
                "taskLists": map_rows(&g["taskLists"], |x| json!({"account": s_(x, "account"), "id": s_(x, "id"), "title": s_(x, "title")})),
            },
            "cloud": {
                "busy": c["busy"].as_bool().unwrap_or(false),
                "signingIn": c["signingIn"].as_bool().unwrap_or(false),
                "signingInto": c["signingInto"].as_str().unwrap_or(""),
                "lastError": c["lastError"].as_str().unwrap_or(""),
                "errorFrom": c["errorFrom"].as_str().unwrap_or(""),
                "accounts": map_rows(&c["accounts"], |a| json!({"id": a["id"], "provider": s_(a, "provider"), "identity": s_(a, "identity"), "syncedAt": a["syncedAt"].as_f64().unwrap_or(0.0),
                                                              "contacts": contacts.iter().filter(|k| k["account"] == a["id"]).count()})),
                "calendars": map_rows(&c["calendars"], |x| json!({"account": s_(x, "account"), "id": s_(x, "id"), "title": s_(x, "title"), "writable": x["writable"].as_bool().unwrap_or(false)})),
                "taskLists": map_rows(&c["taskLists"], |x| json!({"account": s_(x, "account"), "id": s_(x, "id"), "title": s_(x, "title")})),
            },
            "needs": {
                "checked": n["checked"].as_bool().unwrap_or(false),
                "secretTool": n["secretTool"].as_bool().unwrap_or(false),
                "keyring": n["keyring"].as_bool().unwrap_or(false),
                "goa": n["goa"].as_bool().unwrap_or(false),
                "goaMicrosoft": n["goaMicrosoft"].as_bool().unwrap_or(false),
                "goaSignin": n["goaSignin"].as_bool().unwrap_or(false),
                "installing": n["installing"].as_bool().unwrap_or(false),
                "building": n["building"].as_bool().unwrap_or(false),
                "buildError": n["buildError"].as_str().unwrap_or(""),
                "updatesRunning": updates_running(),
            },
            "drives": {
                "hasRclone": d["hasRclone"].as_bool().unwrap_or(false),
                "busy": d["busy"].as_str().unwrap_or(""),
                "installing": d["installing"].as_bool().unwrap_or(false),
                "installError": d["installError"].as_str().unwrap_or(""),
                "errors": if d["errors"].is_object() { d["errors"].clone() } else { json!({}) },
                "gdrive": d["drives"]["gdrive"],
                "onedrive": d["drives"]["onedrive"],
            },
            "calendar": {
                "sources": map_rows(&cal["sources"], |s| json!({"name": s_(s, "name"), "url": s_(s, "url"), "color": s["color"].as_str().unwrap_or("#8E8E93"), "enabled": s["enabled"] != false})),
                "lastError": cal["lastError"].as_str().unwrap_or(""),
                "defaultAlert": cal["defaultAlert"].as_i64().unwrap_or(10),
            },
            "ai": {
                "providers": if ai["providers"].is_null() { json!([]) } else { ai["providers"].clone() },
                "hasKey": if ai["hasKey"].is_object() { ai["hasKey"].clone() } else { json!({}) },
                "envKeys": env_keys(|v| std::env::var_os(v).is_some_and(|x| !x.is_empty())),
                "sessions": sessions,
                "pinned": ai["pinned"].as_i64().unwrap_or(0),
                "undo": self.ai_undo,
            },
            "tools": TOOLS.iter().filter(|t| has_tool(t)).collect::<Vec<_>>(),
        })
    }

    fn finish_ai_undo(&mut self, h: &mut Host) {
        if !self.ai_undo.is_empty() {
            h.ask("ai.forgetRemoved", json!([]));
        }
        self.ai_undo.clear();
        self.undo_t.stop();
    }

    fn act(&mut self, h: &mut Host, what: &str, a: &[Value]) {
        let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        let arg = |i: usize| a.get(i).cloned().unwrap_or(Value::Null);
        match what {
            // Google: [{ cmd, account?, path? }]: sync, disconnect, connectDesktop, connect, configure, forget
            "googleRun" => {
                if arg(0)["cmd"].as_str().is_some_and(|c| ["sync", "disconnect", "connectDesktop", "connect", "configure", "forget"].contains(&c)) {
                    h.ask("google.run", json!([arg(0)]));
                }
            }
            "googleConnect" => h.ask("google.connect", json!([])),
            "googleEvent" => {
                // [account, text]
                if !s(1).trim().is_empty() {
                    h.ask("google.write", json!([{"cmd": "eventAdd", "account": s(0), "calendarId": "primary", "text": s(1)}]));
                }
            }
            "googleTasks" => h.ask("launch.open", json!(["https://tasks.google.com/"])),
            "googleKeep" => h.ask("google.openKeep", json!([])),
            "onlineAccounts" => {
                let app = crate::bus::get("google")["account"]["goaSettings"].as_str().unwrap_or("").to_string();
                if app.is_empty() {
                    h.ask("google.setError", json!(["Install gnome-online-accounts and gnome-control-center (or gnome-online-accounts-gtk), then try again."]));
                    return;
                }
                let argv = if app.ends_with("gnome-control-center") { json!(["env", "XDG_CURRENT_DESKTOP=GNOME", app, "online-accounts", "google"]) } else { json!([app]) };
                h.ask("launch.run", json!([argv]));
            }
            // iCloud and Microsoft 365
            "cloudSync" => h.ask("cloudaccounts.sync", json!([])),
            "cloudDisconnect" => h.ask("cloudaccounts.disconnect", json!([s(0)])),
            // [Apple ID, app-specific password] (sent once, kept nowhere here)
            "icloudSignIn" => {
                if !s(0).trim().is_empty() && !s(1).trim().is_empty() {
                    h.ask("cloudaccounts.connectICloud", json!([s(0).trim(), s(1)]));
                }
            }
            "microsoftSignIn" => h.ask("cloudaccounts.connectMicrosoft", json!([])),
            "appleAccount" => h.ask("launch.open", json!(["https://account.apple.com/account/manage"])),
            "needsInstall" => {
                if let Some(l) = arg(0).as_array() {
                    let l: Vec<String> = l.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect();
                    h.ask("needs.install", json!([l]));
                }
            }
            "needsBuild" => h.ask("needs.buildNative", json!([])),
            // Drives
            "driveInstall" => h.ask("clouddrives.install", json!([])),
            "driveOpen" => h.ask("clouddrives.open", json!([s(0)])),
            // [mount | disconnect | connect, drive]
            "driveAct" => {
                if ["mount", "disconnect", "connect"].contains(&s(0).as_str()) {
                    h.ask("clouddrives.act", json!([s(0), s(1)]));
                }
            }
            "driveCancel" => h.ask("clouddrives.cancel", json!([])),
            // Calendar's subscriptions
            "calSet" => h.ask("calendar.setSource", json!([s(0), {"enabled": arg(1).as_bool().unwrap_or(false)}])),
            "calRemove" => h.ask("calendar.removeSource", json!([s(0)])),
            "calAdd" => h.ask("calendar.addSource", json!([s(0), s(1)])),
            "calAlert" => h.ask("calendar.setDefaultAlert", json!([arg(0).as_f64().unwrap_or(10.0)])),
            // AI
            "aiSetModel" => h.ask("ai.setModel", json!([s(0)])),
            // [draft, old key, credential] (checked by the page first; the credential goes to the keyring, never back)
            "aiSave" => h.ask("ai.accountsSave", json!([arg(0), s(1), s(2)])),
            "aiRemove" => {
                self.finish_ai_undo(h); // (an earlier removal is final now)
                let key = s(0);
                let ai = crate::bus::get("ai");
                let Some(m) = ai["models"].as_array().and_then(|ms| ms.iter().find(|m| m["key"].as_str() == Some(key.as_str()))).cloned() else { return };
                h.ask("ai.removeModel", json!([key]));
                self.ai_undo = format!("{} removed", m["name"].as_str().unwrap_or(""));
                if let Some(tx) = self.tx.clone() {
                    self.undo_t.start(slint::TimerMode::SingleShot, Duration::from_millis(8000), move || tx.send(M::AiUndoDone));
                }
            }
            "aiUndo" => {
                if !self.ai_undo.is_empty() {
                    h.ask("ai.restoreRemoved", json!([]));
                    self.ai_undo.clear();
                    self.undo_t.stop();
                }
            }
            "aiDeleteAll" => h.ask("ai.deleteAllChats", json!([])),
            other => eprintln!("tessera-ui: settings (accounts): no act {other}"),
        }
    }

    fn send_feed(&mut self, h: &mut Host) {
        if !self.live {
            return;
        }
        let f = self.feed();
        let text = f.to_string();
        if text == self.sent {
            return;
        }
        self.sent = text;
        h.tell("settings", json!({"feed": {"accounts": f}}));
    }
}

impl Service for AccountsBridge {
    fn name(&self) -> &'static str {
        "AccountsBridge"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["accounts"]
    }
    fn start(&mut self, h: &mut Host) {
        let tx = Tx::new(h.sender("AccountsBridge"));
        self.tx = Some(tx.clone());
        self.poll.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || tx.send(M::Poll));
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let list: Vec<Value> = args.as_array().cloned().unwrap_or_else(|| if args.is_null() { vec![] } else { vec![args.clone()] });
        let (what, rest): (String, Vec<Value>) = match call.split_once('.') {
            Some((_, w)) => (w.to_string(), list),
            None => (list.first().and_then(Value::as_str).unwrap_or("").to_string(), list.into_iter().skip(1).collect()),
        };
        match what.as_str() {
            "live" => {
                self.live = rest.first().and_then(Value::as_bool).unwrap_or(true);
                if !self.live {
                    self.finish_ai_undo(h);
                } else {
                    self.sent.clear();
                }
            }
            // The services that install through Updates hear that a job ended
            "updatesFinished" => {
                let j = rest.first().cloned().unwrap_or(Value::Null);
                for p in ["fingerprint", "face", "clouddrives", "bootsnapshots"] {
                    h.ask(&format!("{p}.updatesFinished"), json!([j]));
                }
            }
            w => self.act(h, w, &rest),
        }
        self.send_feed(h);
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        match msg.downcast::<M>().map(|m| *m) {
            Ok(M::AiUndoDone) => {
                self.finish_ai_undo(h);
                self.send_feed(h);
            }
            Ok(M::Poll) => {
                // (host-shell may say Settings is open: the state "settingsOpen")
                if let Some(open) = h.state("panels.settingsOpen").as_bool() {
                    if open != self.live {
                        self.live = open;
                        if !open {
                            self.finish_ai_undo(h);
                        } else {
                            self.sent.clear();
                        }
                    }
                }
                self.send_feed(h);
            }
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn env_keys_are_names_only() {
        let k = env_keys(|v| v == "OPENAI_API_KEY" || v == "GOOGLE_API_KEY");
        assert_eq!(k, ["openai", "aistudio", "vertex"]);
        assert!(env_keys(|_| false).is_empty());
    }
}
