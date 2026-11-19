//! LoginScreen.qml: the Tessera login screen (greetd): its look follows
//! yours. The greeter can't read your home, so the little it needs goes to
//! /usr/share/tessera/greeter.json through the root helper
//! (`tessera-root login-look`, no password for an administrator), 3 s after
//! a change and only when it differs from what is there. Never from the dev
//! sandbox; never before the helper's policy knows login-look.
//!
//! Switching (Rust only): on a computer that signs in with another login
//! manager (SDDM, GDM, LightDM, ly, none), Settings → Lock & Login Screen
//! switches to Tessera's through `tessera-root login-switch on` (the
//! password, once) and back with `login-switch off`. The helper keeps the
//! one before in /etc/tessera/login-switch.json and turns it back on by
//! itself when Tessera's fails at the next start (state "reverted", with
//! why); after you log in, a notification says so, once per revert.
//!
//! Bus "loginscreen": { installed, hasCopy, canWrite, status, share, dm,
//! tessera, record, boot, canSwitch, hint, busy, error }, also sent to
//! Settings as its "look" feed's `login` (settings.feed).
//! Calls: loginscreen.preview (shows the login screen as it would look,
//! with a stand-in password; Escape closes it), loginscreen.refresh,
//! loginscreen.switch [style] (Tessera's, with that style), loginscreen.back.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Probe(i32),
    Share(String),
    Written(i32),
    Later,
    Switch(String),
    Done(i32, String),
}

pub struct LoginScreen {
    tx: Option<Tx>,
    has_copy: bool,
    can_write: bool,
    status: String,
    share: String,
    writing: bool,
    look: String,
    later: slint::Timer,
    /// what Settings was last sent (its "look" feed's `login`)
    sent: Value,
    /// the switch probe's: the login manager on, Tessera's config, the
    /// helper's record, this boot, whether the helper switches
    switch: Value,
    /// "switch" | "back" while the helper runs
    busy: String,
    error: String,
}

impl Default for LoginScreen {
    fn default() -> Self {
        Self {
            tx: None,
            has_copy: false,
            can_write: false,
            status: String::new(),
            share: "/usr/share/tessera".into(),
            writing: false,
            look: String::new(),
            later: slint::Timer::default(),
            sent: Value::Null,
            switch: json!({}),
            busy: String::new(),
            error: String::new(),
        }
    }
}

