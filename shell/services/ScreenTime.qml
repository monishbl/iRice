pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common

// How long each app was in front today (and the six days before), for the
// Screen Time widget. Counts only while you're not idle (2 minutes without
// input stops the clock), adding time when the front app changes and once a
// minute. Kept in state.json.
Singleton {
    id: root

    function day(d) {
        return Qt.formatDate(d ?? new Date(), "yyyy-MM-dd");
    }
    readonly property var today: UiState.data.screenTime[day()] ?? {}
    readonly property int todayTotal: Object.values(today).reduce((a, b) => a + b, 0)
    // [{ id, seconds }] most used first
    readonly property var todayApps: Object.entries(today).map(([id, s]) => ({ id: id, seconds: s })).sort((a, b) => b.seconds - a.seconds)
    // The last 7 days' totals, oldest first: [{ label, seconds }]
    readonly property var week: {
        const out = [];
        for (let i = 6; i >= 0; i--) {
            const d = new Date(Date.now() - i * 86400000);
            const v = UiState.data.screenTime[day(d)] ?? {};
            out.push({ label: Qt.formatDate(d, "ddd").slice(0, 1), seconds: Object.values(v).reduce((a, b) => a + b, 0) });
        }
        return out;
    }

    function format(s) {
        const m = Math.round(s / 60);
        return m >= 60 ? Math.floor(m / 60) + "h " + (m % 60) + "m" : m + "m";
    }

    IdleMonitor {
        id: idle
        timeout: 120
        respectInhibitors: true // a playing video counts as use
    }
    // (iRice's own windows, like Settings, don't count)
    readonly property string app: Apps.activeEntry?.id === "org.quickshell" ? "" : (Apps.activeEntry?.id ?? "")
    property real since: Date.now()
    property string counting: ""

    function flush() {
        const now = Date.now();
        const secs = Math.round((now - since) / 1000);
        since = now;
        if (!counting || secs <= 0 || secs > 600)
            return;
        const all = Object.assign({}, UiState.data.screenTime);
        const d = day();
        all[d] = Object.assign({}, all[d] ?? {});
        all[d][counting] = (all[d][counting] ?? 0) + secs;
        // Keep a week
        const keep = new Set([...Array(7).keys()].map(i => day(new Date(Date.now() - i * 86400000))));
        for (const k of Object.keys(all))
            if (!keep.has(k))
                delete all[k];
        UiState.data.screenTime = all;
    }
    function restart() {
        flush();
        counting = idle.isIdle || !UiState.loaded ? "" : app;
    }
    onAppChanged: restart()
    Connections {
        target: idle
        function onIsIdleChanged() { root.restart(); }
    }
    Timer {
        interval: 60000
        repeat: true
        running: root.counting !== ""
        onTriggered: root.flush()
    }
    Component.onDestruction: flush()
}
