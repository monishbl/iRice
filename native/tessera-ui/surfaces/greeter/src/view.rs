//! What each style shows for the model's state, as the QML styles word it:
//! the line under the field, the field's placeholder, the session, the
//! clock in each style's form, and the shake's offsets. Pure functions.

use super::model::{Finger, Model};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// GreeterSurface.qml, the Glass look's
    Glass,
    /// material/modules/greeter/GreeterSurface.qml
    Material,
    /// GreeterPlain.qml
    Plain,
    /// GreeterText.qml, ly's
    Text,
}

impl Style {
    /// TESSERA_GREETER_STYLE (look | text | plain) and the look: `look` is
    /// Glass or Material, the others are the same in both
    pub fn pick(style: &str, material: bool) -> Style {
        match style {
            "text" => Style::Text,
            "plain" => Style::Plain,
            _ if material => Style::Material,
            _ => Style::Glass,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Style::Glass => "glass",
            Style::Material => "material",
            Style::Plain => "plain",
            Style::Text => "text",
        }
    }
}

/// A power action's name, as a style writes it
fn power_word(style: Style, action: &str) -> String {
    match (style, action) {
        (Style::Text, a) => a.to_string(),
        (_, "suspend") => "Sleep".into(),
        (_, "reboot") => "Restart".into(),
        (Style::Glass, "poweroff") => "Shut Down".into(),
        (_, "poweroff") => "Shut down".into(),
        (_, a) => a.to_string(),
    }
}

/// The line under the field (a power choice to confirm, an error, the
/// reader's words, the preview's hint), and whether it's bad news
pub fn say(style: Style, m: &Model) -> (String, bool) {
    let bad = !m.error.is_empty() || !m.power_choice.is_empty();
    if !m.power_choice.is_empty() {
        let w = power_word(style, &m.power_choice);
        let s = match style {
            Style::Glass => format!("{w}: click it again or press Return. Esc cancels."),
            Style::Plain => format!("{w}: click it again or press Enter. Esc cancels."),
            Style::Material => format!("{w}: press Enter or click it again"),
            Style::Text => format!("{w}: Enter (or the same key again) to confirm, Esc to cancel"),
        };
        return (s, bad);
    }
    if !m.error.is_empty() {
        return (m.error.clone(), bad);
    }
    let s = match style {
        Style::Text if m.busy => "logging in…".to_string(),
        Style::Text if !m.message.is_empty() => m.message.clone(),
        Style::Text if m.mock => "preview: the password is tessera, Esc quits".into(),
        Style::Text => "Enter to log in".into(),
        _ if !m.message.is_empty() => m.message.clone(),
        Style::Material if m.mock => "Preview: the password is tessera. Esc closes it.".into(),
        _ if m.mock => "Preview: the password is tessera. Esc quits.".into(),
        _ => String::new(),
    };
    (s, bad)
}

/// The password field's placeholder. (Glass's used to name a product's
/// fingerprint feature while a reader listens; it says it in words now,
/// here and in the QML)
pub fn placeholder(style: Style, m: &Model) -> String {
    match style {
        _ if m.busy && style != Style::Text => "Logging in…".into(),
        Style::Glass if m.finger != Finger::None => "Use Fingerprint or Enter Password".into(),
        Style::Glass => "Enter Password".into(),
        Style::Plain => "Password".into(),
        Style::Material => "Enter password".into(),
        Style::Text => String::new(),
    }
}

/// The session as each style shows it (▾ when there's a choice; the text
/// style's `< … >`)
pub fn session_label(style: Style, m: &Model) -> String {
    let name = m.session_name();
    match style {
        Style::Text => format!("< {} >", name.unwrap_or("Loading…")),
        Style::Material => name.unwrap_or("").to_string(),
        _ => format!("{}{}", name.unwrap_or(""), if m.sessions.len() > 1 { "  ▾" } else { "" }),
    }
}

/// The plain style's initial: the person's (or the typed name's) first letter
pub fn initial(m: &Model) -> String {
    let s = if m.other { m.other_name.clone() } else { m.person().map(|p| p.full.clone()).unwrap_or_default() };
    let s = if s.is_empty() { "?".to_string() } else { s };
    s.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default()
}