fn hex_ok(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// What the greeter gets (keys in order: greeter.json is written sorted, so
/// the two compare as text). `seed`/`dark`/`material` come from the look.
pub fn look_json(cfg: &Value, dark: bool, look_seed: &str) -> String {
    let a = &cfg["appearance"];
    let l = &cfg["lock"];
    let seed_cfg = a["materialSeed"].as_str().unwrap_or("wallpaper");
    let s = if hex_ok(seed_cfg) { seed_cfg.to_string() } else { look_seed.chars().take(7).collect() };
    let rnd = |v: &Value, d: f64| v.as_f64().unwrap_or(d).round() as i64;
    json!({
        "clock24": a["clock24"] == true,
        "clockFace": l["clockFace"].as_str().unwrap_or("dynamic"),
        "clockRound": l["clockRound"].as_bool().unwrap_or(true),
        "clockWeight": rnd(&l["clockWeight"], 640.0),
        "clockWidth": rnd(&l["clockWidth"], 112.0),
        "look": if a["look"] == "material" { "material" } else { "glass" },
        "materialStyle": a["materialStyle"].as_str().unwrap_or("tonalSpot"),
        "mode": if dark { "dark" } else { "light" },
        "seed": if hex_ok(&s) { s } else { "#6750A4".into() },
        "style": l["loginStyle"].as_str().unwrap_or("look"),
    })
    .to_string()
}

/// The look with another style (one the helper takes; else as it was)
pub fn with_style(look: &str, style: &str) -> String {
    let mut v: Value = serde_json::from_str(look).unwrap_or(Value::Null);
    if ["look", "text", "plain"].contains(&style) && v.is_object() {
        v["style"] = json!(style);
    }
    if v.is_null() { look.to_string() } else { v.to_string() }
}

/// The switch probe's script: one `key:value` (or a bare flag) per line
fn switch_probe_script() -> String {
    [
        "echo \"dm:$(basename \"$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)\" .service 2>/dev/null)\"".to_string(),
        "grep -q /usr/share/tessera/ /etc/greetd/config.toml 2>/dev/null && [ -d /usr/share/tessera/shell ] && echo tessera".into(),
        "echo \"record:$(head -c 4096 /etc/tessera/login-switch.json 2>/dev/null | tr -d '\\n')\"".into(),
        "echo \"boot:$(cat /proc/sys/kernel/random/boot_id 2>/dev/null)\"".into(),
        format!("{} && echo switcher", root_knows("login-switch")),
        ":".into(),
    ]
    .join("; ")
}

/// The probe's lines as the bus's fields (`hint`: the login manager
/// install.sh noted before there was a record, for going back)
pub fn read_switch_probe(text: &str, hint: &str) -> Value {
    let value = |k: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{k}:"))).map(|v| v.trim().to_string()).unwrap_or_default();
    let flag = |k: &str| text.lines().any(|l| l.trim() == k);
    let record: Value = serde_json::from_str(&value("record")).ok().filter(Value::is_object).unwrap_or(Value::Null);
    let dm = value("dm");
    // (a unit's name only: anything else isn't one)
    let ok = |s: &str| s.len() <= 40 && s.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c));
    json!({
        "checked": true,
        "dm": if ok(&dm) { dm } else { String::new() },
        "tessera": flag("tessera"),
        "record": record,
        "boot": value("boot"),
        "canSwitch": flag("switcher"),
        "hint": if ok(hint.trim()) { hint.trim() } else { "" },
    })
}

/// A login manager's name as people know it
pub fn dm_name(unit: &str) -> String {
    match unit {
        "" => "the console".into(),
        "sddm" => "SDDM".into(),
        "gdm" => "GDM".into(),
        "lightdm" => "LightDM".into(),
        "lxdm" => "LXDM".into(),
        "xdm" => "XDM".into(),
        o => o.into(),
    }
}

/// After a switch back by itself: the notification (title, body), once per
/// revert (`seen`: the revert's time last told)
pub fn revert_notice(record: &Value, seen: &str) -> Option<(String, String)> {
    if record["state"] != "reverted" {
        return None;
    }
    let when = record["time"].as_i64().unwrap_or(0).to_string();
    if when == seen.trim() {
        return None;
    }
    let before = dm_name(record["previous"].as_str().unwrap_or(""));
    let why = record["reason"].as_str().filter(|r| !r.is_empty()).unwrap_or("it didn't start");
    let title = if record["previous"] == "" { "You sign in on the console again".to_string() } else { format!("{before} is your login screen again") };
    Some((title, format!("Tessera's login screen was turned off because {why}. To try it again, open Settings → Lock & Login Screen.")))
}

/// What a helper's exit says, for Settings (126: the password prompt was
/// dismissed, nothing changed)
pub fn switch_error(code: i32, err: &str) -> String {
    match code {
        0 | 126 => String::new(),
        127 => "Not allowed: an administrator's password is needed.".into(),
        _ => {
            let m = root_message(err);
            let line = m.lines().last().unwrap_or("").trim().to_string();
            let mut c = line.chars();
            match c.next() {
                Some(f) => format!("{}{}.", f.to_uppercase().collect::<String>(), c.as_str().trim_end_matches('.')),
                None => "That didn't work. Update Tessera, then try again.".into(),
            }
        }
    }
}

fn seen_file() -> std::path::PathBuf {
    tessera_core::util::state_dir().join("login-reverted-seen")
}

/// install.sh's note of the login manager before (tessera uninstall's)
fn install_hint() -> String {
    let data = tessera_core::util::xdg("XDG_DATA_HOME", ".local/share");
    std::fs::read_to_string(data.join("tessera/previous-login-manager")).unwrap_or_default().trim().to_string()
}

