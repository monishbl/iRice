//! LoginScreen.qml: the Tessera login screen (greetd): its look follows
//! yours. The greeter can't read your home, so the little it needs goes to
//! /usr/share/tessera/greeter.json through the root helper
//! (`tessera-root login-look`, no password for an administrator), 3 s after
//! a change and only when it differs from what is there. Never from the dev
//! sandbox; never before the helper's policy knows login-look.
//!
//! Bus "loginscreen": { installed, hasCopy, canWrite, status, share }.
//! Calls: loginscreen.preview (shows the login screen as it would look,
//! with a stand-in password; Escape closes it).

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
}

impl Default for LoginScreen {
    fn default() -> Self {
        Self { tx: None, has_copy: false, can_write: false, status: String::new(), share: "/usr/share/tessera".into(), writing: false, look: String::new(), later: slint::Timer::default() }
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

impl LoginScreen {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn publish(&self) {
        put("loginscreen", json!({"installed": self.has_copy, "hasCopy": self.has_copy, "canWrite": self.can_write, "status": self.status, "share": self.share}));
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
        // (/usr/share/irice: the login screen installed before iRice became Tessera)
        let t1 = tx.clone();
        std::thread::spawn(move || {
            let script = format!("[ -d /usr/share/tessera/shell ] || [ -d /usr/share/irice/shell ] || exit 1; {} && exit 0; exit 2", root_knows("login-look"));
            let (code, _, _) = run_full(&strs(&["sh", "-c", &script]), None);
            t1.send(M::Probe(code));
        });
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&strs(&["sh", "-c", "[ -d /usr/share/tessera ] || [ ! -d /usr/share/irice ] || echo old"]), None);
            if out.trim() == "old" {
                tx.send(M::Share("/usr/share/irice".into()));
            }
        });
        self.publish();
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.recompute(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, _args: &Value) {
        if call != "loginscreen.preview" {
            return;
        }
        let cfg = h.cfg();
        let style = cfg["lock"]["loginStyle"].as_str().unwrap_or("look").to_string();
        let wall = tessera_core::wallpaper::showing_now(cfg, h.st.shell.look.material);
        let clock24 = if cfg["appearance"]["clock24"] == true { "1" } else { "0" };
        let shell_dir = crate::util::root().join("shell").to_string_lossy().into_owned();
        let sh = "QS=qs; for d in \"${XDG_DATA_HOME:-}/tessera\" \"$HOME/.local/share/tessera\" \"${XDG_DATA_HOME:-}/irice\" \"$HOME/.local/share/irice\"; do if [ -x \"$d/quickshell/bin/qs\" ]; then QS=\"$d/quickshell/bin/qs\"; export QML_IMPORT_PATH=\"$d/quickshell/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}\"; break; fi; done; TESSERA_GREETER_MOCK=1 TESSERA_GREETER_STYLE=\"$1\" TESSERA_GREETER_WALLPAPER=\"$2\" TESSERA_GREETER_CLOCK24=\"$4\" exec \"$QS\" -p \"$3/greeter.qml\"";
        let cmd = vec!["sh".to_string(), "-c".into(), sh.into(), "sh".into(), style, wall, shell_dir, clock24.into()];
        std::thread::spawn(move || {
            let _ = run_full(&cmd, None);
        });
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
        let _ = h;
        self.publish();
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
}
