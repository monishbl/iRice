//! Earbuds nearby (shell/services/FastPair.qml; assets/scripts/fastpair.py):
//! Google Fast Pair earbuds and headphones in pairing mode get a card in the
//! island with Connect; with the case open, their batteries (left, right,
//! case). Looks only while Bluetooth is on, lightly (continuously on the
//! charger). Settings -> Bluetooth turns it off (fastPair.enabled).
//! Calls: share.fastConnect [device], share.fastRepair [device, known].

use crate::localsend::js;
use crate::proc::{json_line, Keep};
use crate::{act, Job, Sharing};
use serde_json::{json, Value};
use std::collections::HashMap;
use tessera_core::host::Host;

pub struct FastPair {
    pub watch: Keep,
    pub devices: Vec<Value>,
    /// Said once per device for a while (a card that keeps coming back would nag):
    /// address (the paired one's, when known) -> time
    pub offered: HashMap<String, i64>,
    /// address -> time last seen with batteries
    pub battery_shown: HashMap<String, i64>,
    /// Left, right and case, from the adverts of earbuds paired here (an open
    /// case): paired address -> { left, right, case: { level, charging }, at }
    pub levels: HashMap<String, Value>,
    pub connecting: String,
    pub connecting_device: Option<Value>,
    pub busy: bool,
}

impl Default for FastPair {
    fn default() -> Self {
        Self { watch: Keep::new("fast-watch", "fast_rest"), devices: vec![], offered: HashMap::new(), battery_shown: HashMap::new(), levels: HashMap::new(), connecting: String::new(), connecting_device: None, busy: false }
    }
}

/// "L 80% · R 80% · Case 60%" (the batteries of one advert)
pub fn battery_text(b: &Value) -> String {
    let part = |label: &str, x: &Value| -> String {
        if x.is_null() {
            String::new()
        } else {
            format!("{label} {}%{}", x["level"].as_f64().unwrap_or(0.0), if x["charging"].as_bool().unwrap_or(false) { " \u{26a1}" } else { "" })
        }
    };
    [part("L", &b["left"]), part("R", &b["right"]), part("Case", &b["case"])].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" \u{b7} ")
}

/// The battery parts: [{ label, value (0..1), charging }]
pub fn parts(b: &Value) -> Value {
    let mut out = Vec::new();
    for (label, k) in [("L", "left"), ("R", "right"), ("Case", "case")] {
        let x = &b[k];
        if !x.is_null() {
            out.push(json!({"label": label, "value": x["level"].as_f64().unwrap_or(0.0) / 100.0, "charging": x["charging"].as_bool().unwrap_or(false)}));
        }
    }
    Value::Array(out)
}

/// LE adverts carry no name: an address isn't one
fn is_address(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 17 && b.chunks(3).enumerate().all(|(_, c)| c.len() >= 2 && c[0].is_ascii_hexdigit() && c[1].is_ascii_hexdigit() && (c.len() == 2 || c[2] == b':' || c[2] == b'-'))
}

impl Sharing {
    pub fn fast_levels_for(&self, address: &str) -> Value {
        match self.fast.levels.get(address) {
            Some(l) if tessera_core::util::now_ms() - l["at"].as_i64().unwrap_or(0) < 120_000 => parts(l),
            _ => json!([]),
        }
    }

