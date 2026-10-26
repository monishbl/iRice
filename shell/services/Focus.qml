pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Notifications
import qs.common

// Focus modes: Do Not Disturb, Work, Personal, Sleep and any you add. A mode
// silences notification banners except from allowed apps (critical ones always
// show), and can switch the wallpaper and light/dark mode while it's on. Modes
// can run on a schedule or for a while ("for 1 hour", "until this evening").
// Modes live in ~/.config/irice/focus.json (plain JSON: Quickshell 0.2's
// JsonAdapter can't hold lists of objects safely).
Singleton {
    id: root

    readonly property var defaults: [
        { id: "dnd", name: "Do Not Disturb", icon: "moon", allowApps: [], wallpaper: "", appearance: "", schedule: { enabled: false, from: "22:00", to: "07:00", days: [0, 1, 2, 3, 4, 5, 6] } },
        { id: "work", name: "Work", icon: "briefcase", allowApps: [], wallpaper: "", appearance: "", schedule: { enabled: false, from: "09:00", to: "17:00", days: [1, 2, 3, 4, 5] } },
        { id: "personal", name: "Personal", icon: "user", allowApps: [], wallpaper: "", appearance: "", schedule: { enabled: false, from: "18:00", to: "22:00", days: [0, 1, 2, 3, 4, 5, 6] } },
        { id: "sleep", name: "Sleep", icon: "bed", allowApps: [], wallpaper: "", appearance: "dark", schedule: { enabled: false, from: "22:30", to: "07:00", days: [0, 1, 2, 3, 4, 5, 6] } }
    ]
    property var modes: defaults

    // Current state
    property string active: "" // mode id, "" when off
    property real until: 0 // ms timestamp; 0 = until turned off (or the schedule ends)
    property string source: "" // manual | schedule
    readonly property var mode: modes.find(m => m.id === active) ?? null
    readonly property bool on: mode !== null
    // A schedule turned off by hand stays off until its window ends
    property real skipScheduleUntil: 0
    // What a mode changed, to put back afterwards
    property var restore: ({})

    function modeById(id) {
        return modes.find(m => m.id === id) ?? null;
    }

    // Banners: allowed when off, critical, or from an allowed app
    function allows(n) {
        if (!on || n.urgency === NotificationUrgency.Critical)
            return true;
        const names = [n.appName, n.desktopEntry].map(x => (x ?? "").toLowerCase()).filter(x => x !== "");
        return (mode.allowApps ?? []).some(a => names.includes(a.toLowerCase()));
    }

    // durationMs: 0 = until turned off; -1 = until this evening (or tomorrow morning)
    function activate(id, durationMs, fromSchedule) {
        const m = modeById(id);
        if (!m)
            return;
        if (on && active !== id)
            undoEffects();
        active = id;
        source = fromSchedule ? "schedule" : "manual";
        until = durationMs > 0 ? Date.now() + durationMs : durationMs === -1 ? untilEvening() : 0;
        applyEffects(m);
        save();
    }

    function deactivate() {
        if (!on)
            return;
        if (source === "schedule" || scheduledNow(mode))
            skipScheduleUntil = scheduleEnd(mode);
        undoEffects();
        active = "";
        until = 0;
        source = "";
        save();
    }

    function toggle(id) {
        if (active === (id ?? "dnd"))
            deactivate();
        else
            activate(id ?? "dnd", 0, false);
    }

    function untilEvening() {
        const d = new Date();
        if (d.getHours() < 19)
            d.setHours(19, 0, 0, 0);
        else {
            d.setDate(d.getDate() + 1);
            d.setHours(8, 0, 0, 0);
        }
        return d.getTime();
    }

    // "On until 7:00 PM" style status
    readonly property string statusText: !on ? "Off" : until > 0 ? "On until " + Qt.formatTime(new Date(until), "h:mm AP") : source === "schedule" ? "On until " + fmt(mode.schedule.to) : "On"
    function fmt(hhmm) {
        const [h, m] = hhmm.split(":").map(Number);
        const d = new Date();
        d.setHours(h, m, 0, 0);
        return Qt.formatTime(d, "h:mm AP");
    }

    function applyEffects(m) {
        const r = {};
        if (m.wallpaper) {
            const w = Config.options.wallpaper;
            r.wallpaper = { path: w.path, dynamic: w.dynamic, live: w.live };
            Wallpaper.set(m.wallpaper);
        }
        if (m.appearance && m.appearance !== Config.options.appearance.mode) {
            r.appearance = Config.options.appearance.mode;
            Config.options.appearance.mode = m.appearance;
        }
        restore = r;
    }
    function undoEffects() {
        if (restore.wallpaper !== undefined) {
            const w = Config.options.wallpaper, r = restore.wallpaper;
            w.path = typeof r === "string" ? r : r.path;
            w.dynamic = r.dynamic ?? "off";
            w.live = r.live ?? "";
        }
        if (restore.appearance !== undefined)
            Config.options.appearance.mode = restore.appearance;
        restore = {};
    }

    // Schedules: minutes since midnight; windows may cross midnight
    function minutes(hhmm) {
        const [h, m] = (hhmm ?? "0:0").split(":").map(Number);
        return h * 60 + m;
    }
    function scheduledNow(m) {
        const s = m?.schedule;
        if (!s?.enabled)
            return false;
        const d = new Date();
        const now = d.getHours() * 60 + d.getMinutes(), from = minutes(s.from), to = minutes(s.to);
        if (from <= to)
            return s.days.includes(d.getDay()) && now >= from && now < to;
        // Overnight: started today after `from`, or yesterday and before `to`
        const yesterday = (d.getDay() + 6) % 7;
        return (s.days.includes(d.getDay()) && now >= from) || (s.days.includes(yesterday) && now < to);
    }
    function scheduleEnd(m) {
        const d = new Date();
        const to = minutes(m?.schedule?.to ?? "0:0");
        const end = new Date(d);
        end.setHours(Math.floor(to / 60), to % 60, 0, 0);
        if (end <= d)
            end.setDate(end.getDate() + 1);
        return end.getTime();
    }

    function tick() {
        const now = Date.now();
        if (on && until > 0 && now >= until) {
            deactivate();
            return;
        }
        if (on && source === "schedule" && !scheduledNow(mode)) {
            undoEffects();
            active = "";
            source = "";
            save();
            return;
        }
        if (!on && now >= skipScheduleUntil) {
            const m = modes.find(m => scheduledNow(m));
            if (m)
                activate(m.id, 0, true);
        }
    }
    Timer {
        running: true
        repeat: true
        interval: 30000
        triggeredOnStart: true
        onTriggered: root.tick()
    }

    // Edit modes from Settings
    function setMode(id, patch) {
        modes = modes.map(m => m.id === id ? Object.assign({}, m, patch) : m);
        save();
    }
    function addMode(name) {
        const id = name.toLowerCase().replace(/[^a-z0-9]+/g, "-") + "-" + Date.now().toString(36);
        modes = modes.concat([{ id: id, name: name, icon: "circle", allowApps: [], wallpaper: "", appearance: "", schedule: { enabled: false, from: "09:00", to: "17:00", days: [1, 2, 3, 4, 5] } }]);
        save();
        return id;
    }
    function removeMode(id) {
        if (active === id)
            deactivate();
        modes = modes.filter(m => m.id !== id);
        save();
    }

    // Persistence: modes and the current state
    readonly property string path: Config.dir + "/focus.json"
    property bool loaded: false
    function save() {
        if (loaded)
            saveTimer.restart();
    }
    Timer {
        id: saveTimer
        interval: 300
        onTriggered: file.setText(JSON.stringify({ modes: root.modes, active: root.active, until: root.until, source: root.source, restore: root.restore, skipScheduleUntil: root.skipScheduleUntil }, null, 2))
    }
    FileView {
        id: file
        path: root.path
        onLoaded: {
            try {
                const j = JSON.parse(text());
                if (Array.isArray(j.modes) && j.modes.length > 0)
                    root.modes = j.modes;
                root.active = j.active ?? "";
                root.until = j.until ?? 0;
                root.source = j.source ?? "";
                root.restore = j.restore ?? {};
                root.skipScheduleUntil = j.skipScheduleUntil ?? 0;
            } catch (e) {}
            root.loaded = true;
            root.tick();
        }
        onLoadFailed: {
            root.loaded = true;
            root.save();
        }
    }

    IpcHandler {
        target: "focus"
        function on(id: string): void { root.activate(id || "dnd", 0, false); }
        function off(): void { root.deactivate(); }
        function toggle(id: string): void { root.toggle(id || "dnd"); }
    }
}
