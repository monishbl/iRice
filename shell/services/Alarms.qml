pragma Singleton
import QtQuick
import Quickshell
import qs.common

// Countdown timers and alarms ("timer 10 min", "alarm 7am" in Spotlight).
// Kept in UiState so they survive a shell restart; one Timer waits for the
// next one due. When one goes off: the alarm sound, a notification and the
// island.
Singleton {
    id: root

    readonly property var list: (UiState.data.alarms ?? []).slice().sort((a, b) => a.at - b.at)
    readonly property var next: list.length > 0 ? list[0] : null

    function add(kind, at, label) {
        const e = { id: Date.now().toString(36) + Math.random().toString(36).slice(2, 5), at: at, label: label || "", kind: kind };
        UiState.data.alarms = (UiState.data.alarms ?? []).concat([e]);
        return e;
    }
    function addTimer(seconds, label) {
        return add("timer", Date.now() + seconds * 1000, label);
    }
    function addAlarm(date, label) {
        return add("alarm", date.getTime(), label);
    }
    function cancel(id) {
        UiState.data.alarms = (UiState.data.alarms ?? []).filter(a => a.id !== id);
    }

    // "10 min", "1h 30m", "90 seconds" -> seconds (0 if none)
    function parseDuration(text) {
        let total = 0, found = false;
        const re = /(\d+(?:\.\d+)?)\s*(h|hr|hrs|hours?|m|min|mins|minutes?|s|sec|secs|seconds?)\b/gi;
        let m;
        while ((m = re.exec(text)) !== null) {
            const n = parseFloat(m[1]), u = m[2].toLowerCase();
            total += u.startsWith("h") ? n * 3600 : u.startsWith("m") ? n * 60 : n;
            found = true;
        }
        if (!found) {
            // a bare number means minutes ("timer 5")
            const bare = text.match(/^\s*(\d+)\s*$/);
            if (bare)
                total = parseInt(bare[1]) * 60;
        }
        return Math.round(total);
    }
    // "7", "7am", "7:30 pm", "19:45" -> the next such time (null if none)
    function parseClock(text) {
        const m = text.trim().match(/^(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm)?$/i);
        if (!m)
            return null;
        let h = parseInt(m[1]);
        const min = m[2] ? parseInt(m[2]) : 0;
        const ap = (m[3] || "").toLowerCase();
        if (h > 23 || min > 59 || (ap && h > 12))
            return null;
        if (ap === "pm" && h < 12)
            h += 12;
        if (ap === "am" && h === 12)
            h = 0;
        const d = new Date();
        d.setHours(h, min, 0, 0);
        if (d.getTime() <= Date.now())
            d.setDate(d.getDate() + 1);
        return d;
    }
    function describe(seconds) {
        const h = Math.floor(seconds / 3600), m = Math.floor(seconds % 3600 / 60), s = seconds % 60;
        return [h ? h + " h" : "", m ? m + " min" : "", s && !h ? s + " s" : ""].filter(x => x).join(" ") || "0 s";
    }

    Timer {
        running: root.next !== null
        interval: root.next ? Math.max(200, root.next.at - Date.now()) : 1000
        onTriggered: {
            const now = Date.now();
            const due = root.list.filter(a => a.at <= now + 250);
            if (due.length === 0)
                return;
            UiState.data.alarms = (UiState.data.alarms ?? []).filter(a => a.at > now + 250);
            for (const a of due) {
                const title = a.kind === "timer" ? "Timer done" : "Alarm";
                const body = a.label || (a.kind === "timer" ? "Your timer has finished." : Qt.formatTime(new Date(a.at), "h:mm AP"));
                // (even with interface sounds off: it's an alarm)
                Quickshell.execDetached(["sh", "-c", "f=/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga; pw-play \"$f\" 2>/dev/null || paplay \"$f\""]);
                Quickshell.execDetached(["notify-send", "-a", "Clock", "-u", "critical", "-i", "alarm-clock", title, body]);
                Island.post({ kind: "custom", icon: "alarm", title: title, subtitle: body, color: Theme.accents.orange });
            }
        }
    }
}