impl LoginScreen {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn publish(&mut self, h: &mut Host) {
        let mut v = json!({"installed": self.has_copy, "hasCopy": self.has_copy, "canWrite": self.can_write, "status": self.status, "share": self.share,
                           "busy": self.busy, "error": self.error});
        if let (Some(o), Some(s)) = (v.as_object_mut(), self.switch.as_object()) {
            for (k, x) in s {
                o.insert(k.clone(), x.clone());
            }
        }
        put("loginscreen", v.clone());
        // Settings → Lock & Login Screen reads it as feed["look"]["login"]
        if v != self.sent {
            self.sent = v.clone();
            h.ask("settings.feed", json!(["look", {"login": v}]));
        }
    }
    fn recompute(&mut self, h: &Host) {
        let l = &h.st.shell.look;
        let now = look_json(h.cfg(), l.dark, &l.seed);
        if now != self.look {
            self.look = now;
            if self.tx.is_some() {
                self.arm();
            }
        }
    }
    fn arm(&self) {
        let tx = self.tx();
        self.later.start(slint::TimerMode::SingleShot, Duration::from_millis(3000), move || tx.send(M::Later));
    }
    /// The copy (installed, writable) and the switch's state
    fn probe(&self) {
        let t1 = self.tx();
        std::thread::spawn(move || {
            // (the old name's copy: the login screen installed before the rename)
            let script = format!("[ -d /usr/share/tessera/shell ] || [ -d {}/shell ] || exit 1; {} && exit 0; exit 2", tessera_core::old_names::LOGIN_SHARE, root_knows("login-look"));
            let (code, _, _) = run_full(&strs(&["sh", "-c", &script]), None);
            t1.send(M::Probe(code));
        });
        let t2 = self.tx();
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&strs(&["sh", "-c", &switch_probe_script()]), None);
            t2.send(M::Switch(out));
        });
    }
    fn run_helper(&mut self, h: &mut Host, what: &str, args: Vec<String>) {
        if !self.busy.is_empty() {
            return;
        }
        self.error.clear();
        if tessera_core::env::sandboxed() {
            self.error = "Not from the test session: it would change this computer's login screen.".into();
            self.publish(h);
            return;
        }
        if !has_tool("pkexec") {
            self.error = NO_PKEXEC.into();
            self.publish(h);
            return;
        }
        self.busy = what.into();
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let cmd = root_command(&a);
        let tx = self.tx();
        std::thread::spawn(move || {
            let (code, _, err) = run_full(&cmd, None);
            tx.send(M::Done(code, err));
        });
        self.publish(h);
    }
}

