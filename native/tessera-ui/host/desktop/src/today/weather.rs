//! Weather.qml. wttr.in is fetched and parsed by core/src/services/weather.rs
//! (every 30 minutes, and refresh()); what the shell adds is saying it:
//! the state field `weather` ({ ready, icon, temp, condition, high, low,
//! place }, the menu bar's) and `daylight` ({ sunrise, sunset } in minutes
//! after midnight: Daylight.qml's, for the theme and the wallpaper). A change
//! of location or units fetches again at once.

use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::services::weather::Weather;

#[derive(Default)]
pub struct W {
    /// location and units as last seen
    seen: (String, String),
}

fn settings(cfg: &Value) -> (String, String) {
    (cfg["weather"]["location"].as_str().unwrap_or("").to_string(), cfg["weather"]["units"].as_str().unwrap_or("").to_string())
}

impl W {
    pub fn start(&mut self, h: &mut Host) {
        h.st.shell.want("weather");
        self.seen = settings(h.cfg());
    }

    /// The settings changed: new place or units fetch again
    pub fn config_changed(&mut self, h: &Host) {
        let s = settings(h.cfg());
        if s != self.seen {
            self.seen = s;
            tessera_core::services::weather::refresh();
        }
    }

    /// Say the reading (set_state tells the surfaces only when it changed)
    pub fn publish(&self, h: &mut Host) {
        let Some(w) = h.st.shell.get::<Weather>().cloned() else { return };
        h.set_state(
            "weather",
            json!({"ready": w.ready, "icon": w.icon, "temp": w.temp, "condition": w.condition, "high": w.high, "low": w.low, "place": w.place}),
        );
        if w.ready && w.sunrise >= 0 && w.sunset > w.sunrise {
            h.set_state("daylight", json!({"sunrise": w.sunrise, "sunset": w.sunset}));
        }
    }
}
