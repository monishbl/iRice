//! Weather.qml. wttr.in is fetched and parsed by core/src/services/weather.rs
//! (every 30 minutes, and refresh()); what the shell adds is saying it:
//! the state field `weather` ({ ready, icon, temp, condition, high, low,
//! place }, the menu bar's item and its tip) only while the bar shows it
//! (menubar.weather, on by default: a reading nobody sees changes no state, so
//! no part's turn and no bar redraw follows), and the day's sunrise and sunset
//! (minutes after midnight: Daylight.qml's, for the theme's Auto and the
//! wallpaper's time of day) to the shell's `background.sun`, as Weather.qml
//! set Daylight and Background.qml told the parts; once per change. (It was a
//! state field `daylight` nothing read, 18 Nov: Auto stayed on 6:30 and 18:30
//! and every part woke for it.) A change of location or units fetches again
//! at once; the bar's item turned on says the reading at once.

use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::services::weather::Weather;

#[derive(Default)]
pub struct W {
    /// location and units as last seen
    seen: (String, String),
    /// the sunrise and sunset last said to the shell
    sun: Option<(i32, i32)>,
}

/// The bar shows the weather (its item: menubar.weather, on by default)
pub fn shown(cfg: &Value) -> bool {
    cfg["menubar"]["weather"].as_bool().unwrap_or(true)
}

/// A reading's sunrise and sunset, when it has real ones
pub fn sun_of(w: &Weather) -> Option<(i32, i32)> {
    (w.ready && w.sunrise >= 0 && w.sunset > w.sunrise && w.sunset < 1440).then_some((w.sunrise, w.sunset))
}

/// The state field `weather`: what the bar's item and tip show
pub fn state_of(w: &Weather) -> Value {
    json!({"ready": w.ready, "icon": w.icon, "temp": w.temp, "condition": w.condition, "high": w.high, "low": w.low, "place": w.place})
}

fn settings(cfg: &Value) -> (String, String) {
    (cfg["weather"]["location"].as_str().unwrap_or("").to_string(), cfg["weather"]["units"].as_str().unwrap_or("").to_string())
}

impl W {
    pub fn start(&mut self, h: &mut Host) {
        h.st.shell.want("weather");
        self.seen = settings(h.cfg());
    }

    /// The settings changed: new place or units fetch again; the bar's
    /// item turned on gets the reading at once
    pub fn config_changed(&mut self, h: &mut Host) {
        let s = settings(h.cfg());
        if s != self.seen {
            self.seen = s;
            tessera_core::services::weather::refresh();
        }
        self.publish(h);
    }

    /// Say the reading (set_state tells the surfaces only when it changed)
    pub fn publish(&mut self, h: &mut Host) {
        let Some(w) = h.st.shell.get::<Weather>().cloned() else { return };
        if shown(h.cfg()) {
            h.set_state("weather", state_of(&w));
        }
        if let Some(sun) = sun_of(&w) {
            if self.sun != Some(sun) {
                self.sun = Some(sun);
                h.ask("background.sun", json!([sun.0, sun.1]));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_item_and_the_sun() {
        assert!(shown(&json!({})) && shown(&json!({"menubar": {"weather": true}})));
        assert!(!shown(&json!({"menubar": {"weather": false}})));
        let w = Weather { ready: true, icon: "sun", temp: 21, sunrise: 451, sunset: 1041, ..Default::default() };
        assert_eq!(sun_of(&w), Some((451, 1041)));
        // (no reading yet, or one without times: the defaults stand)
        assert_eq!(sun_of(&Weather { ready: false, ..w.clone() }), None);
        assert_eq!(sun_of(&Weather { sunrise: -1, ..w.clone() }), None);
        assert_eq!(sun_of(&Weather { sunrise: 1100, sunset: 400, ..w.clone() }), None);
        // (the bar's fields only: the hourly forecast, rain and feels-like change no state)
        let more = Weather { feels_like: 30, rain_chance: 80, ..w.clone() };
        assert_eq!(state_of(&w), state_of(&more));
        assert_eq!(state_of(&w)["temp"], json!(21));
    }
}