impl Service for LoginScreen {
    fn name(&self) -> &'static str {
        "LoginScreen"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["loginscreen"]
    }
    fn start(&mut self, h: &mut Host) {
        let tx = Tx::new(h.sender("LoginScreen"));
        self.tx = Some(tx.clone());
        let l = &h.st.shell.look;
        self.look = look_json(h.cfg(), l.dark, &l.seed);
        self.probe();
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&strs(&["sh", "-c", &format!("[ -d /usr/share/tessera ] || [ ! -d {} ] || echo old", tessera_core::old_names::LOGIN_SHARE)]), None);
            if out.trim() == "old" {
                tx.send(M::Share(tessera_core::old_names::LOGIN_SHARE.into()));
            }
        });
        self.publish(h);
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.recompute(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call {
            "loginscreen.refresh" => self.probe(),
            "loginscreen.switch" => {
                let style = args.get(0).and_then(Value::as_str).unwrap_or("");
                let l = &h.st.shell.look;
                let look = with_style(&look_json(h.cfg(), l.dark, &l.seed), style);
                let shell = crate::util::root().join("shell").to_string_lossy().into_owned();
                self.run_helper(h, "switch", vec!["login-switch".into(), "on".into(), shell, look]);
            }
            "loginscreen.back" => {
                let mut a = vec!["login-switch".to_string(), "off".into()];
                // (an install from before the record: the one install.sh noted)
                if !self.switch["record"]["previous"].is_string() {
                    let hint = self.switch["hint"].as_str().unwrap_or("").to_string();
                    if !hint.is_empty() {
                        a.push(hint);
                    }
                }
                self.run_helper(h, "back", a);
            }
            "loginscreen.preview" => self.preview(h),
            _ => {}
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Probe(code) => {
                self.has_copy = code != 1;
                self.can_write = code == 0;
                if self.can_write {
                    self.arm();
                }
            }
            M::Share(s) => self.share = s,
            M::Switch(text) => {
                self.switch = read_switch_probe(&text, &install_hint());
                // Switched back by itself at the start: say so, once
                let seen = std::fs::read_to_string(seen_file()).unwrap_or_default();
                if let Some((title, body)) = revert_notice(&self.switch["record"], &seen) {
                    if !tessera_core::env::sandboxed() {
                        tessera_core::util::notify("Settings", "preferences-desktop-display", "normal", &title, &body);
                        let _ = std::fs::create_dir_all(tessera_core::util::state_dir());
                        let _ = tessera_core::util::write_atomic(&seen_file(), self.switch["record"]["time"].as_i64().unwrap_or(0).to_string().as_bytes());
                    }
                }
            }
            M::Done(code, err) => {
                self.busy.clear();
                self.error = switch_error(code, &err);
                self.probe();
            }
            M::Later => {
                if !self.can_write || tessera_core::env::sandboxed() || self.writing {
                    return;
                }
                let cur = std::fs::read_to_string(format!("{}/greeter.json", self.share)).unwrap_or_default();
                if cur.trim() == self.look {
                    return;
                }
                self.writing = true;
                let cmd = root_command(&["login-look", &self.look]);
                let tx = self.tx();
                std::thread::spawn(move || {
                    let (code, _, _) = run_full(&cmd, None);
                    tx.send(M::Written(code));
                });
            }
            M::Written(code) => {
                self.writing = false;
                self.status = if code == 0 { "The login screen has your look.".into() } else { "Couldn't give the login screen your look. It's tried again after your next change.".into() };
            }
        }
        self.publish(h);
    }
}

