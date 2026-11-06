//! Wi-Fi networks as QR codes (replaces services/WifiShare.qml), through
//! assets/scripts/wifi-qr.py: the network you're on (or a saved one) as a code
//! another phone joins, and a Wi-Fi code in a screenshot offering to join.
//! The code is a private file in the runtime folder, removed when the card
//! closes.
//!
//! Calls (prefix "wifishare"): share(name), close, scan(file) (the island
//! offer), find(file) (the codes in a picture, put on the bus as
//! found/foundFile), join(w), copy(password). State on the bus as
//! "wifishare": {stage, ssid, password, security, qr, error}.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

const NAME: &str = "WifiShare";

enum M {
    Shared(u32, String),
    Scanned(String, String, bool),
}

#[derive(Default)]
pub struct WifiShare {
    /// "" | loading | ready | error
    stage: String,
    ssid: String,
    password: String,
    security: String,
    qr: String,
    error: String,
    request: u32,
    found: Value,
    found_file: String,
}

fn forget_file(file: &str) {
    let cmd = script_cmd("wifi-qr.py", &["forget", file]);
    let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
    crate::util::run_async(&c);
}

/// A code a computer can join from: open, or WPA with a real password
pub fn joinable(w: &Value) -> bool {
    let sec = w["security"].as_str().unwrap_or("");
    sec == "nopass" || ((sec == "WPA" || sec == "SAE") && w["password"].as_str().unwrap_or("").len() >= 8)
}

impl WifiShare {
    fn publish(&self) {
        crate::bus::put(
            NAME,
            json!({"stage": self.stage, "ssid": self.ssid, "password": self.password, "security": self.security, "qr": self.qr,
                   "error": self.error, "found": self.found, "foundFile": self.found_file}),
        );
    }

    fn close(&mut self) {
        if !self.qr.is_empty() {
            forget_file(&self.qr);
        }
        self.request += 1;
        self.stage.clear();
        self.qr.clear();
        self.password.clear(); // (not kept once the card is gone)
        self.error.clear();
    }

    fn offer(&self, h: &mut Host, w: &Value) {
        let can = joinable(w);
        let pw = w["password"].as_str().unwrap_or("");
        let open = w["security"] == "nopass";
        let mut ev = json!({
            "kind": "custom", "icon": "wifi-high", "title": "Wi-Fi network in your screenshot",
            "subtitle": format!("{}{}", w["ssid"].as_str().unwrap_or(""), if open { " · open network" } else { "" }),
            "color": accent::BLUE,
        });
        if can {
            ev["action"] = json!("Join");
            ev["actionCall"] = json!("wifishare.join");
            ev["actionArgs"] = json!([w]);
        } else if !pw.is_empty() {
            ev["action"] = json!("Copy Password");
            ev["actionCall"] = json!("wifishare.copy");
            ev["actionArgs"] = json!([pw]);
        }
        if can && !pw.is_empty() {
            ev["action2"] = json!("Copy Password");
            ev["action2Call"] = json!("wifishare.copy");
            ev["action2Args"] = json!([pw]);
        }
        island_post(h, ev, 12000);
    }
}

impl Service for WifiShare {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["wifishare"]
    }
    fn start(&mut self, _h: &mut Host) {
        self.publish();
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_list(args);
        match call.strip_prefix("wifishare.").unwrap_or("") {
            "share" => {
                let name = str_arg(&a, 0);
                self.close();
                self.ssid = name.clone();
                self.stage = "loading".into();
                self.request += 1;
                let (req, send) = (self.request, h.sender(NAME));
                capture_then(script_cmd("wifi-qr.py", &["share", &name]), None, move |_, t| send(Box::new(M::Shared(req, t))));
            }
            "close" => self.close(),
            "scan" | "find" => {
                let file = str_arg(&a, 0);
                let offer = call.ends_with("scan");
                let send = h.sender(NAME);
                let f2 = file.clone();
                capture_then(script_cmd("wifi-qr.py", &["scan", &file]), None, move |_, t| send(Box::new(M::Scanned(f2, t, offer))));
            }
            "join" => {
                if let Some(w) = a.first() {
                    let open = w["security"] == "nopass";
                    // WifiJoin.join({ ssid, hidden, security, psk })
                    h.ask("wifijoin.join", json!({"ssid": w["ssid"], "hidden": w["hidden"], "security": if open { "open" } else { "psk" }, "psk": w["password"]}));
                }
            }
            "copy" => {
                let pw = str_arg(&a, 0);
                if !pw.is_empty() {
                    crate::util::run_async(&["wl-copy", "--", &pw]);
                }
            }
            _ => {}
        }
        self.publish();
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Shared(req, text) => {
                let r: Value = serde_json::from_str(&text).unwrap_or(json!({}));
                if req != self.request || self.stage != "loading" {
                    // (closed, or another network asked for, meanwhile: its code
                    // holds the password, so it isn't left behind)
                    if let Some(q) = r["qr"].as_str() {
                        forget_file(q);
                    }
                    return;
                }
                if r["ok"] == true {
                    self.qr = r["qr"].as_str().unwrap_or("").into();
                    self.password = r["password"].as_str().unwrap_or("").into();
                    self.security = r["security"].as_str().unwrap_or("").into();
                    self.stage = "ready".into();
                } else {
                    self.error = r["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("The network couldn't be shared.").into();
                    self.stage = "error".into();
                }
            }
            M::Scanned(file, text, offer) => {
                let found = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["found"].as_array().cloned()).unwrap_or_default();
                if offer {
                    for w in &found {
                        self.offer(h, w);
                    }
                }
                self.found = json!(found);
                self.found_file = file;
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_computer_can_join() {
        assert!(joinable(&json!({"security": "nopass", "password": ""})));
        assert!(joinable(&json!({"security": "WPA", "password": "12345678"})));
        assert!(!joinable(&json!({"security": "WPA", "password": "short"})));
        assert!(!joinable(&json!({"security": "WEP", "password": "12345678"})));
    }
}
