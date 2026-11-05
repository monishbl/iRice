//! The Wi-Fi menu's model: what the shell's feed says (NetworkFeed.qml), and
//! the pure rules the QML follows (WifiList.qml, SpeedTestCard.qml), as
//! functions with tests.

use serde_json::Value;

/// One network, as the feed sends it (WifiList's row)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Net {
    pub name: String,
    pub signal: f32,
    pub known: bool,
    pub connected: bool,
    /// WifiSecurityType.Open (Unknown is neither open nor secured)
    pub open: bool,
    pub secured: bool,
    pub enterprise: bool,
    /// "Connected", "Connecting…", "Couldn't connect", "Known network" or ""
    pub status: String,
    pub busy: bool,
    /// WifiJoin.failedFor: why joining went wrong ("" when it didn't)
    pub failed: String,
}

/// Wi-Fi without NetworkManager (ServiceNotice)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notice {
    pub title: String,
    pub text: String,
    pub action: String,
    pub busy: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wifi {
    pub adapter: bool,
    pub enabled: bool,
    pub unmanaged: bool,
    pub notice: Notice,
    /// connected first, then known, then by signal, at most 30 (the feed's
    /// snapshot: sortedNow())
    pub networks: Vec<Net>,
}

/// Download, upload or ping in the speed test card
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stat {
    pub label: String,
    pub icon: String,
    pub value: String,
    pub active: bool,
}

/// The speed test, with the strings SpeedTest.qml formats
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Speed {
    pub running: bool,
    pub stage: String,
    /// the line under the title: the phase, or "Just now · 12 MB used"
    pub sub: String,
    /// when the last saved result was taken (ms), for "Last test … ago"; 0: none
    pub last_at: i64,
    /// the live number (download/upload speed, or the ping while measuring it)
    pub live: String,
    pub progress: f32,
    pub show_stats: bool,
    pub stats: Vec<Stat>,
    pub resp: String,
    pub verdict: String,
    pub server: String,
    pub error: String,
    pub action: String,
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_default().to_string()
}
fn b(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}

pub fn net_from(v: &Value) -> Net {
    Net {
        name: s(v, "name"),
        signal: v["signal"].as_f64().unwrap_or(0.0) as f32,
        known: b(v, "known"),
        connected: b(v, "connected"),
        open: b(v, "open"),
        secured: b(v, "secured"),
        enterprise: b(v, "enterprise"),
        status: s(v, "status"),
        busy: b(v, "busy"),
        failed: s(v, "failed"),
    }
}

pub fn wifi_from(v: &Value) -> Wifi {
    let n = &v["notice"];
    Wifi {
        adapter: b(v, "adapter"),
        enabled: b(v, "enabled"),
        unmanaged: b(v, "unmanaged"),
        notice: Notice { title: s(n, "title"), text: s(n, "text"), action: s(n, "action"), busy: b(n, "busy") },
        networks: v["networks"].as_array().map(|a| a.iter().map(net_from).filter(|n| !n.name.is_empty()).take(30).collect()).unwrap_or_default(),
    }
}

pub fn speed_from(v: &Value) -> Speed {
    Speed {
        running: b(v, "running"),
        stage: s(v, "stage"),
        sub: s(v, "sub"),
        last_at: v["lastAt"].as_f64().unwrap_or(0.0) as i64,
        live: s(v, "live"),
        progress: v["progress"].as_f64().unwrap_or(0.0) as f32,
        show_stats: b(v, "showStats"),
        stats: v["stats"]
            .as_array()
            .map(|a| a.iter().map(|x| Stat { label: s(x, "label"), icon: s(x, "icon"), value: s(x, "value"), active: b(x, "active") }).collect())
            .unwrap_or_default(),
        resp: s(v, "resp"),
        verdict: s(v, "verdict"),
        server: s(v, "server"),
        error: s(v, "error"),
        action: if v["action"].is_string() { s(v, "action") } else { "Test speed".into() },
    }
}

/// SpeedTestCard's ago(): how long since `at`, both in ms
pub fn ago(now: i64, at: i64) -> String {
    let m = ((now - at) as f64 / 60000.0).round() as i64;
    if m < 1 {
        "just now".into()
    } else if m < 60 {
        format!("{m} min ago")
    } else if m < 1440 {
        format!("{} h ago", (m as f64 / 60.0).round() as i64)
    } else {
        format!("{} d ago", (m as f64 / 1440.0).round() as i64)
    }
}

