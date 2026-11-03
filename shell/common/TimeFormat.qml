pragma Singleton
import QtQuick
import Quickshell

// How times read everywhere (Settings → Appearance → 24-hour time): the
// patterns and helpers every clock, event and status line uses, so the one
// setting changes them all.
//   Qt.formatTime(d, TimeFormat.hm)   "9:41 AM" or "09:41"
//   TimeFormat.hour(d)                "9" or "09" (a big clock's hours)
Singleton {
    readonly property bool h24: Config.options.appearance.clock24
    readonly property string hm: h24 ? "HH:mm" : "h:mm AP"
    readonly property string hms: h24 ? "HH:mm:ss" : "h:mm:ss AP"
    // (large clocks: no AM/PM at that size)
    readonly property string hmShort: h24 ? "HH:mm" : "h:mm"
    readonly property string hOnly: h24 ? "HH" : "h AP"
    function hour(d) {
        return h24 ? String(d.getHours()).padStart(2, "0") : String((d.getHours() + 11) % 12 + 1);
    }
    function time(d) {
        return Qt.formatTime(d, hm);
    }
}