/// The time's pieces in the local zone
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Parts {
    pub weekday: String,
    pub wd: String,
    pub day: u32,
    pub month: String,
    pub mon: String,
    pub hour: u32,
    pub minute: u32,
}

pub fn parts_now() -> Parts {
    let secs = crate::util::now_ms() / 1000;
    let t = crate::util::strftime("%A|%a|%d|%B|%b|%H|%M", secs);
    let f: Vec<&str> = t.split('|').collect();
    let n = |i: usize| f.get(i).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let s = |i: usize| f.get(i).copied().unwrap_or("").to_string();
    Parts { weekday: s(0), wd: s(1), day: n(2), month: s(3), mon: s(4), hour: n(5), minute: n(6) }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Clock {
    /// "Wednesday 5 November" (dddd d MMMM)
    pub date: String,
    /// Glass's big time: "9:41" or "09:41"
    pub time: String,
    /// plain's line: "Wednesday 5 November   9:41"
    pub plain_top: String,
    /// text's: "Wed 5 Nov  9:41 AM"
    pub text_top: String,
    /// Material: the hours padded to two (dynamic, stacked, overlap), unpadded (wide)
    pub hours: String,
    pub hours_short: String,
    pub minutes: String,
    /// AM/PM, or with 24-hour time the day and month (dynamic's)
    pub half: String,
    pub hour_angle: f32,
    pub minute_angle: f32,
}

pub fn clock(p: &Parts, h24: bool) -> Clock {
    let h12 = (p.hour + 11) % 12 + 1;
    let hh = if h24 { format!("{:02}", p.hour) } else { h12.to_string() };
    let mm = format!("{:02}", p.minute);
    let ap = if p.hour < 12 { "AM" } else { "PM" };
    let date = format!("{} {} {}", p.weekday, p.day, p.month);
    Clock {
        time: format!("{hh}:{mm}"),
        plain_top: format!("{date}   {hh}:{mm}"),
        text_top: format!("{} {} {}  {}", p.wd, p.day, p.mon, if h24 { format!("{hh}:{mm}") } else { format!("{h12}:{mm} {ap}") }),
        hours: format!("{hh:0>2}"),
        hours_short: hh.clone(),
        minutes: mm,
        half: if h24 { format!("{} {}", p.day, p.mon) } else { ap.to_string() },
        hour_angle: ((p.hour % 12) as f32 + p.minute as f32 / 60.0) * 30.0,
        minute_angle: p.minute as f32 * 6.0,
        date,
    }
}

/// The Material clock's settings (lock.clockFace, clockWeight; the weather
/// face would go online for it: the dynamic one instead), and whether the
/// date shows under it (the dynamic face shows it itself with 24-hour time)
pub fn material_clock(cfg: &serde_json::Value, h24: bool) -> (String, i32, bool) {
    let raw = cfg["lock"]["clockFace"].as_str().unwrap_or("");
    let face = match raw {
        "stacked" | "wide" | "overlap" | "analog" => raw,
        _ => "dynamic",
    };
    let weight = cfg["lock"]["clockWeight"].as_f64().unwrap_or(640.0);
    // (in steps of 40, as LockClock.axes)
    let weight = ((weight / 40.0).round() * 40.0).clamp(100.0, 1000.0) as i32;
    (face.to_string(), weight, !(raw == "dynamic" && h24))
}

/// 24-hour time: greeter-start.sh's TESSERA_GREETER_CLOCK24, else the
/// config's appearance.clock24
pub fn h24(env: Option<&str>, cfg: &serde_json::Value) -> bool {
    match env {
        Some(v) if !v.is_empty() => v == "1",
        _ => cfg["appearance"]["clock24"].as_bool().unwrap_or(false),
    }
}

/// The shake's offsets, one per 16 ms frame: Glass's and plain's keyframes
/// (NumberAnimations: 50, 70, 70, 60 ms, linear), Material's SpringAnimation
/// from 18 to 0 (spring 24, damping 0.19, epsilon 0.2, stepped as Qt steps
/// it); none for the text style
pub fn shake_frames(style: Style) -> Vec<f32> {
    let keys: &[(f32, f32)] = match style {
        Style::Glass => &[(0.0, 0.0), (50.0, 12.0), (120.0, -10.0), (190.0, 5.0), (250.0, 0.0)],
        Style::Plain => &[(0.0, 0.0), (50.0, 10.0), (120.0, -8.0), (190.0, 4.0), (250.0, 0.0)],
        Style::Text => return Vec::new(),
        Style::Material => {
            let (mut x, mut v) = (18.0f32, 0.0f32);
            let mut out = vec![x];
            for _ in 0..200 {
                v += 24.0 * (0.0 - x) - 0.19 * v;
                x += v * 0.016;
                out.push(x);
                if x.abs() < 0.2 && v.abs() < 0.2 / 0.016 {
                    break;
                }
            }
            out.push(0.0);
            return out;
        }
    };
    let end = keys.last().unwrap().0;
    let mut out = Vec::new();
    let mut t = 0.0;
    while t <= end {
        let i = keys.iter().rposition(|k| k.0 <= t).unwrap_or(0).min(keys.len() - 2);
        let (a, b) = (keys[i], keys[i + 1]);
        out.push(a.1 + (b.1 - a.1) * ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0));
        t += 16.0;
    }
    out.push(0.0);
    out
}

