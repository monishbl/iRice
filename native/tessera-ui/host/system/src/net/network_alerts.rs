//! Island alerts when the network changes (replaces services/NetworkAlerts.qml):
//! joined or left a Wi-Fi network, a VPN, the hotspot, Ethernet. Something
//! says it changed (`nmcli monitor`, or `ip monitor link address` without
//! NetworkManager); 700 ms after it settles what's connected is compared
//! with the last look. (tessera-events' D-Bus path is the host-shell's; the
//! QML's own fallback, the monitor process, is what's used here.)

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use std::time::Duration;
use tessera_core::host::{Host, Service};

const NAME: &str = "NetworkAlerts";

/// name -> (kind, hotspot)
pub type Conns = BTreeMap<String, (String, bool)>;

pub fn kind_of(t: &str) -> &'static str {
    match t {
        "802-11-wireless" => "wifi",
        "vpn" | "wireguard" => "vpn",
        "802-3-ethernet" => "ethernet",
        _ => "",
    }
}

/// `nmcli -t` lines NAME:TYPE:DEVICE:MODE (the name may hold escaped colons)
pub fn parse_nm(text: &str) -> Conns {
    let mut now = Conns::new();
    for l in text.split('\n') {
        let f: Vec<String> = l.replace("\\:", "\u{1}").split(':').map(str::to_string).collect();
        if f.len() < 4 {
            continue;
        }
        let name = f[..f.len() - 3].join(":").replace('\u{1}', ":");
        let kind = kind_of(&f[f.len() - 3]);
        if !kind.is_empty() && f[f.len() - 2] != "lo" {
            now.insert(name, (kind.to_string(), f[f.len() - 1] == "ap"));
        }
    }
    now
}

/// A VPN interface's name, as people know it
pub fn vpn_name(n: &str) -> String {
    let t = |p: &str| n.starts_with(p);
    if t("tailscale") {
        "Tailscale".into()
    } else if t("nordlynx") {
        "NordVPN".into()
    } else if t("proton") {
        "Proton VPN".into()
    } else if t("mullvad") {
        "Mullvad".into()
    } else if t("zt") {
        "ZeroTier".into()
    } else {
        n.into()
    }
}

/// kind<TAB>interface<TAB>Wi-Fi name<TAB>access point (1)
pub fn parse_links(text: &str) -> Conns {
    let mut now = Conns::new();
    for l in text.split('\n') {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 4 {
            continue;
        }
        let (kind, iface, ssid, ap) = (f[0], f[1], f[2], f[3]);
        let name = match kind {
            "wifi" => if ssid.is_empty() { "Wi-Fi".to_string() } else { ssid.to_string() },
            "vpn" => vpn_name(iface),
            _ => format!("Ethernet {iface}"),
        };
        now.insert(name, (kind.to_string(), ap == "1"));
    }
    now
}

/// (name, kind, hotspot, up) for what changed between two looks
pub fn compare(before: &Conns, now: &Conns) -> Vec<(String, String, bool, bool)> {
    let mut out = vec![];
    for (n, c) in now {
        if !before.contains_key(n) {
            out.push((n.clone(), c.0.clone(), c.1, true));
        }
    }
    for (n, c) in before {
        if !now.contains_key(n) {
            out.push((n.clone(), c.0.clone(), c.1, false));
        }
    }
    out
}