/// The card's subtitle: the feed's (the phase, "Just now · …"), else the
/// last saved test's age
pub fn subtitle(sp: &Speed, now: i64) -> String {
    if !sp.sub.is_empty() {
        sp.sub.clone()
    } else if sp.last_at > 0 && !sp.running && sp.stage != "error" {
        format!("Last test {}", ago(now, sp.last_at))
    } else {
        String::new()
    }
}

/// WifiList's passwordOk: WPA needs 8 to 63 characters; WEP keys are 5 or
/// 13 (or 10 or 26 in hex)
pub fn password_ok(t: &str) -> bool {
    let n = t.chars().count();
    (8..=63).contains(&n) || [5, 10, 13, 26].contains(&n)
}

/// Why Join didn't go ahead (None: it does)
pub fn password_hint(t: &str) -> Option<&'static str> {
    if password_ok(t) {
        None
    } else if t.is_empty() {
        Some("Type the password first.")
    } else {
        Some("Wi-Fi passwords are at least 8 characters.")
    }
}

/// What a click on a network does (WifiList's row onClicked)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Disconnect,
    Connect,
    /// enterprise: the full form, here Settings → Network
    Settings,
    /// secured and unknown: the inline password (again: put away)
    Ask,
}

pub fn click(n: &Net) -> Click {
    if n.connected {
        Click::Disconnect
    } else if n.known || n.open {
        Click::Connect
    } else if n.enterprise {
        Click::Settings
    } else {
        Click::Ask
    }
}

/// The right-click menu's actions (WifiList's openMenu)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Disconnect,
    Connect,
    Password,
    SignIn,
    Share,
    Copy,
    Forget,
    Details,
}

/// One entry: None is a separator; (text, what, danger)
pub type Entry = Option<(&'static str, Act, bool)>;

pub fn menu(n: &Net) -> Vec<Entry> {
    let mut v: Vec<Entry> = Vec::new();
    if n.connected {
        v.push(Some(("Disconnect", Act::Disconnect, false)));
    } else if n.known || n.open {
        v.push(Some(("Connect", Act::Connect, false)));
    }
    if !n.connected && n.secured {
        v.push(Some(if n.enterprise { ("Connect with sign-in…", Act::SignIn, false) } else { ("Connect with password…", Act::Password, false) }));
    }
    v.push(None);
    if (n.connected || n.known) && !n.enterprise {
        v.push(Some(("Share network…", Act::Share, false)));
    }
    v.push(Some(("Copy name", Act::Copy, false)));
    if n.known {
        v.push(Some(("Forget network", Act::Forget, true)));
    }
    v.push(None);
    v.push(Some(("Network details…", Act::Details, false)));
    v
}

/// WifiList's strengthIcon
pub fn strength_icon(s: f32) -> &'static str {
    if s > 0.66 {
        "wifi-high"
    } else if s > 0.33 {
        "wifi-medium"
    } else {
        "wifi-low"
    }
}

/// What the list says in place of networks ("" when there are some)
pub fn empty_text(w: &Wifi) -> &'static str {
    if w.enabled && !w.networks.is_empty() {
        ""
    } else if !w.adapter {
        "No Wi-Fi adapter"
    } else if !w.enabled {
        "Wi-Fi is off"
    } else {
        "Looking for networks…"
    }
}

/// Material's segmented list: which rows start and end a group. A row's
/// password prompt or its failure note under it breaks the group, as the
/// QML's ListRow finds its neighbours (`asking`: the row waiting for a
/// password)
pub fn groups(nets: &[Net], asking: Option<&str>) -> Vec<(bool, bool)> {
    let broken = |n: &Net| asking == Some(n.name.as_str()) || !n.failed.is_empty();
    (0..nets.len())
        .map(|i| {
            let first = i == 0 || broken(&nets[i - 1]);
            let last = i + 1 == nets.len() || broken(&nets[i]);
            (first, last)
        })
        .collect()
}