impl LoginScreen {
    fn preview(&mut self, h: &mut Host) {
        let cfg = h.cfg();
        let style = cfg["lock"]["loginStyle"].as_str().unwrap_or("look").to_string();
        let wall = tessera_core::wallpaper::showing_now(cfg, h.st.shell.look.material);
        let clock24 = if cfg["appearance"]["clock24"] == true { "1" } else { "0" };
        let shell_dir = crate::util::root().join("shell").to_string_lossy().into_owned();
        let sh = "QS=qs; for d in \"${XDG_DATA_HOME:-}/tessera\" \"$HOME/.local/share/tessera\"; do if [ -x \"$d/quickshell/bin/qs\" ]; then QS=\"$d/quickshell/bin/qs\"; export QML_IMPORT_PATH=\"$d/quickshell/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}\"; break; fi; done; TESSERA_GREETER_MOCK=1 TESSERA_GREETER_STYLE=\"$1\" TESSERA_GREETER_WALLPAPER=\"$2\" TESSERA_GREETER_CLOCK24=\"$4\" exec \"$QS\" -p \"$3/greeter.qml\"";
        let qml = vec!["sh".to_string(), "-c".into(), sh.into(), "sh".into(), style.clone(), wall, shell_dir, clock24.into()];
        // This binary's own login screen (tessera-ui --greeter), as a preview
        // (TESSERA_GREETER_MOCK: the password "tessera" "logs in", Escape quits);
        // the QML one only if it can't start
        let look = if h.st.shell.look.material { "material" } else { "glass" };
        // (" (deleted)": an update replaced this binary; the new one at that path previews just as well)
        let me = std::env::current_exe().map(|p| p.to_string_lossy().trim_end_matches(" (deleted)").to_string()).unwrap_or_default();
        let rust = (!me.is_empty()).then(|| {
            vec!["env".to_string(), "TESSERA_GREETER_MOCK=1".into(), format!("TESSERA_GREETER_CLOCK24={clock24}"), me, "--greeter".into(), "--style".into(), style, "--look".into(), look.into()]
        });
        let cmd = rust.unwrap_or(qml);
        std::thread::spawn(move || {
            let _ = run_full(&cmd, None);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_greeters_look_is_sorted_and_defaulted() {
        let s = look_json(&json!({}), true, "#112233ff");
        assert_eq!(s, r##"{"clock24":false,"clockFace":"dynamic","clockRound":true,"clockWeight":640,"clockWidth":112,"look":"glass","materialStyle":"tonalSpot","mode":"dark","seed":"#112233","style":"look"}"##);
        let s = look_json(&json!({"appearance": {"materialSeed": "#abcdef", "look": "material"}}), false, "nope");
        assert!(s.contains(r##""seed":"#abcdef""##) && s.contains("\"mode\":\"light\""));
        assert!(look_json(&json!({}), false, "zzz").contains("#6750A4"));
    }

    #[test]
    fn a_style_picked_before_the_switch_goes_with_it() {
        let s = look_json(&json!({}), true, "#112233");
        assert!(with_style(&s, "text").contains("\"style\":\"text\""));
        assert!(with_style(&s, "rm -rf").contains("\"style\":\"look\""));
        assert!(with_style(&s, "").contains("\"style\":\"look\""));
    }

    #[test]
    fn the_switch_probe_reads_the_record_and_flags() {
        let t = "dm:sddm\nrecord:{\"previous\":\"sddm\",\"state\":\"reverted\",\"reason\":\"x\",\"time\":5,\"boot\":\"b1\"}\nboot:b2\nswitcher\n";
        let v = read_switch_probe(t, "");
        assert_eq!(v["dm"], "sddm");
        assert_eq!(v["tessera"], false);
        assert_eq!(v["canSwitch"], true);
        assert_eq!(v["record"]["state"], "reverted");
        assert_eq!(v["boot"], "b2");
        let v = read_switch_probe("dm:greetd\ntessera\nrecord:\nboot:b\n", "ly\n");
        assert!(v["record"].is_null());
        assert_eq!((v["tessera"].as_bool(), v["canSwitch"].as_bool(), v["hint"].as_str()), (Some(true), Some(false), Some("ly")));
        // (not a unit's name: nothing)
        assert_eq!(read_switch_probe("dm:a b;c\n", "x y")["dm"], "");
        assert_eq!(read_switch_probe("", "x y")["hint"], "");
    }

    #[test]
    fn a_revert_is_told_once() {
        let r = json!({"previous": "sddm", "state": "reverted", "reason": "the login screen didn't appear", "time": 42});
        let (title, body) = revert_notice(&r, "").unwrap();
        assert_eq!(title, "SDDM is your login screen again");
        assert!(body.contains("because the login screen didn't appear"));
        assert!(revert_notice(&r, "42\n").is_none());
        assert!(revert_notice(&json!({"previous": "sddm", "state": "off", "time": 1}), "").is_none());
        assert!(revert_notice(&Value::Null, "").is_none());
        assert_eq!(revert_notice(&json!({"previous": "", "state": "reverted", "time": 1}), "").unwrap().0, "You sign in on the console again");
    }

    #[test]
    fn helper_errors_read_as_sentences() {
        assert_eq!(switch_error(0, ""), "");
        assert_eq!(switch_error(126, "dismissed"), "");
        assert!(switch_error(127, "").starts_with("Not allowed"));
        assert_eq!(switch_error(64, "tessera-root: couldn't install greetd: target not found\n"), "Couldn't install greetd: target not found.");
        assert!(switch_error(1, "").starts_with("That didn't work"));
        assert_eq!(dm_name("sddm"), "SDDM");
        assert_eq!(dm_name(""), "the console");
        assert_eq!(dm_name("ly"), "ly");
    }
}
