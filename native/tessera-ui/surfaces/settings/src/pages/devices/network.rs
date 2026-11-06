//! Wi-Fi & Bluetooth (pages/network.qml, with bluetooth.qml as its part):
//! Wi-Fi (the Wi-Fi menu's list, controlcenter/WifiList.qml, with its
//! password row, right-click menu, join form (WifiJoinForm.qml) and share
//! card (WifiShareCard.qml); the networks you've saved), the speed test
//! (network/SpeedTestCard.qml, its server and history), the connection's
//! details, then Bluetooth (BluetoothList.qml's two lists, on and off, Fast
//! Pair). Wi-Fi's networks and every action are the QML shell's
//! (RustBridgeDevices.qml: Quickshell's NetworkManager objects); Bluetooth's
//! devices are read here (services/bluez.rs), their extras (trusted,
//! pairing, the picture) and actions the shell's.

use super::super::super::model::{Ctx, MenuItem, Opt, Page, Row};
use super::{ask, copy, feed, place, s};
use crate::services::bluez::{self, Bluetooth};
use crate::services::status::Wifi;
use serde_json::{json, Value};

// ── Pure logic ──────────────────────────────────────────────────────────

/// WifiList.strengthIcon (signal 0..1)
pub fn strength_icon(s: f64) -> &'static str {
    if s > 0.66 {
        "wifi-high"
    } else if s > 0.33 {
        "wifi-medium"
    } else {
        "wifi-low"
    }
}

/// A Wi-Fi password the network can take (WifiList.passwordOk)
pub fn password_ok(t: &str) -> bool {
    let n = t.chars().count();
    (8..=63).contains(&n) || [5, 10, 13, 26].contains(&n)
}

/// What a network's row says under its name
pub fn wifi_status(n: &Value) -> &'static str {
    let b = |k: &str| n[k].as_bool().unwrap_or(false);
    if b("connected") {
        "Connected"
    } else if b("connecting") {
        "Connecting…"
    } else if b("failed") {
        "Couldn't connect"
    } else if b("known") {
        "Known network"
    } else {
        ""
    }
}

/// BluetoothList.deviceIcon (BlueZ's icon name)
pub fn device_icon(icon: &str) -> &'static str {
    if icon.contains("headset") || icon.contains("headphone") {
        "headphones"
    } else if icon.contains("phone") {
        "device-mobile"
    } else if icon.contains("keyboard") {
        "keyboard"
    } else if icon.contains("mouse") {
        "mouse"
    } else if icon.contains("audio") {
        "speaker-hifi"
    } else {
        "bluetooth"
    }
}

/// BluetoothList.statusOf
pub fn device_status(d: &bluez::Device, pairing: bool) -> String {
    if d.connected {
        match d.battery {
            Some(b) => format!("Connected · {b}%"),
            None => "Connected".into(),
        }
    } else if pairing {
        "Pairing…".into()
    } else if d.paired {
        "Not connected".into()
    } else {
        String::new()
    }
}

/// BluetoothList.sortedNow: "paired" or "nearby" (named, one per name), connected first
pub fn device_list<'a>(bt: &'a Bluetooth, which: &str) -> Vec<&'a bluez::Device> {
    let mut v: Vec<&bluez::Device> = bt.devices.iter().filter(|d| if which == "paired" { d.paired } else { !d.paired && !d.name.is_empty() }).collect();
    v.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.paired.cmp(&a.paired)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    let mut seen = std::collections::HashSet::new();
    v.retain(|d| d.paired || d.name.is_empty() || seen.insert(d.name.clone()));
    v.truncate(40);
    v
}

/// "1000" Mbit/s as the Connection row says it
pub fn ethernet_speed(speed: f64) -> String {
    if speed <= 0.0 {
        "Connected".into()
    } else if speed >= 1000.0 {
        format!("{} Gbit/s", speed / 1000.0)
    } else {
        format!("{speed} Mbit/s")
    }
}

/// What the join form is missing (WifiJoinForm.submit's problem)
pub fn form_problem(f: &Value) -> String {
    let g = |k: &str| s(&f[k]);
    let hidden = f["hidden"].as_bool().unwrap_or(false);
    let name = if hidden { g("name") } else { g("ssid") };
    let sec = g("security");
    let tls = g("eap") == "tls";
    let psk = g("psk").chars().count();
    if name.trim().is_empty() {
        "Type the network name.".into()
    } else if sec == "psk" && !(8..=63).contains(&psk) {
        "Wi-Fi passwords are 8 to 63 characters.".into()
    } else if sec == "enterprise" && g("identity").trim().is_empty() {
        "Type your username (often your email address).".into()
    } else if sec == "enterprise" && !tls && g("password").is_empty() {
        "Type your password.".into()
    } else if sec == "enterprise" && tls && (g("clientCert").is_empty() || g("privateKey").is_empty()) {
        "Choose your certificate and private key files.".into()
    } else if sec == "enterprise" && g("ca") == "file" && g("caFile").is_empty() {
        "Choose the CA certificate file, or use the system's certificates.".into()
    } else {
        String::new()
    }
}

