//! BootSplash.qml: the boot animation (Settings → Lock & Login Screen):
//! Plymouth's theme, read from its config (readable by all), and set through
//! the root helper (`tessera-root bootsplash <id>`, always asks for the
//! password: it rebuilds the boot image), only when the user picks one.
//!
//! Bus "bootsplash": checked, installed, helper, splash, hooked, builder,
//! theme, record, designs, original, chosen, blocked, pending, error, done,
//! busy. Calls: bootsplash.refresh (Settings does it when its page opens),
//! bootsplash.set [id].

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

enum M {
    Probe(String),
    Set(i32, String),
}

#[derive(Default)]
pub struct BootSplash {
    tx: Option<Tx>,
    checked: bool,
    installed: bool,
    helper: bool,
    splash: bool,
    hooked: bool,
    builder: String,
    theme: String,
    record: Value,
    pending: String,
    error: String,
    done: bool,
    probing: bool,
}

/// `^(tessera|irice)-(.+)$` → the id (irice-<id>: set before iRice became Tessera)
pub fn design_id(theme: &str) -> &str {
    theme.strip_prefix("tessera-").or_else(|| theme.strip_prefix("irice-")).filter(|r| !r.is_empty()).unwrap_or("")
}

impl BootSplash {
    fn designs() -> Value {
        let p = crate::util::root().join("shell/assets/plymouth/designs.json");
        let v = crate::util::read_json(&p);
        if v.is_array() { v } else { json!([]) }
    }
    fn original(&self) -> String {
        if self.record.is_object() && self.record["applied"].as_str() == Some(self.theme.as_str()) {
            self.record["original"].as_str().unwrap_or("").to_string()
        } else {
            self.theme.clone()
        }
    }
    fn chosen(&self, designs: &Value) -> String {
        let id = design_id(&self.theme);
        if designs.as_array().is_some_and(|d| d.iter().any(|x| x["id"].as_str() == Some(id))) {
            return id.to_string();
        }
        if self.theme == "text" && self.original() != "text" { "none".into() } else { "system".into() }
    }
    /// Why a choice can't be made here, and what to do (else "")
    fn blocked(&self) -> &'static str {
        if !self.checked {
            ""
        } else if !self.installed {
            "Your computer starts without a boot animation: Plymouth isn't installed. Install the plymouth package, add plymouth to HOOKS in /etc/mkinitcpio.conf, then come back here."
        } else if self.builder == "dracut" {
            "This computer builds its boot image with dracut, which Tessera doesn't rebuild. To use one of these, run in a terminal: sudo plymouth-set-default-theme -R tessera-breathe (or another design's name)."
        } else if self.builder == "booster" {
            "This computer builds its boot image with booster, which doesn't show boot animations."
        } else if self.builder.is_empty() {
            "There's no mkinitcpio to rebuild the boot image with."
        } else if !self.hooked {
            "Plymouth isn't in the boot image. Add plymouth to HOOKS in /etc/mkinitcpio.conf (after kms), then come back here."
        } else if !self.helper {
            "Update Tessera first (Settings → Software Update): the update installs the boot animations. Then pick one here."
        } else {
            ""
        }
    }
    fn publish(&self) {
        let designs = Self::designs();
        put(
            "bootsplash",
            json!({"checked": self.checked, "installed": self.installed, "helper": self.helper, "splash": self.splash, "hooked": self.hooked,
                   "builder": self.builder, "theme": self.theme, "record": self.record, "designs": designs, "original": self.original(),
                   "chosen": self.chosen(&designs), "blocked": self.blocked(), "pending": self.pending, "error": self.error, "done": self.done,
                   "busy": !self.pending.is_empty()}),
        );
    }
    fn probe(&mut self) {
        if self.probing {
            return;
        }
        self.probing = true;
        let script = [
            "command -v plymouth-set-default-theme >/dev/null && test -d /usr/share/plymouth/themes && echo installed".to_string(),
            "t=$(sed -n 's/^[[:space:]]*Theme[[:space:]]*=[[:space:]]*//p' /etc/plymouth/plymouthd.conf 2>/dev/null | tail -n 1)".into(),
            "[ -n \"$t\" ] || t=$(sed -n 's/^[[:space:]]*Theme[[:space:]]*=[[:space:]]*//p' /usr/share/plymouth/plymouthd.defaults 2>/dev/null | tail -n 1)".into(),
            "echo \"theme:$t\"".into(),
            "echo \"record:$(head -c 512 /etc/tessera/bootsplash.json 2>/dev/null || head -c 512 /etc/irice/bootsplash.json 2>/dev/null)\"".into(),
            "grep -qw splash /proc/cmdline && echo splash".into(),
            "for b in mkinitcpio dracut booster; do command -v $b >/dev/null && { echo \"builder:$b\"; break; }; done".into(),
            "grep -hE '^[[:space:]]*HOOKS[+]?=' /etc/mkinitcpio.conf /etc/mkinitcpio.conf.d/*.conf 2>/dev/null | grep -qw plymouth && echo hooked".into(),
            format!("{} && {{ test -d /usr/lib/tessera/plymouth/tessera-breathe || test -d /usr/lib/irice/plymouth/irice-breathe; }} && echo helper", root_knows("bootsplash")),
            ":".into(),
        ]
        .join("; ");
        let tx = self.tx.clone().expect("started");
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&strs(&["sh", "-c", &script]), None);
            tx.send(M::Probe(out));
        });
    }
}