/// (event, ms) of one alert; None for a kind with none
pub fn announce(name: &str, kind: &str, hotspot: bool, up: bool) -> Option<(Value, i64)> {
    let ev = |icon: &str, title: &str, sub: String, color: &str, ripple: Option<bool>| {
        let mut v = json!({"kind": "network", "icon": icon, "title": title, "subtitle": sub, "color": color});
        if let Some(r) = ripple {
            v["ripple"] = json!(r);
        }
        v
    };
    if hotspot {
        return Some((ev("broadcast", "Personal Hotspot", if up { format!("On · {name}") } else { "Off".into() }, if up { accent::GREEN } else { accent::GREY }, Some(up)), 3000));
    }
    match kind {
        "wifi" => Some((ev(if up { "wifi-high" } else { "wifi-slash" }, name, if up { "Wi-Fi connected".into() } else { "Wi-Fi disconnected".into() }, if up { accent::BLUE } else { accent::GREY }, None), if up { 3000 } else { 2500 })),
        "vpn" => Some((ev(if up { "shield-check" } else { "shield-slash" }, name, if up { "VPN connected".into() } else { "VPN disconnected".into() }, if up { accent::GREEN } else { accent::ORANGE }, Some(up)), 3000)),
        "ethernet" => Some((ev(if up { "plugs-connected" } else { "network-slash" }, "Ethernet", if up { "Connected".into() } else { "Unplugged".into() }, if up { accent::BLUE } else { accent::GREY }, Some(up)), 2500)),
        _ => None,
    }
}

const NM_LIST: &str = "nmcli -t -f NAME,TYPE,DEVICE connection show --active | while IFS= read -r l; do t=$(printf '%s' \"$l\" | awk -F: '{print $(NF-1)}'); m=; [ \"$t\" = 802-11-wireless ] && m=$(nmcli -g 802-11-wireless.mode connection show \"${l%:*:*}\" 2>/dev/null); printf '%s:%s\\n' \"$l\" \"$m\"; done";
const IP_LINKS: &str = "for d in /sys/class/net/*; do n=${d##*/}; [ \"$n\" = lo ] && continue; s=$(cat \"$d/operstate\" 2>/dev/null); [ \"$s\" = down ] || [ \"$s\" = lowerlayerdown ] && continue; [ -n \"$(ip -o addr show dev \"$n\" scope global 2>/dev/null)\" ] || continue; ssid=; ap=; if [ -d \"$d/wireless\" ] || [ -e \"$d/phy80211\" ]; then k=wifi; if command -v iw >/dev/null; then i=$(iw dev \"$n\" info 2>/dev/null); ssid=$(printf '%s\\n' \"$i\" | sed -n 's/^[[:space:]]*ssid //p' | head -n 1); printf '%s' \"$i\" | grep -q 'type AP' && ap=1; fi; else case \"$n\" in wg*|tailscale*|tun*|tap*|ppp*|nordlynx*|proton*|mullvad*|zt*|ipsec*|vti*) k=vpn ;; *) if [ -e \"$d/device\" ]; then k=ethernet; else continue; fi ;; esac; fi; printf '%s\\t%s\\t%s\\t%s\\n' \"$k\" \"$n\" \"$ssid\" \"$ap\"; done";

enum M {
    /// the watched process said something (restart the settle timer)
    Settle,
    Look,
    Result(String, String),
}

#[derive(Default)]
pub struct NetworkAlerts {
    mode: String,
    on: bool,
    last: Option<Conns>,
    watch: Option<Watch>,
    settle: slint::Timer,
}

impl NetworkAlerts {
    fn mode_now() -> String {
        if std::path::Path::new("/run/NetworkManager").exists() && has_tool("nmcli") {
            "nm".into()
        } else if has_tool("ip") {
            "ip".into()
        } else {
            String::new()
        }
    }