/// The join form's request (WifiJoin.join's)
pub fn form_request(f: &Value) -> Value {
    let g = |k: &str| s(&f[k]);
    let hidden = f["hidden"].as_bool().unwrap_or(false);
    let tls = g("eap") == "tls";
    let ca = g("ca");
    json!({
        "ssid": if hidden { g("name") } else { g("ssid") }.trim(),
        "hidden": hidden,
        "security": g("security"),
        "eap": g("eap"), "phase2": g("phase2"), "identity": g("identity"), "anonymous": g("anonymous"), "domain": g("domain"),
        "ca": if ca == "file" { g("caFile") } else { ca },
        "clientCert": g("clientCert"), "privateKey": g("privateKey"),
        "password": if tls { String::new() } else { g("password") },
        "keyPassword": g("keyPassword"), "psk": g("psk"),
    })
}

/// A new form (WifiJoinForm's defaults)
fn new_form(ssid: &str, hidden: bool, security: &str) -> Value {
    json!({"ssid": ssid, "name": ssid, "hidden": hidden, "security": security, "eap": "peap", "phase2": "mschapv2", "identity": "", "password": "", "anonymous": "", "domain": "", "ca": "system", "caFile": "", "clientCert": "", "privateKey": "", "keyPassword": "", "psk": "", "problem": ""})
}

// ── Wi-Fi ───────────────────────────────────────────────────────────────

fn set_form(cx: &mut Ctx, k: &str, v: Value) {
    let mut f = cx.local.get("wifi.form").cloned().unwrap_or(Value::Null);
    if f.is_object() {
        f[k] = v;
        cx.local_set("wifi.form", f);
    }
}

/// Open the join form (WifiList's `form`): a saved network's settings come back from the shell
fn open_form(cx: &mut Ctx, ssid: &str, hidden: bool, security: &str) {
    cx.local_set("wifi.form", new_form(ssid, hidden, security));
    cx.local_set("wifi.asking", "");
    cx.local.remove("wifi.formFilled");
    ask(cx, "wifiForm", json!([if hidden { "" } else { ssid }]));
}

/// A network's row was clicked (WifiList's onClicked)
fn activate(cx: &mut Ctx, n: &Value) {
    let name = s(&n["name"]);
    let b = |k: &str| n[k].as_bool().unwrap_or(false);
    if b("connected") {
        ask(cx, "wifiDisconnect", json!([name]));
    } else if b("known") || b("open") {
        ask(cx, "wifiConnect", json!([name]));
    } else if b("enterprise") {
        open_form(cx, &name, false, "enterprise");
    } else {
        let now = cx.local_s("wifi.asking");
        let next = if now == name { String::new() } else { name };
        cx.local_set("wifi.asking", next.clone());
        cx.local_set("wifi.typed", "");
        cx.local_set("wifi.hint", "");
        ask(cx, "wifiAsking", json!([next]));
    }
}

fn share(cx: &mut Ctx, name: &str) {
    cx.local_set("wifi.asking", "");
    cx.local.remove("wifi.form");
    cx.local_set("wifi.sharing", true);
    ask(cx, "wifiShare", json!([name]));
}

/// A network's right-click menu (WifiList.openMenu): (item, what it does)
fn wifi_menu(n: &Value) -> Vec<(MenuItem, &'static str)> {
    let b = |k: &str| n[k].as_bool().unwrap_or(false);
    let mut items = Vec::new();
    if b("connected") {
        items.push((MenuItem::new("Disconnect"), "disconnect"));
    } else if b("known") || b("open") {
        items.push((MenuItem::new("Connect"), "connect"));
    }
    if !b("connected") && b("secured") {
        items.push((MenuItem::new(if b("enterprise") { "Connect with sign-in…" } else { "Connect with password…" }), "password"));
    }
    if (b("connected") || b("known")) && !b("enterprise") {
        items.push((MenuItem::new("Share network…"), "share"));
    }
    items.push((MenuItem::new("Copy name"), "copy"));
    if b("known") {
        items.push((MenuItem::new("Forget network").danger(), "forget"));
    }
    items
}

fn wifi_list_rows(cx: &mut Ctx, nets: &[Value], from: usize, to: usize, first: bool, last: bool) -> Row {
    let part: Vec<Value> = nets[from..to].to_vec();
    let mut data = Vec::new();
    for n in &part {
        let items = wifi_menu(n);
        let n2 = n.clone();
        let whats: Vec<&'static str> = items.iter().map(|x| x.1).collect();
        let mid = cx.on(move |cx, a| {
            let name = s(&n2["name"]);
            match whats.get(a.i.max(0) as usize).copied() {
                Some("disconnect") => ask(cx, "wifiDisconnect", json!([name])),
                Some("connect") => ask(cx, "wifiConnect", json!([name])),
                Some("password") => {
                    if n2["enterprise"].as_bool().unwrap_or(false) {
                        open_form(cx, &name, false, "enterprise");
                    } else {
                        cx.local_set("wifi.asking", name.clone());
                        cx.local_set("wifi.typed", "");
                        cx.local_set("wifi.hint", "");
                        ask(cx, "wifiAsking", json!([name]));
                    }
                }
                Some("share") => share(cx, &name),
                Some("copy") => copy(&name),
                Some("forget") => ask(cx, "wifiForget", json!([name])),
                _ => {}
            }
        });
        cx.menu(&mid, items.into_iter().map(|x| x.0).collect());
        let b = |k: &str| n[k].as_bool().unwrap_or(false);
        let style = (if b("secured") { 1 } else { 0 }) | (if b("busy") { 2 } else { 0 }) | (if b("connected") && !b("enterprise") { 4 } else { 0 });
        data.push(Opt::new(s(&n["name"])).detail(wifi_status(n)).icon(strength_icon(n["signal"].as_f64().unwrap_or(0.0))).on(b("connected")).style(style).id(mid));
    }
    let id = cx.on(move |cx, a| {
        let Some(n) = part.get(a.i.max(0) as usize) else { return };
        if a.s == "action" {
            share(cx, &s(&n["name"]));
        } else {
            activate(cx, n);
        }
    });
    let mut r = Row::custom("devices", "list").id(id).data(data);
    r.step = place(first, last);
    r
}

