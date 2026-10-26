pragma Singleton
import QtQuick
import Quickshell
import qs.common

// Current weather from wttr.in (no API key). Refreshes every 30 minutes and
// when the location or units change.
Singleton {
    id: root

    property bool ready: false
    property string error: ""
    property string place: ""
    property int temp: 0
    property int feelsLike: 0
    property int high: 0
    property int low: 0
    property string condition: ""
    property string icon: "cloud"
    property var hourly: [] // [{ time, temp, icon }]
    // Minutes after midnight (mirrored in Daylight for the theme and wallpaper)
    property int sunrise: Daylight.sunrise
    property int sunset: Daylight.sunset

    readonly property bool metric: Config.options.weather.units !== "imperial"
    readonly property string unit: metric ? "°" : "°"

    // wttr.in / WWO weather codes → Phosphor icons
    function iconFor(code, night) {
        const c = +code;
        if (c === 113) return night ? "moon-stars" : "sun";
        if (c === 116) return night ? "cloud-moon" : "cloud-sun";
        if (c === 119 || c === 122) return "cloud";
        if ([143, 248, 260].includes(c)) return "cloud-fog";
        if ([200, 386, 389, 392, 395].includes(c)) return "cloud-lightning";
        if ([179, 182, 185, 227, 230, 317, 320, 323, 326, 329, 332, 335, 338, 350, 362, 365, 368, 371, 374, 377].includes(c)) return "cloud-snow";
        return "cloud-rain";
    }

    function refresh() {
        const x = new XMLHttpRequest();
        x.onreadystatechange = () => {
            if (x.readyState !== XMLHttpRequest.DONE)
                return;
            if (x.status !== 200) {
                error = x.status === 0 ? "Offline" : "Weather unavailable";
                return;
            }
            try {
                const j = JSON.parse(x.responseText);
                const cur = j.current_condition[0];
                const today = j.weather[0];
                const hour = new Date().getHours();
                const astro = today.astronomy?.[0];
                const mins = t => {
                    const m = (t ?? "").match(/(\d+):(\d+)\s*(AM|PM)/i);
                    return m ? (+m[1] % 12 + (m[3].toUpperCase() === "PM" ? 12 : 0)) * 60 + +m[2] : -1;
                };
                if (mins(astro?.sunrise) >= 0 && mins(astro?.sunset) > mins(astro?.sunrise)) {
                    Daylight.sunrise = mins(astro.sunrise);
                    Daylight.sunset = mins(astro.sunset);
                }
                const nowMin = hour * 60 + new Date().getMinutes();
                const night = nowMin < sunrise || nowMin >= sunset;
                temp = +(metric ? cur.temp_C : cur.temp_F);
                feelsLike = +(metric ? cur.FeelsLikeC : cur.FeelsLikeF);
                high = +(metric ? today.maxtempC : today.maxtempF);
                low = +(metric ? today.mintempC : today.mintempF);
                condition = cur.weatherDesc?.[0]?.value?.trim() ?? "";
                icon = iconFor(cur.weatherCode, night);
                const area = j.nearest_area?.[0];
                place = Config.options.weather.location || area?.areaName?.[0]?.value || "";
                hourly = today.hourly.filter(h => +h.time / 100 >= hour - 1).slice(0, 5).map(h => ({
                    time: Qt.formatTime(new Date(2000, 0, 1, +h.time / 100), "h AP"),
                    temp: +(metric ? h.tempC : h.tempF),
                    icon: iconFor(h.weatherCode, (+h.time / 100) < 6 || (+h.time / 100) >= 19)
                }));
                error = "";
                ready = true;
            } catch (e) {
                error = "Weather unavailable";
            }
        };
        x.open("GET", "https://wttr.in/" + encodeURIComponent(Config.options.weather.location) + "?format=j1");
        x.send();
    }

    Timer {
        interval: 30 * 60 * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }
    Connections {
        target: Config.options.weather
        function onLocationChanged() { root.refresh(); }
        function onUnitsChanged() { root.refresh(); }
    }
}