    pub fn fast_sync(&mut self, h: &mut Host) {
        let wanted = self.cfg_bool("fastPair.enabled", true) && self.bluetooth_on(h);
        let cmd = vec!["python3".to_string(), "-I".into(), self.script("fastpair.py"), "watch".into()];
        self.fast.watch.sync(&mut self.sched, h.sender("Sharing"), wanted, &cmd, 10_000, true);
    }
    pub fn fast_exit(&mut self, h: &mut Host, gen: u64) {
        let wanted = self.cfg_bool("fastPair.enabled", true) && self.bluetooth_on(h);
        self.fast.watch.ended(&mut self.sched, gen, wanted, 10_000);
    }
    pub fn fast_line(&mut self, h: &mut Host, gen: u64, line: &str) {
        if !self.fast.watch.is(gen) {
            return;
        }
        let Some(m) = json_line(line) else { return };
        // (LE adverts carry no name: an address isn't one)
        self.fast.devices = m["devices"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|d| {
                        let mut d = d.clone();
                        if is_address(&js(&d["name"])) {
                            d["name"] = json!("");
                        }
                        d
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.fast_devices_changed(h);
    }

    fn paired_now(&self, h: &Host, d: &Value) -> bool {
        let bt = h.st.shell.get::<tessera_core::services::bluez::Bluetooth>();
        let known = js(&d["known"]["address"]);
        let name = js(&d["name"]);
        bt.is_some_and(|bt| bt.devices.iter().any(|b| b.paired && b.connected && ((!known.is_empty() && b.address == known) || (!b.name.is_empty() && b.name == name))))
    }
    fn own_audio_connected(&self, h: &Host) -> bool {
        let bt = h.st.shell.get::<tessera_core::services::bluez::Bluetooth>();
        bt.is_some_and(|bt| {
            bt.devices.iter().any(|b| {
                let s = format!("{} {}", b.icon, b.name).to_lowercase();
                b.paired && b.connected && ["audio", "headset", "headphone", "earbud"].iter().any(|w| s.contains(w))
            })
        })
    }

    fn fast_devices_changed(&mut self, h: &mut Host) {
        let now = tessera_core::util::now_ms();
        // (keyed by Bluetooth address, which earbuds change every ~15
        // minutes: entries past their 10 minutes go, or they'd pile up)
        self.fast.offered.retain(|_, t| now - *t <= 600_000);
        self.fast.battery_shown.retain(|_, t| now - *t <= 600_000);
        let own_audio = self.own_audio_connected(h);
        for d in self.fast.devices.clone() {
            // (earbuds keep advertising, from new addresses, while they
            // connect and after: offered once per pair of earbuds)
            let known_addr = js(&d["known"]["address"]);
            let id = if known_addr.is_empty() { js(&d["address"]) } else { known_addr.clone() };
            let rssi = d["rssi"].as_f64();
            let show_ui = d["showUi"].as_bool() != Some(false);
            let has_battery = !d["battery"].is_null();
            let known = !d["known"].is_null();
            if d["pairing"].as_bool().unwrap_or(false) && rssi.is_some_and(|r| r > -70.0) && now - self.fast.offered.get(&id).copied().unwrap_or(0) > 600_000 && !self.fast.busy && !self.paired_now(h, &d) {
                self.fast.offered.insert(id.clone(), now);
                // (`known`: paired with this computer before, from another
                // address: they're connected, not paired again)
                let kname = js(&d["known"]["name"]);
                let title = if !kname.is_empty() { kname } else if !js(&d["name"]).is_empty() { js(&d["name"]) } else { "Earbuds nearby".into() };
                self.island(
                    h,
                    json!({"kind": "custom", "key": format!("bt:{id}"), "icon": "headphones", "title": title,
                        "subtitle": if known { "Paired with this computer, not connected" } else { "Ready to pair with this computer" },
                        "action": "Connect", "onAction": act("share.fastConnect", json!([d]))}),
                    15000,
                );
            } else if has_battery && known {
                // (our own earbuds: their levels, and their own card)
                let a = known_addr.clone();
                let mut lv = d["battery"].clone();
                lv["at"] = json!(now);
                self.fast.levels.insert(a.clone(), lv);
                if show_ui && rssi.is_some_and(|r| r > -60.0) && now - self.fast.battery_shown.get(&a).copied().unwrap_or(0) > 60_000 {
                    let kname = js(&d["known"]["name"]);
                    self.island(
                        h,
                        json!({"kind": "device", "key": format!("bt:{a}"), "icon": "headphones", "title": if kname.is_empty() { "Earbuds".to_string() } else { kname },
                            "subtitle": if d["known"]["connected"].as_bool().unwrap_or(false) { "Connected" } else { "Case open nearby" }, "color": crate::BLUE, "batteries": parts(&d["battery"])}),
                        6000,
                    );
                }
                self.fast.battery_shown.insert(a, now);
            } else if has_battery && show_ui && rssi.is_some_and(|r| r > -60.0) && own_audio {
                // the case was opened (heard again after a quiet spell). Only
                // with your own earbuds connected, and these right here: the
                // advert can't say whose they are (that needs Fast Pair's
                // account keys, which come with Google's pairing)
                let a = js(&d["address"]);
                if now - self.fast.battery_shown.get(&a).copied().unwrap_or(0) > 60_000 {
                    let n = js(&d["name"]);
                    self.island(h, json!({"kind": "custom", "icon": "headphones", "title": if n.is_empty() { "Earbuds".to_string() } else { n }, "subtitle": battery_text(&d["battery"])}), 6000);
                }
                self.fast.battery_shown.insert(a, now);
            }
        }
    }

    // Pair, trust, connect (one at a time), in the offer's own card (its
    // key): "Connecting..." until it's done (pairing can take a minute), then
    // "Connected", as the Bluetooth alert says it too
    pub fn fast_connect(&mut self, h: &mut Host, d: &Value) {
        let name = if !js(&d["known"]["name"]).is_empty() { js(&d["known"]["name"]) } else { js(&d["name"]) };
        self.fast_run(h, d, vec!["connect".into(), js(&d["address"]), name]);
    }
    /// Paired before, but they won't take the old pairing (reset earbuds):
    /// forget it and pair afresh
    pub fn fast_repair(&mut self, h: &mut Host, d: &Value, known: &str) {
        let name = if !js(&d["known"]["name"]).is_empty() { js(&d["known"]["name"]) } else { js(&d["name"]) };
        self.fast_run(h, d, vec!["repair".into(), js(&d["address"]), known.into(), name]);
    }
    fn fast_run(&mut self, h: &mut Host, d: &Value, args: Vec<String>) {
        if self.fast.busy {
            return;
        }
        let kname = js(&d["known"]["name"]);
        self.fast.connecting = if !kname.is_empty() { kname } else if !js(&d["name"]).is_empty() { js(&d["name"]) } else { "the earbuds".into() };
        self.fast.connecting_device = Some(d.clone());
        let mut cmd = vec!["python3".to_string(), "-I".into(), self.script("fastpair.py")];
        cmd.extend(args);
        self.fast.busy = true;
        self.run(h, Job::FastConnect, &cmd, None);
        let key = format!("bt:{}", Self::fast_key(d));
        let title = self.fast.connecting.clone();
        self.island(h, json!({"kind": "custom", "key": key, "icon": "headphones", "title": title, "subtitle": "Connecting\u{2026} keep the case open nearby"}), 120_000);
    }
    fn fast_key(d: &Value) -> String {
        let k = js(&d["known"]["address"]);
        if k.is_empty() { js(&d["address"]) } else { k }
    }
    pub fn fast_connected(&mut self, h: &mut Host, stdout: &str) {
        let r = serde_json::from_str::<Value>(stdout.trim()).unwrap_or_default();
        let d = self.fast.connecting_device.take();
        self.fast.busy = false;
        // (newly paired earbuds have another address than their advert's)
        let addr = if !js(&r["address"]).is_empty() { js(&r["address"]) } else { d.as_ref().map(Self::fast_key).unwrap_or_default() };
        let key = format!("bt:{addr}");
        let old = d.as_ref().map(|d| format!("bt:{}", Self::fast_key(d)));
        if let Some(old) = old.filter(|o| *o != key) {
            h.ask("island.remove", json!({"key": old}));
        }
        let name = std::mem::take(&mut self.fast.connecting);
        if r["ok"].as_bool().unwrap_or(false) {
            // (unless the Bluetooth alert has said it, with the battery)
            self.island(h, json!({"kind": "device", "key": key, "icon": "headphones", "title": name, "subtitle": "Connected", "color": crate::BLUE, "unlessKind": "device"}), 3500);
        } else {
            let said = js(&r["error"]);
            let mut ev = json!({"kind": "custom", "key": key, "icon": "headphones", "title": format!("Couldn't connect {name}"), "subtitle": if said.is_empty() { "Try again from Bluetooth settings.".to_string() } else { said }, "color": crate::DANGER});
            if !r["known"].is_null() && d.is_some() {
                ev["action"] = json!("Pair again");
                ev["onAction"] = act("share.fastRepair", json!([d, js(&r["known"])]));
            }
            self.island(h, ev, if r["known"].is_null() { 6000 } else { 15000 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batteries_read_as_text() {
        let b = json!({"left": {"level": 80, "charging": true}, "case": {"level": 60}});
        assert_eq!(battery_text(&b), "L 80% \u{26a1} \u{b7} Case 60%");
        let p = parts(&b);
        assert_eq!(p[0]["label"], "L");
        assert_eq!(p[1]["value"], 0.6);
    }

    #[test]
    fn an_address_is_not_a_name() {
        assert!(is_address("AA:BB:CC:00:11:22"));
        assert!(is_address("aa-bb-cc-00-11-22"));
        assert!(!is_address("Pixel Buds"));
    }
}