/// A form field's action: what's typed goes in the form
fn field(cx: &mut Ctx, key: &'static str) -> String {
    cx.on(move |cx, a| set_form(cx, key, Value::from(a.s.clone())))
}
/// A form choice's action: the n-th value
fn pick(cx: &mut Ctx, key: &'static str, values: &'static [&'static str]) -> String {
    cx.on(move |cx, a| {
        if let Some(v) = values.get(a.i.max(0) as usize) {
            set_form(cx, key, Value::from(*v));
        }
    })
}
/// A form file's action: Choose… opens a file chooser, its answer goes in the form
fn file(cx: &mut Ctx, key: &'static str, title: &'static str, filter: &'static str) -> String {
    cx.on(move |cx, a| {
        if a.i == 2 {
            cx.choose_file(&a.id, false, title, "~", filter);
        } else if a.i == 1 {
            set_form(cx, key, Value::from(a.s.clone()));
        }
    })
}

/// The join form's rows (WifiJoinForm.qml), each field kept as it's typed
fn join_form(cx: &mut Ctx, rows: &mut Vec<Row>, f: &Value, join: &Value) {
    let g = |k: &str| s(&f[k]);
    let hidden = f["hidden"].as_bool().unwrap_or(false);
    let sec = g("security");
    let eap = g("eap");
    let tls = eap == "tls";
    let inner = eap == "peap" || eap == "ttls";
    let ca = g("ca");
    let target = if hidden { g("name") } else { g("ssid") };
    let mine = s(&join["ssid"]) == target;
    let joining = join["busy"].as_bool().unwrap_or(false) && mine;
    let failure = if mine && s(&join["stage"]) == "error" { s(&join["error"]) } else { String::new() };

    rows.push(Row::new(if hidden { "Join other network".to_string() } else { g("ssid") })
        .detail(if hidden { "For a network that doesn't show its name." } else { "This network needs a username and password, or a certificate." }));
    if hidden {
        let id = field(cx, "name");
        rows.push(Row::new("Network name").field(g("name"), "Name", id));
        let id = pick(cx, "security", &["open", "psk", "enterprise"]);
        let i = ["open", "psk", "enterprise"].iter().position(|x| *x == sec).unwrap_or(0) as i32;
        rows.push(Row::new("Security").segmented(&["None", "Password", "Enterprise"], i, id).width(260.0));
        if sec == "psk" {
            let id = field(cx, "psk");
            rows.push(Row::new("Password").field(g("psk"), "8 to 63 characters", id).secret());
        }
    }
    if sec == "enterprise" {
        let methods = ["peap", "ttls", "tls", "pwd"];
        let id = pick(cx, "eap", &["peap", "ttls", "tls", "pwd"]);
        rows.push(Row::new("Sign-in method").segmented(&["PEAP", "TTLS", "TLS", "PWD"], methods.iter().position(|x| *x == eap).unwrap_or(0) as i32, id).width(260.0));
        if inner {
            let values = ["mschapv2", "pap", "gtc", "mschap"];
            let labels: Vec<String> = ["MSCHAPv2 (most networks)", "PAP", "GTC", "MSCHAP"].iter().map(|x| x.to_string()).collect();
            let id = pick(cx, "phase2", &["mschapv2", "pap", "gtc", "mschap"]);
            rows.push(Row::new("Inner authentication").choice(&labels, values.iter().position(|x| *x == g("phase2")).map_or(-1, |i| i as i32), id));
        }
        let id = field(cx, "identity");
        rows.push(Row::new("Username").field(g("identity"), "you@example.org", id));
        if !tls {
            let id = field(cx, "password");
            rows.push(Row::new("Password").field(g("password"), "", id).secret());
        }
        if tls {
            let id = file(cx, "clientCert", "Choose your certificate", "Certificates|*.pem *.crt *.cer *.der *.p12 *.pfx");
            rows.push(Row::new("Your certificate").path(g("clientCert"), "Choose file…", id));
            let id = file(cx, "privateKey", "Choose your private key", "Keys|*.pem *.key *.der *.p12 *.pfx");
            rows.push(Row::new("Private key").path(g("privateKey"), "Choose file…", id));
            let id = field(cx, "keyPassword");
            rows.push(Row::new("Private key password").field(g("keyPassword"), "If the key has one", id).secret());
        }
        let cas = ["system", "file", "none"];
        let id = pick(cx, "ca", &["system", "file", "none"]);
        rows.push(Row::new("Network certificate").segmented(&["System", "From file", "Don't check"], cas.iter().position(|x| *x == ca).unwrap_or(0) as i32, id).width(300.0));
        if ca == "file" {
            let id = file(cx, "caFile", "Choose the network's CA certificate", "Certificates|*.pem *.crt *.cer *.der");
            rows.push(Row::new("CA certificate").path(g("caFile"), "Choose file…", id));
        }
        if ca == "none" {
            rows.push(Row::text("Not recommended: anyone can pretend to be this network and collect your password.").tone(2));
        }
        if ca != "none" {
            let id = field(cx, "domain");
            rows.push(Row::new("Domain").detail("Recommended: the certificate must belong to this domain. Your organisation's setup guide names it.").field(g("domain"), "e.g. radius.example.org", id));
        }
        if !tls {
            let id = field(cx, "anonymous");
            rows.push(Row::new("Anonymous identity").field(g("anonymous"), "Optional, e.g. anonymous@example.org", id));
        }
    }
    let problem = g("problem");
    let said = if !problem.is_empty() { problem } else { failure };
    if !said.is_empty() {
        rows.push(Row::text(said).tone(2));
    }
    let id = cx.on(|cx, a| {
        let f = cx.local.get("wifi.form").cloned().unwrap_or(Value::Null);
        if a.i == 0 {
            ask(cx, "wifiDismiss", Value::Null);
            cx.local.remove("wifi.form");
            return;
        }
        let problem = form_problem(&f);
        set_form(cx, "problem", Value::from(problem.clone()));
        if problem.is_empty() {
            ask(cx, "wifiJoinForm", json!([form_request(&f)]));
            // (the typed secrets aren't kept once they're sent)
            for k in ["password", "keyPassword", "psk"] {
                set_form(cx, k, Value::from(""));
            }
        }
    });
    let stage = s(&join["stage"]);
    let label = if joining { if stage == "saving" { "Saving…" } else { "Joining…" } } else { "Join network" };
    rows.push(Row::new("").buttons(vec![Opt::new("Cancel"), Opt::new(label).primary().on(joining)], id));
}

pub fn network(cx: &mut Ctx, p: &mut Page) {
    p.title = "Network".into();
    cx.want("status");
    cx.want("bluez");
    let shell = cx.shell;
    let f = feed(cx);
    let wifi = &f["wifi"];
    let be = &f["backends"];
    let unmanaged = be["wifiUnmanaged"].as_bool().unwrap_or(false);
    let st = shell.get::<Wifi>();
    let device = st.map_or(wifi["device"].as_bool().unwrap_or(false), |w| w.present) || wifi["device"].as_bool().unwrap_or(false);
    let enabled = st.map_or(wifi["enabled"].as_bool().unwrap_or(false), |w| w.enabled);
    let nets: Vec<Value> = wifi["networks"].as_array().cloned().unwrap_or_default();

    // A saved enterprise network's settings came back: into the form, once
    if let (Some(saved), true) = (wifi["saved"].as_object(), cx.local.contains_key("wifi.form") && !cx.local_b("wifi.formFilled")) {
        if saved.get("exists").and_then(Value::as_bool) == Some(true) && saved.get("keyMgmt").and_then(Value::as_str) == Some("wpa-eap") {
            let r = Value::Object(saved.clone());
            let mut form = cx.local["wifi.form"].clone();
            let eap = s(&r["eap"]);
            if ["peap", "ttls", "tls", "pwd"].contains(&eap.as_str()) {
                form["eap"] = Value::from(eap);
            }
            if !s(&r["phase2"]).is_empty() {
                form["phase2"] = r["phase2"].clone();
            }
            for k in ["identity", "anonymous", "domain", "caFile", "clientCert", "privateKey"] {
                form[k] = Value::from(s(&r[k]));
            }
            form["ca"] = Value::from(if !s(&r["caFile"]).is_empty() { "file" } else if s(&r["systemCa"]) == "yes" { "system" } else { "none" });
            cx.local_set("wifi.form", form);
        }
        cx.local_set("wifi.formFilled", true);
    }
    // Joined: back to the list
    let join = wifi["join"].clone();
    if let Some(form) = cx.local.get("wifi.form").cloned() {
        let target = if form["hidden"].as_bool().unwrap_or(false) { s(&form["name"]) } else { s(&form["ssid"]) };
        if s(&join["stage"]) == "connected" && s(&join["ssid"]) == target.trim() {
            cx.local.remove("wifi.form");
        }
    }
    let share_card = &wifi["share"];
    if cx.local_b("wifi.sharing") && s(&share_card["stage"]).is_empty() && cx.local_b("wifi.shareSeen") {
        cx.local_set("wifi.sharing", false);
    }
    if !s(&share_card["stage"]).is_empty() {
        cx.local_set("wifi.shareSeen", true);
    }

    // ── Wi-Fi ──
    let mut rows: Vec<Row> = Vec::new();
    if !unmanaged {
        let connected = nets.iter().find(|n| n["connected"].as_bool() == Some(true)).map(|n| s(&n["name"]));
        let detail = if !device {
            "No Wi-Fi adapter found".to_string()
        } else if enabled {
            connected.map_or("On, not connected".to_string(), |n| format!("Connected to {n}"))
        } else {
            "Off".into()
        };
        let id = cx.on(move |cx, a| {
            if a.s == "button" {
                ask(cx, "wifiRescan", Value::Null);
            } else {
                ask(cx, "wifiEnable", json!([!enabled]));
            }
        });
        let mut r = Row::new("Wi-Fi").detail(detail).ctl("devices", "btn-switch", id);
        r.text = if enabled { "Scan again".into() } else { String::new() };
        r.data = vec![Opt::new("").icon("arrow-clockwise")];
        r.on = enabled;
        r.from = 1.0;
        rows.push(r);
    }
    if unmanaged {
        let can_start = be["canStartNm"].as_bool().unwrap_or(false);
        let app = s(&be["wifiApp"]);
        let action = if can_start { "Start NetworkManager".to_string() } else if !app.is_empty() { format!("Open {}", s(&be["wifiAppName"])) } else { String::new() };
        let mut r = Row::new(s(&be["wifiTitle"])).icon("wifi-slash").detail(s(&be["wifiReason"]));
        if !action.is_empty() {
            let id = cx.on(move |cx, _| ask(cx, if can_start { "startNm" } else { "openWifiApp" }, Value::Null));
            r = r.button(action, id).busy(s(&f["healthFixing"]) == "networkmanager");
        }
        rows.push(r);
    } else if enabled {
        if let Some(form) = cx.local.get("wifi.form").cloned() {
            join_form(cx, &mut rows, &form, &join);
        } else if cx.local_b("wifi.sharing") {
            let stage = s(&share_card["stage"]);
            let qr = s(&share_card["qr"]);
            let id = cx.on(|cx, a| match a.s.as_str() {
                "back" => {
                    cx.local_set("wifi.sharing", false);
                    cx.local_set("wifi.shareSeen", false);
                    ask(cx, "wifiShareClose", Value::Null);
                }
                "reveal" => ask(cx, "wifiShareReveal", Value::Null),
                "copy" => {
                    ask(cx, "wifiShareCopy", Value::Null);
                    cx.local_set("wifi.copied", crate::util::now_ms());
                }
                "install" => ask(cx, "installTools", json!([["qrencode", "zbar"]])),
                _ => {}
            });
            let error = s(&share_card["error"]);
            let pw = s(&share_card["password"]);
            let mut r = Row::custom("devices", "share").id(id);
            r.label = s(&share_card["ssid"]);
            r.text = if stage.is_empty() { "loading".into() } else { stage.clone() };
            if stage == "ready" && !qr.is_empty() {
                r.image = cx.picture(&qr, 336, 336);
            }
            r.high = error.clone();
            r.from = if error.contains("qrencode") { 1.0 } else { 0.0 };
            r.busy = f["needs"]["installing"].as_bool().unwrap_or(false);
            r.enabled = !f["updatesRunning"].as_bool().unwrap_or(false);
            r.on = !pw.is_empty();
            r.value = if pw.is_empty() { "••••••••".into() } else { pw };
            r.to = if share_card["hasPassword"].as_bool().unwrap_or(false) { 1.0 } else { 0.0 };
            let copied = cx.local.get("wifi.copied").and_then(Value::as_i64).unwrap_or(0);
            r.step = if crate::util::now_ms() - copied < 1500 { 1.0 } else { 0.0 };
            rows.push(r);
        } else {
            let asking = cx.local_s("wifi.asking");
            let at = nets.iter().position(|n| s(&n["name"]) == asking);
            if nets.is_empty() {
                let mut r = Row::custom("devices", "list");
                r.placeholder = if !device { "No Wi-Fi adapter".into() } else { "Looking for networks…".into() };
                rows.push(r);
            } else if let Some(i) = at {
                rows.push(wifi_list_rows(cx, &nets, 0, i + 1, true, false));
                // The password row under the network (WifiList's `asking`)
                let typed = cx.local_s("wifi.typed");
                let ready = password_ok(&typed);
                let name = asking.clone();
                let id = cx.on(move |cx, a| match a.s.as_str() {
                    "other" => {
                        open_form(cx, &name, false, "enterprise");
                    }
                    "cancel" => {
                        cx.local_set("wifi.asking", "");
                        ask(cx, "wifiAsking", json!([""]));
                    }
                    t => {
                        if a.f == 1.0 {
                            cx.local_set("wifi.typed", t);
                            cx.local_set("wifi.hint", "");
                            return;
                        }
                        let pw = cx.local_s("wifi.typed");
                        if !password_ok(&pw) {
                            cx.local_set("wifi.hint", if pw.is_empty() { "Type the password first." } else { "Wi-Fi passwords are at least 8 characters." });
                            return;
                        }
                        ask(cx, "wifiJoin", json!([name, pw]));
                        cx.local_set("wifi.asking", "");
                        cx.local_set("wifi.typed", "");
                    }
                });
                let mut r = Row::custom("devices", "join").id(id);
                r.text = typed;
                r.style = if ready { 1 } else { 0 };
                r.high = cx.local_s("wifi.hint");
                rows.push(r);
                if i + 1 < nets.len() {
                    rows.push(wifi_list_rows(cx, &nets, i + 1, nets.len(), false, false));
                }
            } else {
                rows.push(wifi_list_rows(cx, &nets, 0, nets.len(), true, false));
            }
            for n in &nets {
                let t = s(&n["failedText"]);
                if !t.is_empty() && s(&n["name"]) != asking {
                    rows.push(Row::text(t).tone(2));
                }
            }
            // Join other network…
            let id = cx.on(|cx, _| open_form(cx, "", true, "psk"));
            let mut r = Row::custom("devices", "list").id(id).data(vec![Opt::new("Join other network…").icon("plus")]);
            r.step = 2.0;
            rows.push(r);
        }
    }
    let g = p.group("Wi-Fi");
    g.footer(if unmanaged { "" } else { "Right-click a network to connect, disconnect, forget it or copy its name." });
    for r in rows {
        g.row(r);
    }

    // ── Known networks ──
    if !unmanaged {
        let known: Vec<Value> = wifi["known"].as_array().cloned().unwrap_or_default();
        let mut rows = Vec::new();
        if known.is_empty() {
            rows.push(Row::new("No saved networks").tone(1));
        }
        for k in &known {
            let (ssid, name) = (s(&k["ssid"]), s(&k["name"]));
            let ss = ssid.clone();
            let id = cx.on(move |cx, _| ask(cx, "wifiForget", json!([ss])));
            rows.push(Row::new(ssid.clone()).detail(if name != ssid { format!("Saved as {name}") } else { String::new() }).button("Forget", id).danger());
        }
        let fe = s(&wifi["forgetError"]);
        let g = p.group("Known networks");
        g.footer(if !fe.is_empty() { fe } else { "Networks this computer joins by itself when they're in range. Forget one to stop that and remove its saved password.".into() });
        for r in rows {
            g.row(r);
        }
    }

    // ── The speed test ──
    let sp = &f["speed"];
    let stt = &f["speedTest"];
    let ookla = s(&stt["backend"]) == "ookla";
    let id = cx.on(|cx, _| ask(cx, "speedTest", Value::Null));
    let mut r = Row::custom("devices", "speed").id(id);
    r.label = s(&sp["sub"]);
    r.text = { let b = s(&sp["button"]); if b.is_empty() { "Test speed".into() } else { b } };
    r.on = sp["running"].as_bool().unwrap_or(false);
    r.value = s(&sp["live"]);
    r.num = sp["progress"].as_f64().unwrap_or(0.0) as f32;
    r.busy = sp["stats"].as_bool().unwrap_or(false);
    let stage = s(&sp["stage"]);
    r.data = vec![
        Opt::new("Download").icon("arrow-down").detail(s(&sp["download"])).on(stage == "download"),
        Opt::new("Upload").icon("arrow-up").detail(s(&sp["upload"])).on(stage == "upload"),
        Opt::new("Ping").icon("timer").detail(s(&sp["ping"])).on(stage == "latency"),
    ];
    r.detail = s(&sp["extra"]);
    r.placeholder = s(&sp["verdict"]);
    r.low = s(&sp["server"]);
    r.high = s(&sp["error"]);
    r.step = 3.0;
    p.group("")
        .footer(format!(
            "Runs only when you click. A test takes about 20 seconds and uses up to {}. Under load is your ping while the line is busy: if it's much higher than the idle ping, calls and games lag while something downloads.",
            if ookla { "a few hundred MB" } else { "about 370 MB on a fast line, less on a slower one" }
        ))
        .row(r);

    // ── Connection ──
    let link = &stt["link"];
    let kind = s(&link["kind"]);
    let iface = s(&link["iface"]);
    let num = |k: &str| link[k].as_f64();
    let g = p.group("Connection");
    g.footer(if kind == "wifi" { "Signal above −67 dBm is good for calls and video; below −75 dBm, move closer to the router or switch to 5 GHz." } else { "" });
    g.row(Row::new("Not connected").visible(iface.is_empty()).detail("Connect to Wi-Fi or plug in a cable to see its details"));
    g.row(Row::new("Network").visible(kind == "wifi").shows(s(&link["ssid"])));
    if kind == "wifi" && link["signal"].is_number() {
        let mut r = Row::new("Signal").ctl("devices", "signal", String::new());
        r.num = num("quality").unwrap_or(0.0) as f32;
        r.value = format!("{} dBm · {}", num("signal").unwrap_or(0.0), s(&stt["signalWords"]));
        g.row(r);
    }
    g.row(Row::new("Ethernet").visible(kind == "wired").shows(ethernet_speed(link["speed"].as_f64().or_else(|| s(&link["speed"]).parse().ok()).unwrap_or(0.0))));

    // ── More options ──
    p.more(cx);
    {
        let band: Vec<String> = [s(&link["standard"]), s(&link["band"]), num("channel").filter(|c| *c != 0.0).map_or(String::new(), |c| format!("channel {c}")), num("width").filter(|w| *w != 0.0).map_or(String::new(), |w| format!("{w} MHz wide"))]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect();
        let rate = match (num("rx"), num("tx")) {
            (rx, Some(tx)) if tx != 0.0 => format!("↓ {} · ↑ {} Mbit/s", rx.unwrap_or(0.0).round(), tx.round()),
            (rx, _) => format!("{} Mbit/s", rx.unwrap_or(0.0).round()),
        };
        let ip = s(&stt["ip"]);
        p.group("Connection")
            .row(Row::new("Band").visible(kind == "wifi" && !s(&link["band"]).is_empty()).shows(band.join(" · ")))
            .row(Row::new("Link rate").visible(kind == "wifi" && (link["rx"].is_number() || link["tx"].is_number())).detail("The most the Wi-Fi link carries right now; your internet is usually slower").shows(rate))
            .row(Row::new("Security").visible(kind == "wifi" && !s(&link["security"]).is_empty()).shows(s(&link["security"])))
            .row(Row::new("Interface").visible(!iface.is_empty()).shows(iface.clone()))
            .row(Row::new("Public IP").visible(!ip.is_empty()).detail(s(&stt["isp"])).shows(ip));

        let ready = stt["ooklaReady"].as_bool().unwrap_or(false);
        let has = stt["hasOokla"].as_bool().unwrap_or(false);
        let footer = if ookla && !ready {
            s(&stt["ooklaInstall"])
        } else if ookla {
            format!("Ookla tests against a nearby speedtest.net server, picked automatically.{}", if has { "" } else { " Using speedtest-cli; Ookla's official speedtest (paru -S ookla-speedtest-bin) measures more, including latency under load." })
        } else {
            "Cloudflare tests against its nearest data centre, using curl. Nothing to install.".into()
        };
        let pick = cx.pick("network.speedTest", &["cloudflare", "ookla"]);
        p.group("Server").footer(footer).row(Row::new("Test with")
            .detail(if ready { if has { "Ookla's speedtest is installed" } else { "speedtest-cli is installed" } } else { "Ookla needs its speedtest app" })
            .segmented(&["Cloudflare", "Ookla"], if ookla { 1 } else { 0 }, pick)
            .enabled(!stt["running"].as_bool().unwrap_or(false))
            .width(220.0));

        let history: Vec<Value> = stt["history"].as_array().cloned().unwrap_or_default();
        let clear = cx.on(|cx, _| ask(cx, "speedClear", Value::Null));
        let g = p.group("History");
        g.footer(if history.is_empty() { "" } else { "The last 10 tests." });
        g.row(Row::new("No tests yet").visible(history.is_empty()).detail("Results appear here after you test your speed"));
        for h in &history {
            g.row(Row::new(s(&h["label"])).detail(s(&h["detail"])).shows(s(&h["value"])));
        }
        g.row(Row::new("Remove all results").visible(!history.is_empty()).button("Clear history", clear).danger());
    }
    p.end_more();

    p.part(cx, "Bluetooth", bluetooth);
}

// ── Bluetooth (bluetooth.qml) ───────────────────────────────────────────

/// A device's right-click menu (BluetoothList.openMenu)
fn bt_menu(d: &bluez::Device, pairing: bool, trusted: bool) -> Vec<(MenuItem, &'static str)> {
    let mut items = Vec::new();
    if d.connected {
        items.push((MenuItem::new("Disconnect"), "disconnect"));
    } else if d.paired {
        items.push((MenuItem::new("Connect"), "connect"));
    }
    if pairing {
        items.push((MenuItem::new("Stop pairing"), "cancel"));
    } else if !d.paired {
        items.push((MenuItem::new("Pair"), "pair"));
    }
    if d.paired {
        items.push((MenuItem::new(if trusted { "Don't connect automatically" } else { "Connect automatically" }), if trusted { "untrust" } else { "trust" }));
    }
    items.push((MenuItem::new("Copy address"), "copy"));
    if d.paired {
        items.push((MenuItem::new("Forget device").danger(), "forget"));
    }
    items
}

fn bt_list(cx: &mut Ctx, bt: &Bluetooth, which: &str, extras: &Value, empty: String) -> Row {
    let devs: Vec<bluez::Device> = device_list(bt, which).into_iter().cloned().collect();
    let mut data = Vec::new();
    for d in &devs {
        let x = &extras[&d.address];
        let pairing = x["pairing"].as_bool().unwrap_or(false);
        let trusted = x["trusted"].as_bool().unwrap_or(false);
        let items = bt_menu(d, pairing, trusted);
        let whats: Vec<&'static str> = items.iter().map(|i| i.1).collect();
        let addr = d.address.clone();
        let mid = cx.on(move |cx, a| {
            let what = match whats.get(a.i.max(0) as usize).copied() {
                Some("disconnect") => "btDisconnect",
                Some("connect") => "btConnect",
                Some("cancel") => "btCancelPair",
                Some("pair") => "btPair",
                Some("trust") => return ask(cx, "btTrust", json!([addr, true])),
                Some("untrust") => return ask(cx, "btTrust", json!([addr, false])),
                Some("copy") => return copy(&addr),
                Some("forget") => "btForget",
                _ => return,
            };
            ask(cx, what, json!([addr]));
        });
        cx.menu(&mid, items.into_iter().map(|i| i.0).collect());
        let picture = s(&x["picture"]);
        let size = if cx.material { 72 } else { 56 };
        let mut o = Opt::new(if d.name.is_empty() { d.address.clone() } else { d.name.clone() })
            .detail(device_status(d, pairing))
            .icon(device_icon(&d.icon))
            .on(d.connected)
            .style(if x["busy"].as_bool().unwrap_or(false) { 2 } else { 0 })
            .id(mid);
        if !picture.is_empty() {
            if let Some(img) = cx.picture(&picture, size, size) {
                o = o.image(img);
            }
        }
        data.push(o);
    }
    let id = cx.on(move |cx, a| {
        let Some(d) = devs.get(a.i.max(0) as usize) else { return };
        let what = if d.connected { "btDisconnect" } else if d.paired { "btConnect" } else { "btPair" };
        ask(cx, what, json!([d.address]));
    });
    let mut r = Row::custom("devices", "list").id(id).data(data);
    r.placeholder = empty;
    r.step = 3.0;
    r
}

pub fn bluetooth(cx: &mut Ctx, p: &mut Page) {
    let shell = cx.shell;
    let bt: Bluetooth = shell.get::<Bluetooth>().cloned().unwrap_or_default();
    let f = feed(cx);
    let be = &f["backends"];
    let state = s(&be["btState"]);
    let broken = state != "ok" && !state.is_empty();
    let adapter = bt.present || f["bt"]["adapter"].as_bool().unwrap_or(false);
    let on = if shell.get::<Bluetooth>().is_some() { bt.powered } else { f["bt"]["enabled"].as_bool().unwrap_or(false) };
    let name = s(&f["bt"]["name"]);
    let extras = f["bt"]["devices"].clone();

    let detail = if broken {
        s(&be["btReason"])
    } else if !adapter {
        "Not available".into()
    } else if on {
        format!("On · this computer appears as “{}”", if name.is_empty() { "this computer" } else { name.as_str() })
    } else {
        "Off".into()
    };
    let fix = s(&be["btFix"]);
    let fixing = be["btFixing"].as_bool().unwrap_or(false);
    let id = cx.on(move |cx, a| {
        if a.s == "button" {
            ask(cx, if broken { "btFix" } else { "btRescan" }, Value::Null);
        } else {
            ask(cx, "btEnable", json!([!on]));
        }
    });
    let mut r = Row::new(if broken { s(&be["btTitle"]) } else { "Bluetooth".into() }).detail(detail).ctl("devices", "btn-switch", id);
    if broken && !fix.is_empty() {
        r.text = if fixing { "Working…".into() } else { s(&be["btFixLabel"]) };
        r.style = 1;
        r.busy = fixing;
    } else if !broken && on {
        r.text = "Look again".into();
        r.data = vec![Opt::new("").icon("arrow-clockwise")];
    }
    r.from = if broken { 0.0 } else { 1.0 };
    r.on = on;
    r.enabled = adapter;
    p.group("").footer(s(&be["btFixError"])).row(r);

    if !broken && on {
        let mine = bt_list(cx, &bt, "paired", &extras, "No devices yet. Pair one from Nearby devices.".into());
        p.group("My devices")
            .footer("Click a device to connect or disconnect it. Right-click for more: connect automatically, forget, copy its address.")
            .row(mine);
        let nearby = bt_list(cx, &bt, "nearby", &extras, "Looking for devices…".into());
        p.group("Nearby devices")
            .footer("Looking for devices while this page is open. Put the device in pairing mode to see it here, then click it to pair.")
            .row(nearby);
    }

    let fp = cx.b("fastPair.enabled");
    let t = cx.toggle("fastPair.enabled");
    p.group("")
        .footer("Earbuds and headphones with Google Fast Pair (Pixel Buds, many Sony, JBL, Bose, OnePlus and others): open the case near the computer to connect them in one click, and to see their batteries. Looks only while Bluetooth is on.")
        .row(Row::new("Show earbuds nearby").switch(fp, t));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_rows_say_what_wifilist_says() {
        assert_eq!(strength_icon(0.9), "wifi-high");
        assert_eq!(strength_icon(0.5), "wifi-medium");
        assert_eq!(strength_icon(0.1), "wifi-low");
        assert_eq!(wifi_status(&json!({"connected": true})), "Connected");
        assert_eq!(wifi_status(&json!({"known": true, "failed": true})), "Couldn't connect");
        assert_eq!(wifi_status(&json!({"known": true})), "Known network");
        assert!(password_ok("12345678") && password_ok("12345") && !password_ok("1234567"));
        let m: Vec<&str> = wifi_menu(&json!({"known": true, "secured": true})).iter().map(|x| x.1).collect();
        assert_eq!(m, ["connect", "password", "share", "copy", "forget"]);
    }

    #[test]
    fn the_join_form_says_whats_missing() {
        let mut f = new_form("", true, "psk");
        assert_eq!(form_problem(&f), "Type the network name.");
        f["name"] = json!("Cafe");
        assert_eq!(form_problem(&f), "Wi-Fi passwords are 8 to 63 characters.");
        f["psk"] = json!("12345678");
        assert_eq!(form_problem(&f), "");
        assert_eq!(form_request(&f)["ssid"], json!("Cafe"));
        let mut e = new_form("Work", false, "enterprise");
        assert!(form_problem(&e).starts_with("Type your username"));
        e["identity"] = json!("me");
        e["eap"] = json!("tls");
        assert!(form_problem(&e).starts_with("Choose your certificate"));
    }

    #[test]
    fn bluetooth_lists_as_bluetoothlist() {
        let d = |n: &str, paired: bool, connected: bool| bluez::Device { name: n.into(), address: n.into(), paired, connected, ..Default::default() };
        let bt = Bluetooth { present: true, powered: true, devices: vec![d("b", true, false), d("a", true, true), d("x", false, false), d("x", false, false), d("", false, false)] };
        let mine: Vec<&str> = device_list(&bt, "paired").iter().map(|d| d.name.as_str()).collect();
        assert_eq!(mine, ["a", "b"]);
        assert_eq!(device_list(&bt, "nearby").len(), 1);
        assert_eq!(device_icon("audio-headset"), "headphones");
        assert_eq!(device_status(&bluez::Device { connected: true, battery: Some(80), ..Default::default() }, false), "Connected · 80%");
        assert_eq!(ethernet_speed(1000.0), "1 Gbit/s");
        assert_eq!(ethernet_speed(100.0), "100 Mbit/s");
    }
}