    /// Re-evaluate `mode` and `on`; start or stop the watching
    fn refresh(&mut self, h: &mut Host) {
        let mode = Self::mode_now();
        if mode != self.mode {
            self.mode = mode;
            self.last = None; // (another way of looking: learn the state again)
            if let Some(w) = self.watch.take() {
                w.stop();
            }
        }
        let cfg = h.cfg();
        let on = island_enabled(cfg) && island_wants(cfg, "network") && !self.mode.is_empty();
        if on && self.watch.is_none() {
            let send = h.sender(NAME);
            let s2 = send.clone();
            let cmd: Vec<&str> = if self.mode == "nm" { vec!["nmcli", "monitor"] } else { vec!["ip", "monitor", "link", "address"] };
            self.watch = Some(Watch::start(&cmd, None, move |_| send(Box::new(M::Settle)), move |_, _| {
                let _ = &s2;
            }));
            self.look(h);
        } else if !on {
            if let Some(w) = self.watch.take() {
                w.stop();
            }
        }
        self.on = on;
        crate::bus::put(
            NAME,
            json!({"mode": self.mode, "canWatch": !self.mode.is_empty(), "reason": if self.mode.is_empty() { "Needs NetworkManager, or the ip command (install iproute2)" } else { "" }, "on": on}),
        );
    }

    fn look(&mut self, h: &mut Host) {
        let send = h.sender(NAME);
        let mode = self.mode.clone();
        let cmd = match mode.as_str() {
            "nm" => sh_cmd(NM_LIST, &[]),
            "ip" => sh_cmd(IP_LINKS, &[]),
            _ => return,
        };
        capture_then(cmd, None, move |_, text| send(Box::new(M::Result(mode, text))));
    }
}

impl Service for NetworkAlerts {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["networkalerts"]
    }
    fn start(&mut self, h: &mut Host) {
        self.refresh(h);
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.refresh(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, _args: &Value) {
        // "networkalerts.look": look now (the first look only learns the state)
        if call.ends_with(".look") {
            self.look(h);
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Settle => {
                if !self.on {
                    return;
                }
                let send = h.sender(NAME);
                self.settle.start(slint::TimerMode::SingleShot, Duration::from_millis(700), move || send(Box::new(M::Look)));
            }
            M::Look => self.look(h),
            M::Result(mode, text) => {
                if mode != self.mode || !self.on {
                    return;
                }
                let now = if mode == "nm" { parse_nm(&text) } else { parse_links(&text) };
                let before = self.last.replace(now.clone());
                let Some(before) = before else { return }; // the first look only learns the state
                for (name, kind, hot, up) in compare(&before, &now) {
                    if let Some((ev, ms)) = announce(&name, &kind, hot, up) {
                        island_post(h, ev, ms);
                    }
                }
            }
        }
    }
    fn quit(&mut self, _h: &mut Host) {
        if let Some(w) = self.watch.take() {
            w.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nm_lines_with_colons_in_names() {
        let t = "Home\\: 5G:802-11-wireless:wlan0:infrastructure\nHotspot:802-11-wireless:wlan0:ap\nwg0:wireguard:wg0:\nlo:loopback:lo:\nWired:802-3-ethernet:eth0:\n";
        let c = parse_nm(t);
        assert_eq!(c["Home: 5G"], ("wifi".to_string(), false));
        assert_eq!(c["Hotspot"], ("wifi".to_string(), true));
        assert_eq!(c["wg0"].0, "vpn");
        assert!(!c.contains_key("lo") && c.contains_key("Wired"));
    }

    #[test]
    fn links_and_changes() {
        let a = parse_links("wifi\twlan0\tCafe\t\nvpn\ttailscale0\t\t\nethernet\teth0\t\t\n");
        assert!(a.contains_key("Cafe") && a.contains_key("Tailscale") && a.contains_key("Ethernet eth0"));
        let b = parse_links("wifi\twlan0\t\t1\n");
        let ch = compare(&a, &b);
        assert!(ch.contains(&("Wi-Fi".into(), "wifi".into(), true, true)));
        assert!(ch.contains(&("Cafe".into(), "wifi".into(), false, false)));
        let (ev, ms) = announce("Cafe", "wifi", false, false).unwrap();
        assert_eq!((ev["subtitle"].as_str(), ms), (Some("Wi-Fi disconnected"), 2500));
        assert_eq!(announce("Hot", "wifi", true, true).unwrap().0["subtitle"], "On · Hot");
    }
}