/// The probe's lines → (flags by name, value(k))
pub fn read_probe(text: &str) -> impl Fn(&str) -> String + '_ {
    move |k: &str| text.lines().find(|l| l.starts_with(&format!("{k}:"))).map(|l| l[k.len() + 1..].trim().to_string()).unwrap_or_default()
}

impl Service for BootSplash {
    fn name(&self) -> &'static str {
        "BootSplash"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["bootsplash"]
    }
    fn start(&mut self, h: &mut Host) {
        self.tx = Some(Tx::new(h.sender("BootSplash")));
        self.splash = true;
        self.hooked = true;
        self.publish();
    }
    fn call(&mut self, _h: &mut Host, call: &str, args: &Value) {
        match call.split('.').nth(1).unwrap_or("") {
            "refresh" => self.probe(),
            "set" => {
                let id = args.get(0).and_then(Value::as_str).unwrap_or("").to_string();
                let chosen = self.chosen(&Self::designs());
                if !self.pending.is_empty() || id == chosen || !self.blocked().is_empty() {
                    return;
                }
                self.error.clear();
                self.done = false;
                if tessera_core::env::sandboxed() {
                    self.error = "Not from the test session: it would change this computer's boot animation.".into();
                    self.publish();
                    return;
                }
                if !has_tool("pkexec") {
                    self.error = NO_PKEXEC.into();
                    self.publish();
                    return;
                }
                self.pending = id.clone();
                let cmd = root_command(&["bootsplash", &id]);
                let tx = self.tx.clone().expect("started");
                std::thread::spawn(move || {
                    let (code, _, err) = run_full(&cmd, None);
                    tx.send(M::Set(code, err));
                });
                self.publish();
            }
            _ => {}
        }
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Probe(text) => {
                self.probing = false;
                let lines: Vec<&str> = text.lines().collect();
                self.installed = lines.contains(&"installed");
                self.splash = lines.contains(&"splash");
                self.hooked = lines.contains(&"hooked");
                self.helper = lines.contains(&"helper");
                let value = read_probe(&text);
                self.theme = value("theme");
                self.builder = value("builder");
                let r: Value = serde_json::from_str(&value("record")).unwrap_or(Value::Null);
                self.record = if r["original"].is_string() && r["applied"].is_string() { r } else { Value::Null };
                self.checked = true;
            }
            M::Set(code, err) => {
                self.pending.clear();
                self.done = code == 0;
                // (126: the password prompt was dismissed; nothing changed)
                if code != 0 && code != 126 {
                    self.error = if code == 127 {
                        "Not allowed: an administrator's password is needed.".into()
                    } else {
                        let m = root_message(&err);
                        let mut c = m.chars();
                        match c.next() {
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                            None => "That didn't change. Update Tessera, then try again.".into(),
                        }
                    };
                }
                self.probe();
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_names_and_probe_lines() {
        assert_eq!(design_id("tessera-breathe"), "breathe");
        assert_eq!(design_id("irice-breathe"), "breathe");
        assert_eq!(design_id("spinner"), "");
        let t = "installed\ntheme:spinner\nbuilder:mkinitcpio\n";
        let v = read_probe(t);
        assert_eq!((v("theme").as_str(), v("builder").as_str()), ("spinner", "mkinitcpio"));
    }
}