/// A wrapped text's height, near enough (Slint measures wrapped text in a
/// layout as one line): `size` px type in `width` px
pub fn wrap_height(text: &str, width: f32, size: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let per_line = (width / (size * 0.53)).max(1.0);
    let mut lines = 0.0f32;
    for para in text.split('\n') {
        lines += (para.chars().count() as f32 / per_line).ceil().max(1.0);
    }
    (lines * (size * 1.3)).ceil()
}

/// Material's wavy bar (WavyProgress.qml): a sine of amplitude 3 and
/// wavelength 40 over `width` + one wavelength, in a 10 px high box, as SVG
/// path commands (drawn once; Slint slides it)
pub fn wave(width: f32) -> String {
    let w = width + 40.0;
    let mut d = String::new();
    let mut x = 0.0f32;
    while x <= w + 0.01 {
        let y = 5.0 + 3.0 * (2.0 * std::f32::consts::PI * x / 40.0).sin();
        d += &format!("{}{x:.1} {y:.2} ", if x == 0.0 { "M " } else { "L " });
        x += 2.0;
    }
    d
}

/// The bar lies flat near 0 and 100% (Google's)
pub fn flat(progress: f32) -> bool {
    let p = progress.clamp(0.0, 1.0);
    p <= 0.1 || p >= 0.95
}

/// Keys the popover takes from its focus scope (Slint wants the answer at once)
pub fn handles(text: &str) -> bool {
    use slint::platform::Key;
    [Key::Escape, Key::Tab, Key::Backtab, Key::UpArrow, Key::DownArrow, Key::Return]
        .iter()
        .any(|k| text == slint::SharedString::from(*k).as_str())
        || text == " "
}

