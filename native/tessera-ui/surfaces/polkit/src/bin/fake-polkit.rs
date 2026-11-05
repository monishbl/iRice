//! A stand-in polkit Authority for testing tessera-ui's agent in the
//! sandbox, on the SESSION bus only (never the system bus: the real polkit
//! is never touched). It owns org.freedesktop.PolicyKit1 there, takes the
//! agent's RegisterAuthenticationAgent, then asks it once with a fake action
//! (BeginAuthentication) and prints how that ended:
//!   fake-polkit: registered :1.23 /org/tessera/PolicyKit1/AuthenticationAgent
//!   fake-polkit: result ok | cancelled (<error>)
//! Run it inside the arena (its DBUS_SESSION_BUS_ADDRESS), with tessera-ui
//! started with TESSERA_POLKIT_TEST=1 and TESSERA_POLKIT_HELPER=<a fake helper>.

use std::collections::HashMap;
use std::sync::mpsc;
use zbus::zvariant::{OwnedValue, Value};

struct Authority {
    tx: mpsc::Sender<(String, String)>,
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.Authority")]
impl Authority {
    fn register_authentication_agent(&self, #[zbus(header)] h: zbus::message::Header<'_>, subject: (String, HashMap<String, OwnedValue>), locale: String, object_path: String) {
        let sender = h.sender().map(|s| s.to_string()).unwrap_or_default();
        println!("fake-polkit: registered {sender} {object_path} (subject {}, locale {locale})", subject.0);
        let _ = self.tx.send((sender, object_path));
    }
    fn unregister_authentication_agent(&self, _subject: (String, HashMap<String, OwnedValue>), object_path: String) {
        println!("fake-polkit: unregistered {object_path}");
    }
}

fn main() {
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").map_or(true, |a| a.is_empty()) {
        eprintln!("fake-polkit: no session bus (refusing to run anywhere else)");
        std::process::exit(2);
    }
    let (tx, rx) = mpsc::channel();
    let conn = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.name("org.freedesktop.PolicyKit1"))
        .and_then(|b| b.serve_at("/org/freedesktop/PolicyKit1/Authority", Authority { tx }))
        .and_then(|b| b.build())
        .expect("fake-polkit: the session bus");
    println!("fake-polkit: waiting for an agent");
    let message = std::env::args().nth(1).unwrap_or_else(|| "Authentication is required to change the test's settings.".into());
    let uid = unsafe { libc::getuid() };
    let ids: Vec<(String, HashMap<String, Value>)> = vec![("unix-user".into(), HashMap::from([("uid".to_string(), Value::U32(uid))]))];
    let details: HashMap<String, String> = HashMap::new();
    // (each agent that registers is asked once: the shell may start tessera-ui
    // again, and an agent that quits refuses what it was asked; until one says ok)
    while let Ok((agent, path)) = rx.recv() {
        let r = conn.call_method(
            Some(agent.as_str()),
            path.as_str(),
            Some("org.freedesktop.PolicyKit1.AuthenticationAgent"),
            "BeginAuthentication",
            &("org.tessera.test.change-settings", message.as_str(), "", &details, "fake-cookie-1", &ids),
        );
        match r {
            Ok(_) => {
                println!("fake-polkit: result ok");
                return;
            }
            Err(e) => println!("fake-polkit: result cancelled ({e})"),
        }
    }
}
