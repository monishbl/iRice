//! Account.qml: who is logged in, read once: the full name (passwd entry's)
//! and the picture (~/.face, or AccountsService's). Published on the bus as
//! "account": { login, fullName, name, picture, pictureUrl }.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

#[derive(Default)]
pub struct Account {
    login: String,
    full_name: String,
    picture: String,
}

struct Read(Vec<String>);

const SCRIPT: &str = "u=\"$(id -un)\"; echo \"$u\"; getent passwd \"$u\" | cut -d: -f5 | cut -d, -f1; for f in \"$HOME/.face\" \"$HOME/.face.icon\" \"/var/lib/AccountsService/icons/$u\"; do [ -s \"$f\" ] && [ -r \"$f\" ] && { echo \"$f\"; exit 0; }; done; echo";

impl Account {
    fn publish(&self) {
        let name = if self.full_name.is_empty() { &self.login } else { &self.full_name };
        put(
            "account",
            json!({"login": self.login, "fullName": self.full_name, "name": name, "picture": self.picture,
                   "pictureUrl": if self.picture.is_empty() { String::new() } else { format!("file://{}", self.picture) }}),
        );
    }
}

impl Service for Account {
    fn name(&self) -> &'static str {
        "Account"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["account"]
    }
    fn start(&mut self, h: &mut Host) {
        self.login = std::env::var("USER").unwrap_or_default();
        self.publish();
        let tx = Tx::new(h.sender("Account"));
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&strs(&["sh", "-c", SCRIPT]), None);
            tx.send(Read(out.split('\n').map(|s| s.trim().to_string()).collect()));
        });
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        if let Ok(r) = msg.downcast::<Read>() {
            let l = r.0;
            if let Some(u) = l.first().filter(|u| !u.is_empty()) {
                self.login = u.clone();
            }
            self.full_name = l.get(1).cloned().unwrap_or_default();
            self.picture = l.get(2).cloned().unwrap_or_default();
            self.publish();
        }
    }
    fn call(&mut self, _h: &mut Host, call: &str, _args: &Value) {
        if call == "account.get" {
            self.publish();
        }
    }
}