/// The keyboard's next place among `n` focusable things (-1: none yet)
pub fn step(cur: i32, n: i32, d: i32) -> i32 {
    if n <= 0 {
        return -1;
    }
    if cur < 0 {
        return if d > 0 { 0 } else { n - 1 };
    }
    (cur + d).rem_euclid(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn net(name: &str) -> Net {
        Net { name: name.into(), secured: true, ..Default::default() }
    }

    #[test]
    fn the_feed_reads_back() {
        let w = wifi_from(&json!({"adapter": true, "enabled": true, "networks": [
            {"name": "Cafe Guest", "signal": 0.8, "open": true, "status": ""},
            {"name": "", "signal": 0.2},
            {"name": "Library", "signal": 0.4, "known": true, "secured": true, "status": "Known network"}
        ]}));
        assert_eq!(w.networks.len(), 2);
        assert!(w.networks[0].open && !w.networks[0].secured);
        assert_eq!(w.networks[1].status, "Known network");
        let sp = speed_from(&json!({"running": true, "stage": "download", "stats": [{"label": "Download", "icon": "arrow-down", "value": "88.2 Mbps", "active": true}]}));
        assert!(sp.running && sp.stats[0].active);
        assert_eq!(speed_from(&json!({})).action, "Test speed");
    }

    #[test]
    fn how_long_ago() {
        let now = 1_000_000_000;
        assert_eq!(ago(now, now - 20_000), "just now");
        assert_eq!(ago(now, now - 5 * 60_000), "5 min ago");
        assert_eq!(ago(now, now - 59 * 60_000), "59 min ago");
        assert_eq!(ago(now, now - 90 * 60_000), "2 h ago"); // (1.5 h rounds up, as Math.round)
        assert_eq!(ago(now, now - 3 * 1440 * 60_000), "3 d ago");
    }

    #[test]
    fn the_cards_subtitle() {
        let now = 10_000_000;
        let mut sp = Speed { last_at: now - 10 * 60_000, ..Default::default() };
        assert_eq!(subtitle(&sp, now), "Last test 10 min ago");
        sp.stage = "error".into();
        assert_eq!(subtitle(&sp, now), "");
        sp.stage = "download".into();
        sp.running = true;
        sp.sub = "Testing download…".into();
        assert_eq!(subtitle(&sp, now), "Testing download…");
        assert_eq!(subtitle(&Speed::default(), now), "");
    }

    #[test]
    fn passwords() {
        assert!(!password_ok(""));
        assert!(password_ok("12345")); // WEP
        assert!(!password_ok("123456"));
        assert!(password_ok("eight888"));
        assert!(password_ok(&"x".repeat(63)));
        assert!(!password_ok(&"x".repeat(64)));
        assert!(password_ok(&"x".repeat(26)));
        assert_eq!(password_hint(""), Some("Type the password first."));
        assert_eq!(password_hint("short"), None);
        assert_eq!(password_hint("shorter"), Some("Wi-Fi passwords are at least 8 characters."));
        // (characters, not bytes)
        assert!(password_ok("ééééé"));
    }

    #[test]
    fn what_a_click_does() {
        let mut n = net("Cafe Guest");
        assert_eq!(click(&n), Click::Ask);
        n.enterprise = true;
        assert_eq!(click(&n), Click::Settings);
        n.known = true;
        assert_eq!(click(&n), Click::Connect);
        n.connected = true;
        assert_eq!(click(&n), Click::Disconnect);
        let open = Net { name: "Park".into(), open: true, ..Default::default() };
        assert_eq!(click(&open), Click::Connect);
    }

    #[test]
    fn the_rows_menu() {
        let texts = |n: &Net| menu(n).iter().map(|e| e.map_or("-", |x| x.0)).collect::<Vec<_>>();
        let mut n = net("Cafe Guest");
        assert_eq!(texts(&n), ["Connect with password…", "-", "Copy name", "-", "Network details…"]);
        n.known = true;
        assert_eq!(texts(&n), ["Connect", "Connect with password…", "-", "Share network…", "Copy name", "Forget network", "-", "Network details…"]);
        n.connected = true;
        assert_eq!(texts(&n)[0], "Disconnect");
        assert!(menu(&n).iter().flatten().any(|e| e.1 == Act::Forget && e.2));
        let mut e = net("Office");
        e.enterprise = true;
        assert_eq!(texts(&e)[0], "Connect with sign-in…");
        assert!(!texts(&e).contains(&"Share network…"));
    }

    #[test]
    fn signal_icons_and_the_empty_list() {
        assert_eq!(strength_icon(0.9), "wifi-high");
        assert_eq!(strength_icon(0.5), "wifi-medium");
        assert_eq!(strength_icon(0.1), "wifi-low");
        let mut w = Wifi::default();
        assert_eq!(empty_text(&w), "No Wi-Fi adapter");
        w.adapter = true;
        assert_eq!(empty_text(&w), "Wi-Fi is off");
        w.enabled = true;
        assert_eq!(empty_text(&w), "Looking for networks…");
        w.networks.push(net("Cafe Guest"));
        assert_eq!(empty_text(&w), "");
    }

    #[test]
    fn groups_break_at_a_password_or_a_failure() {
        let mut nets = vec![net("A"), net("B"), net("C"), net("D")];
        assert_eq!(groups(&nets, None), [(true, false), (false, false), (false, false), (false, true)]);
        assert_eq!(groups(&nets, Some("B")), [(true, false), (false, true), (true, false), (false, true)]);
        nets[2].failed = "Wrong password".into();
        assert_eq!(groups(&nets, None), [(true, false), (false, false), (false, true), (true, true)]);
        assert!(groups(&[], None).is_empty());
    }

    #[test]
    fn wrapped_heights_and_the_wave() {
        assert_eq!(wrap_height("", 300.0, 12.0), 0.0);
        let one = wrap_height("short", 300.0, 12.0);
        assert!(wrap_height(&"word ".repeat(40), 300.0, 12.0) > 2.0 * one);
        let d = wave(300.0);
        assert!(d.starts_with("M 0.0 5.00"));
        assert_eq!(d.matches("L ").count(), 170);
        assert!(flat(0.05) && flat(0.97) && !flat(0.5));
    }

    #[test]
    fn keys_and_focus() {
        use slint::platform::Key;
        assert!(handles(&slint::SharedString::from(Key::Escape)));
        assert!(handles(" "));
        assert!(!handles("a"));
        assert_eq!(step(-1, 4, 1), 0);
        assert_eq!(step(-1, 4, -1), 3);
        assert_eq!(step(3, 4, 1), 0);
        assert_eq!(step(0, 4, -1), 3);
        assert_eq!(step(0, 0, 1), -1);
    }
}
