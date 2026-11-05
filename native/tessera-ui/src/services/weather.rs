//! The weather, as shell/services/Weather.qml reads it: wttr.in's JSON (no
//! API key, no account) for Settings' location (empty: wttr.in guesses from
//! the address) in Settings' units, every 30 minutes and when either setting
//! changes (`refresh()`). A reading is a `Weather` (Msg::Data); a failed one
//! keeps the last good numbers and says why (`error`: "Offline" when there's
//! no connection, "Weather unavailable" otherwise), as the QML does.
//!
//! TESSERA_WEATHER_URL points it somewhere else (a local file or a mock
//! server: anything curl reads), for the dev sandbox and tests.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hour {
    /// 0-23
    pub hour: u32,
    pub temp: i32,
    /// a Phosphor icon name (Weather.iconFor)
    pub icon: &'static str,
    /// chance of rain, %
    pub rain: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    pub ready: bool,
    pub error: String,
    pub place: String,
    pub temp: i32,
    pub feels_like: i32,
    pub high: i32,
    pub low: i32,
    pub condition: String,
    pub icon: &'static str,
    pub hourly: Vec<Hour>,
    /// today's highest chance of rain, %
    pub rain_chance: i32,
    /// minutes after midnight
    pub sunrise: i32,
    pub sunset: i32,
}

/// wttr.in / WWO weather codes -> Phosphor icons (Weather.iconFor)
pub fn icon_for(code: i32, night: bool) -> &'static str {
    match code {
        113 => if night { "moon-stars" } else { "sun" },
        116 => if night { "cloud-moon" } else { "cloud-sun" },
        119 | 122 => "cloud",
        143 | 248 | 260 => "cloud-fog",
        200 | 386 | 389 | 392 | 395 => "cloud-lightning",
        179 | 182 | 185 | 227 | 230 | 317 | 320 | 323 | 326 | 329 | 332 | 335 | 338 | 350 | 362 | 365 | 368 | 371 | 374 | 377 => "cloud-snow",
        _ => "cloud-rain",
    }
}

/// "06:42 AM" -> minutes after midnight
fn minutes(t: &str) -> Option<i32> {
    let t = t.trim();
    let (hm, ap) = t.rsplit_once(' ')?;
    let (h, m) = hm.split_once(':')?;
    let (h, m): (i32, i32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    let pm = ap.eq_ignore_ascii_case("PM");
    if !pm && !ap.eq_ignore_ascii_case("AM") {
        return None;
    }
    Some((h % 12 + if pm { 12 } else { 0 }) * 60 + m)
}

/// wttr.in's `?format=j1` at a local time (`hour`, `minute`), in metric or
/// imperial; `place`: the location asked for. None when it isn't that JSON.
/// `daylight`: the sunrise and sunset to use when the answer has none.
pub fn parse(text: &str, hour: u32, minute: u32, metric: bool, place: &str, daylight: (i32, i32)) -> Option<Weather> {
    let j: serde_json::Value = serde_json::from_str(text).ok()?;
    let cur = j["current_condition"].get(0)?;
    let today = j["weather"].get(0)?;
    let n = |v: &serde_json::Value| -> Option<i32> {
        match v {
            serde_json::Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f as i32),
            serde_json::Value::Number(x) => x.as_f64().map(|f| f as i32),
            _ => None,
        }
    };
    let pick = |o: &serde_json::Value, c: &str, f: &str| n(&o[if metric { c } else { f }]);
    let astro = &today["astronomy"][0];
    let (rise, set) = (astro["sunrise"].as_str().and_then(minutes), astro["sunset"].as_str().and_then(minutes));
    let (sunrise, sunset) = match (rise, set) {
        (Some(r), Some(s)) if s > r => (r, s),
        _ => daylight,
    };
    let now_min = (hour * 60 + minute) as i32;
    let night = now_min < sunrise || now_min >= sunset;
    let hours = today["hourly"].as_array()?;
    let at = |h: &serde_json::Value| n(&h["time"]).unwrap_or(0) / 100;
    let hourly = hours
        .iter()
        .filter(|h| at(h) >= hour as i32 - 1)
        .take(5)
        .map(|h| {
            let hh = at(h);
            Hour { hour: hh.clamp(0, 23) as u32, temp: pick(h, "tempC", "tempF").unwrap_or(0), icon: icon_for(n(&h["weatherCode"]).unwrap_or(0), !(6..19).contains(&hh)), rain: n(&h["chanceofrain"]).unwrap_or(0) }
        })
        .collect();
    let area = j["nearest_area"][0]["areaName"][0]["value"].as_str().unwrap_or("");
    Some(Weather {
        ready: true,
        error: String::new(),
        place: if place.is_empty() { area.to_string() } else { place.to_string() },
        temp: pick(cur, "temp_C", "temp_F")?,
        feels_like: pick(cur, "FeelsLikeC", "FeelsLikeF").unwrap_or(0),
        high: pick(today, "maxtempC", "maxtempF").unwrap_or(0),
        low: pick(today, "mintempC", "mintempF").unwrap_or(0),
        condition: cur["weatherDesc"][0]["value"].as_str().unwrap_or("").trim().to_string(),
        icon: icon_for(n(&cur["weatherCode"]).unwrap_or(0), night),
        hourly,
        rain_chance: hours.iter().map(|h| n(&h["chanceofrain"]).unwrap_or(0)).max().unwrap_or(0).max(0),
        sunrise,
        sunset,
    })
}