/// `bullet` once per character of `text` (the password drawn)
pub fn mask(text: &str, bullet: &str) -> String {
    bullet.repeat(text.chars().count())
}

/// A key that types (GreeterText.qml: one character above a space): not a
/// control or one of Slint's private-use keys (arrows, F1…)
pub fn printable(text: &str) -> bool {
    let mut c = text.chars();
    match (c.next(), c.next()) {
        (Some(ch), None) => ch > ' ' && !ch.is_control() && !('\u{f700}'..='\u{f8ff}').contains(&ch),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{Session, User};
    use super::*;

    fn model(mock: bool) -> Model {
        Model::new(mock, vec![User { name: "ada".into(), full: "Ada Lovelace".into() }, User { name: "grace".into(), full: "Grace Hopper".into() }], vec![Session { name: "Tessera".into(), exec: "x".into() }, Session { name: "Hyprland".into(), exec: "y".into() }])
    }

    #[test]
    fn the_line_under_the_field_in_each_style() {
        let mut m = model(true);
        assert_eq!(say(Style::Glass, &m), ("Preview: the password is tessera. Esc quits.".into(), false));
        assert_eq!(say(Style::Material, &m).0, "Preview: the password is tessera. Esc closes it.");
        assert_eq!(say(Style::Text, &m).0, "preview: the password is tessera, Esc quits");
        m.mock = false;
        assert_eq!(say(Style::Glass, &m).0, "");
        assert_eq!(say(Style::Text, &m).0, "Enter to log in");
        m.message = "Place your finger on the fingerprint reader".into();
        assert_eq!(say(Style::Plain, &m), ("Place your finger on the fingerprint reader".into(), false));
        m.busy = true;
        assert_eq!(say(Style::Text, &m).0, "logging in…");
        m.error = "Incorrect password. Try again.".into();
        assert_eq!(say(Style::Glass, &m), ("Incorrect password. Try again.".into(), true));
        m.power_choice = "poweroff".into();
        assert_eq!(say(Style::Glass, &m).0, "Shut Down: click it again or press Return. Esc cancels.");
        assert_eq!(say(Style::Plain, &m).0, "Shut down: click it again or press Enter. Esc cancels.");
        assert_eq!(say(Style::Material, &m).0, "Shut down: press Enter or click it again");
        assert_eq!(say(Style::Text, &m), ("poweroff: Enter (or the same key again) to confirm, Esc to cancel".into(), true));
        m.power_choice = "suspend".into();
        assert!(say(Style::Glass, &m).0.starts_with("Sleep:"));
    }

    #[test]
    fn placeholders_and_sessions() {
        let mut m = model(false);
        assert_eq!(placeholder(Style::Glass, &m), "Enter Password");
        assert_eq!(placeholder(Style::Plain, &m), "Password");
        assert_eq!(placeholder(Style::Material, &m), "Enter password");
        m.finger = Finger::Waiting;
        assert_eq!(placeholder(Style::Glass, &m), "Use Fingerprint or Enter Password");
        m.busy = true;
        assert_eq!(placeholder(Style::Material, &m), "Logging in…");
        assert_eq!(session_label(Style::Glass, &m), "Tessera  ▾");
        assert_eq!(session_label(Style::Material, &m), "Tessera");
        assert_eq!(session_label(Style::Text, &m), "< Tessera >");
        m.sessions.truncate(1);
        assert_eq!(session_label(Style::Plain, &m), "Tessera");
        assert_eq!(initial(&m), "A");
        m.other = true;
        m.other_name = "émile".into();
        assert_eq!(initial(&m), "É");
        m.other_name.clear();
        assert_eq!(initial(&m), "?");
    }

    #[test]
    fn the_clock_as_each_style_writes_it() {
        let p = Parts { weekday: "Wednesday".into(), wd: "Wed".into(), day: 5, month: "November".into(), mon: "Nov".into(), hour: 21, minute: 7 };
        let c = clock(&p, false);
        assert_eq!(c.date, "Wednesday 5 November");
        assert_eq!(c.time, "9:07");
        assert_eq!(c.plain_top, "Wednesday 5 November   9:07");
        assert_eq!(c.text_top, "Wed 5 Nov  9:07 PM");
        assert_eq!((c.hours.as_str(), c.hours_short.as_str(), c.minutes.as_str(), c.half.as_str()), ("09", "9", "07", "PM"));
        assert_eq!((c.hour_angle, c.minute_angle), (9.0 * 30.0 + 3.5, 42.0));
        let c = clock(&p, true);
        assert_eq!((c.time.as_str(), c.text_top.as_str(), c.half.as_str()), ("21:07", "Wed 5 Nov  21:07", "5 Nov"));
        let midnight = Parts { hour: 0, minute: 0, ..p.clone() };
        assert_eq!(clock(&midnight, false).time, "12:00");
        assert_eq!(clock(&midnight, true).time, "00:00");
        assert_eq!(clock(&Parts { hour: 12, ..p }, false).half, "PM");
    }

    #[test]
    fn the_material_clocks_settings() {
        use serde_json::json;
        assert_eq!(material_clock(&json!({}), false), ("dynamic".into(), 640, true));
        assert_eq!(material_clock(&json!({"lock": {"clockFace": "weather", "clockWeight": 455}}), false), ("dynamic".into(), 440, true));
        assert_eq!(material_clock(&json!({"lock": {"clockFace": "dynamic"}}), true).2, false);
        // (no face set: the date shows, as the QML's raw check)
        assert!(material_clock(&json!({}), true).2);
        assert_eq!(material_clock(&json!({"lock": {"clockFace": "analog"}}), true).0, "analog");
        assert!(h24(Some("1"), &json!({})));
        assert!(!h24(Some("0"), &json!({"appearance": {"clock24": true}})));
        assert!(h24(None, &json!({"appearance": {"clock24": true}})));
    }

    #[test]
    fn the_shakes() {
        let g = shake_frames(Style::Glass);
        assert_eq!(g[0], 0.0);
        assert!(g.iter().cloned().fold(f32::MIN, f32::max) > 11.0 && g.iter().cloned().fold(f32::MAX, f32::min) < -7.0);
        assert_eq!(*g.last().unwrap(), 0.0);
        assert!(g.len() * 16 >= 250 && g.len() * 16 <= 300);
        let m = shake_frames(Style::Material);
        assert_eq!(m[0], 18.0);
        assert!(m.iter().any(|x| *x < -5.0), "it overshoots");
        assert!(m.len() < 120 && *m.last().unwrap() == 0.0, "and settles: {} frames", m.len());
        assert!(shake_frames(Style::Text).is_empty());
    }

    #[test]
    fn masks_and_typing_keys() {
        assert_eq!(mask("pässwörd", "•"), "••••••••");
        assert_eq!(mask("", "*"), "");
        assert!(printable("a") && printable("É") && printable(".") && printable("ß"));
        assert!(!printable("") && !printable("ab") && !printable("\n") && !printable("\u{8}") && !printable("\u{1b}") && !printable("\u{f704}") && !printable(" "));
    }

    #[test]
    fn styles_from_the_settings() {
        assert_eq!(Style::pick("look", false), Style::Glass);
        assert_eq!(Style::pick("look", true), Style::Material);
        assert_eq!(Style::pick("", true), Style::Material);
        assert_eq!(Style::pick("text", true), Style::Text);
        assert_eq!(Style::pick("plain", false), Style::Plain);
    }
}
