//! Joining the Wi-Fi networks the system API can't, and the saved ones
//! (replaces services/WifiJoin.qml): enterprise (802.1X) and hidden networks
//! through assets/scripts/wifi-enterprise.py. The request, password included,
//! goes to it on stdin, never on a command line. Here, not in a menu, so a
//! join carries on when the menu that started it closes.
//!
//! Calls (prefix "wifijoin"): join(req), dismiss, saved(name) (the answer in
//! the bus under "saved"), loadKnown, forget(name). State on the bus as
//! "wifijoin": {ssid, stage, error, busy, knownNetworks, forgetError, saved}.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

const NAME: &str = "WifiJoin";

enum M {
    Line(String),
    Exit(i32),
    Saved(String),
    Known(String),
    ForgetLine(String),
    ForgetExit(i32),
}

#[derive(Default)]
pub struct WifiJoin {
    ssid: String,
    /// "" | saving | connecting | connected | error
    stage: String,
    error: String,
    busy: bool,
    known: Vec<Value>,
    forget_error: String,
    forgetting: bool,
    show_busy: bool,
    known_busy: bool,
    saved: Value,
}

impl WifiJoin {
    fn publish(&self) {
        crate::bus::put(
            NAME,
            json!({"ssid": self.ssid, "stage": self.stage, "error": self.error, "busy": self.busy,
                   "knownNetworks": self.known, "forgetError": self.forget_error, "saved": self.saved}),
        );
    }

    /// WifiJoin.join(req): false while one runs
    fn join(&mut self, h: &mut Host, req: &Value) -> bool {
        if self.busy {
            return false;
        }
        self.ssid = req["ssid"].as_str().unwrap_or("").to_string();
        self.stage = "saving".into();
        self.error.clear();
        self.busy = true;
        let (a, b) = (h.sender(NAME), h.sender(NAME));
        let cmd = script_cmd("wifi-enterprise.py", &["join"]);
        let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
        Watch::start(&c, Some(format!("{req}\n")), move |l| a(Box::new(M::Line(l))), move |code, _| b(Box::new(M::Exit(code))));
        true
    }

    fn load_known(&mut self, h: &mut Host) {
        if self.known_busy {
            return;
        }
        self.known_busy = true;
        let send = h.sender(NAME);
        capture_then(script_cmd("wifi-enterprise.py", &["known"]), None, move |_, t| send(Box::new(M::Known(t))));
    }

    fn forget(&mut self, h: &mut Host, name: &str) {
        if self.forgetting || name.is_empty() {
            return;
        }
        self.forgetting = true;
        self.forget_error.clear();
        let (a, b) = (h.sender(NAME), h.sender(NAME));
        let cmd = script_cmd("wifi-enterprise.py", &["forget", name]);
        let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
        Watch::start(&c, None, move |l| a(Box::new(M::ForgetLine(l))), move |code, _| b(Box::new(M::ForgetExit(code))));
    }
}

fn last_json(t: &str) -> Value {
    let l = t.trim().split('\n').next_back().filter(|l| !l.is_empty()).unwrap_or("{}");
    serde_json::from_str(l).unwrap_or(Value::Null)
}

impl Service for WifiJoin {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["wifijoin"]
    }
    fn start(&mut self, _h: &mut Host) {
        self.publish();
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_list(args);
        match call.strip_prefix("wifijoin.").unwrap_or("") {
            "join" => {
                if let Some(req) = a.first() {
                    self.join(h, &req.clone());
                }
            }
            "dismiss" => {
                if !self.busy {
                    self.stage.clear();
                    self.error.clear();
                }
            }
            "saved" => {
                if !self.show_busy {
                    self.show_busy = true;
                    let send = h.sender(NAME);
                    capture_then(script_cmd("wifi-enterprise.py", &["show", &str_arg(&a, 0)]), None, move |_, t| send(Box::new(M::Saved(t))));
                }
            }
            "loadKnown" => self.load_known(h),
            "forget" => {
                let n = str_arg(&a, 0);
                self.forget(h, &n);
            }
            _ => {}
        }
        self.publish();
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Line(l) => {
                if let Ok(m) = serde_json::from_str::<Value>(&l) {
                    if let Some(s) = m["state"].as_str() {
                        self.stage = s.to_string();
                        if s == "error" {
                            self.error = m["message"].as_str().unwrap_or("Couldn't join the network.").to_string();
                        }
                    }
                }
            }
            M::Exit(code) => {
                self.busy = false;
                if code != 0 && self.stage != "error" {
                    self.stage = "error".into();
                    self.error = "Couldn't join the network. Check that python3 and NetworkManager are installed.".into();
                }
            }
            M::Saved(t) => {
                self.show_busy = false;
                let r = last_json(&t);
                self.saved = if r.is_null() { json!({"exists": false}) } else { r };
            }
            M::Known(t) => {
                self.known_busy = false;
                if let Some(k) = last_json(&t)["known"].as_array() {
                    self.known = k.clone();
                }
            }
            M::ForgetLine(l) => {
                if let Ok(m) = serde_json::from_str::<Value>(&l) {
                    if m["state"] == "error" {
                        self.forget_error = m["message"].as_str().unwrap_or("Couldn't forget the network.").to_string();
                    }
                }
            }
            M::ForgetExit(code) => {
                self.forgetting = false;
                if code != 0 && self.forget_error.is_empty() {
                    self.forget_error = "Couldn't forget the network. Check that NetworkManager is running.".into();
                }
                self.load_known(h);
            }
        }
        self.publish();
    }
}

/// The `Wifi` view the menus show: is this network being joined (busy)?
pub fn joining(name: &str) -> bool {
    let s = crate::bus::get(NAME);
    s["busy"] == true && s["ssid"] == name
}

/// WifiJoin.failedFor(name): why joining it failed ("" when it didn't)
pub fn failed_for(name: &str) -> String {
    let s = crate::bus::get(NAME);
    if s["stage"] == "error" && s["ssid"] == name {
        s["error"].as_str().unwrap_or("").to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_line_of_json() {
        assert_eq!(last_json("noise\n{\"known\":[1]}\n")["known"][0], 1);
        assert!(last_json("nope").is_null());
        assert!(last_json("")["x"].is_null());
    }
}