/// encodeURIComponent
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out += &format!("%{b:02X}");
        }
    }
    out
}

/// Settings' location and units (metric unless "imperial")
pub fn settings(cfg: &serde_json::Value) -> (String, bool) {
    (cfg["weather"]["location"].as_str().unwrap_or("").to_string(), cfg["weather"]["units"].as_str() != Some("imperial"))
}

static POKE: OnceLock<Mutex<mpsc::Sender<()>>> = OnceLock::new();

/// Read it again now (the location or units changed)
pub fn refresh() {
    if let Some(p) = POKE.get().and_then(|p| p.lock().ok()) {
        let _ = p.send(());
    }
}

/// curl: (body, HTTP status; 0 when nothing answered)
fn fetch(url: &str) -> (String, u32) {
    let out = std::process::Command::new("curl").args(["-sL", "--max-time", "20", "-w", "\n%{http_code}", url]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output();
    let Ok(out) = out else { return (String::new(), 0) };
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    // (a file: has no HTTP status, but read is read)
    let code = code.trim().parse().unwrap_or(0);
    let code = if code == 0 && out.status.success() && url.starts_with("file:") { 200 } else { code };
    (body.to_string(), code)
}

pub fn spawn(tx: Sender<Msg>) {
    let (ptx, prx) = mpsc::channel();
    let _ = POKE.set(Mutex::new(ptx));
    std::thread::spawn(move || {
        let mut last = Weather { icon: "cloud", sunrise: 390, sunset: 1110, ..Default::default() };
        loop {
            let (place, metric) = settings(&crate::theme::read_json());
            let url = crate::env::var("WEATHER_URL").filter(|u| !u.is_empty()).unwrap_or_else(|| format!("https://wttr.in/{}?format=j1", encode(&place)));
            let (body, code) = fetch(&url);
            let t = crate::services::calendar::local(crate::util::now_ms());
            if code != 200 {
                last.error = if code == 0 { "Offline".into() } else { "Weather unavailable".into() };
            } else {
                match parse(&body, t.h as u32, t.mi as u32, metric, &place, (last.sunrise, last.sunset)) {
                    Some(w) => last = w,
                    None => last.error = "Weather unavailable".into(),
                }
            }
            if tx.send(Msg::Data(Box::new(last.clone()))).is_err() {
                return;
            }
            // every 30 minutes, or at once when asked (settings changed)
            match prx.recv_timeout(Duration::from_secs(30 * 60)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const J1: &str = r#"{"current_condition":[{"temp_C":"11","temp_F":"52","FeelsLikeC":"9","FeelsLikeF":"48","weatherCode":"116","weatherDesc":[{"value":"Partly cloudy "}]}],
"nearest_area":[{"areaName":[{"value":"Lyon"}]}],
"weather":[{"maxtempC":"14","maxtempF":"57","mintempC":"6","mintempF":"43","astronomy":[{"sunrise":"07:31 AM","sunset":"05:21 PM"}],
"hourly":[{"time":"0","tempC":"7","tempF":"45","weatherCode":"113","chanceofrain":"0"},{"time":"300","tempC":"6","tempF":"43","weatherCode":"113","chanceofrain":"0"},
{"time":"600","tempC":"6","tempF":"43","weatherCode":"122","chanceofrain":"10"},{"time":"900","tempC":"9","tempF":"48","weatherCode":"116","chanceofrain":"20"},
{"time":"1200","tempC":"13","tempF":"55","weatherCode":"296","chanceofrain":"64"},{"time":"1500","tempC":"14","tempF":"57","weatherCode":"116","chanceofrain":"5"},
{"time":"1800","tempC":"11","tempF":"52","weatherCode":"113","chanceofrain":"0"},{"time":"2100","tempC":"8","tempF":"46","weatherCode":"113","chanceofrain":"0"}]}]}"#;

    #[test]
    fn a_reading_from_wttr() {
        let w = parse(J1, 10, 15, true, "", (390, 1110)).unwrap();
        assert_eq!((w.temp, w.high, w.low, w.rain_chance), (11, 14, 6, 64));
        assert_eq!((w.condition.as_str(), w.icon, w.place.as_str()), ("Partly cloudy", "cloud-sun", "Lyon"));
        assert_eq!((w.sunrise, w.sunset), (451, 1041));
        // from the hour before now, five of them; night icons after 19:00
        assert_eq!(w.hourly.iter().map(|h| h.hour).collect::<Vec<_>>(), vec![9, 12, 15, 18, 21]);
        assert_eq!(w.hourly[1].icon, "cloud-rain");
        assert_eq!(w.hourly[4].icon, "moon-stars");
        let f = parse(J1, 20, 0, false, "Paris", (390, 1110)).unwrap();
        assert_eq!((f.temp, f.icon, f.place.as_str()), (52, "cloud-moon", "Paris"));
        assert_eq!(f.hourly.len(), 1);
        assert!(parse("<html>", 10, 0, true, "", (390, 1110)).is_none());
    }

    #[test]
    fn locations_in_the_link() {
        assert_eq!(encode("São Paulo"), "S%C3%A3o%20Paulo");
        assert_eq!(encode(""), "");
        assert_eq!(settings(&serde_json::json!({"weather": {"units": "imperial", "location": "Oslo"}})), ("Oslo".into(), false));
        assert!(settings(&serde_json::Value::Null).1);
        assert_eq!(minutes("12:05 AM"), Some(5));
        assert_eq!(minutes("12:05 PM"), Some(725));
    }
}
