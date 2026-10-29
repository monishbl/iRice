pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Today: the morning screen and its desktop widget. On the first unlock (or
// login) of the day it opens by itself: a greeting, today's events and
// to-dos, and the weather (modules/today/TodayScreen.qml). Days start at
// 4 AM, so a late night doesn't count as tomorrow. Settings → Desktop &
// Widgets → Today:
//   ask     every morning; closing clears it for the day, "Pin for today"
//           keeps it on the desktop as a widget until the day ends
//   widget  no morning screen; the Today widget is on the desktop every day
//   both    the morning screen, and the widget every day
//   off     neither
// Preview: qs ipc call today preview <bloom|shrink|drop|"">
Singleton {
    id: root

    readonly property var cfg: Config.options.today
    readonly property string mode: ["ask", "widget", "both", "off"].includes(cfg.mode) ? cfg.mode : "ask"
    readonly property bool screenOn: mode === "ask" || mode === "both"
    readonly property bool widgetAlways: mode === "widget" || mode === "both"

    // The morning screen: shown, and with which entrance
    property bool open: false
    property string effect: "bloom"
    function show(fx) {
        effect = ["bloom", "shrink", "drop"].includes(fx) ? fx : (["bloom", "shrink", "drop"].includes(cfg.entrance) ? cfg.entrance : "bloom");
        open = true;
        // The chime climbs to its peak 0.66 s in (tools/make-sounds.py,
        // TODAY_PEAK): start it so the peak lands as the panel settles
        // (TodayScreen's entrances: bloom 660, shrink 640, drop 980 ms,
        // times the motion scale; reduced motion: a 200 ms fade)
        const settle = Config.options.appearance.reduceMotion === true ? 0 : ({ bloom: 660, shrink: 640, drop: 980 })[effect] * CoolDown.motionScale;
        chimeDelay.interval = Math.max(0, Math.round(settle - 660));
        chimeDelay.restart();
    }
    Timer {
        id: chimeDelay
        onTriggered: if (root.open) Sounds.play("today")
    }
    function close() {
        open = false;
    }

    // ── The day ─────────────────────────────────────────────────────────
    // "YYYY-MM-DD" of the day it is, days starting at 4 AM
    function dayKey(d) {
        const t = new Date((d ?? new Date()).getTime() - 4 * 3600 * 1000);
        return Qt.formatDate(t, "yyyy-MM-dd");
    }
    property string day: dayKey()
    Timer { // the day turns over at 4 AM; a minute's precision is plenty
        interval: 60 * 1000
        running: true
        repeat: true
        onTriggered: root.day = root.dayKey()
    }
    // A new day doesn't open it there and then (at 4 AM you may still be
    // at your desk): it opens at the next login or unlock
    onDayChanged: tidyWidgets()

    // ── Who it greets ───────────────────────────────────────────────────
    property string accountName: ""
    Process {
        running: true
        command: ["sh", "-c", "getent passwd \"$(id -un)\" | cut -d: -f5 | cut -d, -f1; id -un"]
        stdout: StdioCollector {
            onStreamFinished: {
                const l = text.split("\n").map(s => s.trim());
                root.accountName = l[0] || l[1] || "";
            }
        }
    }
    // The first name, from Settings → About (Your name) or the account
    readonly property string name: ((cfg.name || accountName).trim().split(/\s+/)[0] ?? "")
    function greeting(d) {
        const h = (d ?? new Date()).getHours();
        const part = h >= 4 && h < 12 ? "Good morning" : h >= 12 && h < 17 ? "Good afternoon" : "Good evening";
        return name ? part + ", " + name : part;
    }

    // ── When it opens by itself ─────────────────────────────────────────
    // The first time in a day that the desktop is there to see: the session
    // has started (wallpaper up) and the screen isn't locked
    readonly property bool ready: Wallpaper.started && Config.ready && UiState.loaded && !System.locked
    onReadyChanged: if (ready) maybeShow()
    function maybeShow() {
        if (!ready || !screenOn || open || UiState.data.todayShown === day)
            return;
        delayed.interval = Date.now() - startedAt < 10000 ? 2800 : 900;
        delayed.restart();
    }
    // After the unlock animation has settled; at login, after the startup
    // chime too, so the two sounds don't overlap
    readonly property real startedAt: Date.now()
    Timer {
        id: delayed
        interval: 900
        // (the day only counts as shown once it actually opened: locking
        // again within the delay leaves it for the next unlock)
        onTriggered: if (root.ready && root.screenOn && UiState.data.todayShown !== root.day) {
            UiState.data.todayShown = root.day;
            root.show("");
        }
    }

    // ── The widget ──────────────────────────────────────────────────────
    // Pinned for today: on the desktop until the day ends (added in a free
    // spot; your own widgets aren't moved). Widget/Both: there every day.
    readonly property bool pinned: UiState.data.todayPinned === day
    function pin() {
        UiState.data.todayPinned = day;
        ensureWidget();
        close();
    }
    function hasWidget() {
        return Config.options.desktop.widgets.some(e => e.split(":")[0] === "today");
    }
    function ensureWidget() {
        if (hasWidget())
            return;
        Panels.screen = Quickshell.screens[0] ?? null;
        Panels.addWidget("today", "l");
    }
    // The day changed (or the setting): a widget pinned for an earlier day
    // goes, unless Today is meant to be there every day
    function tidyWidgets() {
        if (!Config.ready || !UiState.loaded)
            return;
        const was = UiState.data.todayPinned;
        if (was !== "" && was !== day) {
            UiState.data.todayPinned = "";
            if (!widgetAlways)
                Config.options.desktop.widgets = Config.options.desktop.widgets.filter(e => e.split(":")[0] !== "today");
        }
        // Once a day: removing it keeps it gone until tomorrow (shell
        // restarts and updates don't bring it back)
        if (widgetAlways && UiState.data.todayEnsured !== day) {
            UiState.data.todayEnsured = day;
            ensureWidget();
        }
    }
    onWidgetAlwaysChanged: if (widgetAlways) {
        UiState.data.todayEnsured = day;
        ensureWidget();
    }
    Connections {
        target: UiState
        function onLoadedChanged() {
            if (UiState.loaded)
                Qt.callLater(root.tidyWidgets);
        }
    }

    IpcHandler {
        target: "today"
        function preview(effect: string): void { // ("show" is taken by qs ipc itself)
            root.show(effect);
        }
        function close(): void {
            root.close();
        }
    }
}
