pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.common

// What the dynamic island shows. `activities` are ongoing things (recording,
// a timer, music, a stopwatch); the first one appears in the compact pill.
// `event` is a short-lived card (a new song, the charger, a timer ending...)
// that pops the island open for a moment. Other parts of the shell can call
// post() to show their own.
Singleton {
    id: root

    readonly property var cfg: Config.options.island
    readonly property bool enabled: cfg.enabled

    // ── Ongoing activities, most important first ────────────────────────
    // Paused ones stay: music for a minute after pausing (its bars stop),
    // a timer or stopwatch until it's reset.
    readonly property var player: Media.player
    readonly property bool musicPlaying: !!player && player.isPlaying
    property real musicPausedAt: 0
    property bool musicLingers: false
    onMusicPlayingChanged: {
        if (musicPlaying) {
            musicLingers = false;
            lingerTimer.stop();
        } else if (player && player.trackTitle) {
            musicLingers = true;
            lingerTimer.restart();
        }
    }
    Timer {
        id: lingerTimer
        interval: 60000
        onTriggered: root.musicLingers = false
    }
    readonly property bool timerStarted: Timers.running || (Timers.remaining > 0 && Timers.remaining < Timers.phaseLength)
    readonly property bool swStarted: Timers.swRunning || Timers.swAccum > 0
    readonly property var activities: {
        const a = [];
        if (ScreenTools.recording)
            a.push({ kind: "recording", icon: "record", color: Theme.accents.red });
        if (timerStarted)
            a.push({ kind: "timer", icon: "timer", color: Theme.accents.orange });
        if (musicPlaying || (musicLingers && !!player))
            a.push({ kind: "music", icon: "music-notes", color: Theme.accent });
        if (swStarted)
            a.push({ kind: "stopwatch", icon: "timer", color: Theme.accents.green });
        return a;
    }
    // The one shown when collapsed (and when open, in one-at-a-time mode).
    // Scroll over the island to switch; a new activity takes the focus.
    property string focusKind: ""
    readonly property var primary: activities.find(x => x.kind === focusKind) ?? activities[0] ?? null
    readonly property int focusIndex: Math.max(0, activities.findIndex(x => x.kind === (primary?.kind ?? "")))
    property var lastKinds: []
    onActivitiesChanged: {
        const kinds = activities.map(x => x.kind);
        const added = kinds.find(k => !lastKinds.includes(k));
        if (added)
            focusKind = added;
        lastKinds = kinds;
    }
    function cycle(step) {
        if (activities.length < 2)
            return;
        focusKind = activities[(focusIndex + step + activities.length) % activities.length].kind;
    }

    // Is this piece of a collapsed live activity switched on? ("music", "bars")
    function shows(kind, piece) {
        return (cfg.livePieces ?? []).includes(kind + "." + piece);
    }

    // Kept open even with nothing happening (pin button, or right-click)
    readonly property bool pinned: cfg.pinned
    function togglePin() {
        Config.options.island.pinned = !Config.options.island.pinned;
    }

    // ── Events ──────────────────────────────────────────────────────────
    // { kind, icon, title, subtitle, color, art?, value?, battery?, action?,
    //   onAction?, card? }. Several can be live at once: they stack in the
    // open island, staggering in when they arrive together, and each leaves
    // when its time is up. `alerts` (a ListModel of uids, so existing rows
    // keep their delegates) is the stack; `event` is the newest.
    readonly property ListModel alerts: ListModel {}
    property var byUid: ({})
    property int nextUid: 1
    property int count: 0
    property int newestUid: 0
    readonly property var event: count > 0 ? (byUid[newestUid] ?? null) : null
    readonly property bool hasCard: { byUid; count; for (let i = 0; i < alerts.count; i++) if (byUid[alerts.get(i).uid]?.card) return true; return false; }
    readonly property int maxAlerts: 4
    // Alerts posted within a moment of each other stagger in as one burst
    property real burstAt: 0
    property int burstIndex: 0

    readonly property bool armed: started.elapsed // ignore initial values
    function post(e, ms) {
        if (!enabled || !armed)
            return;
        const now = Date.now();
        // HUD and music update their row in place
        if (e.kind === "hud" || e.kind === "music")
            for (let i = 0; i < alerts.count; i++) {
                const u = alerts.get(i).uid;
                if (byUid[u]?.kind === e.kind) {
                    const ev = Object.assign({ uid: u, delay: 0, expires: now + (ms ?? 3000) }, e);
                    byUid = Object.assign({}, byUid, { [u]: ev });
                    return;
                }
            }
        burstIndex = now - burstAt < 400 ? burstIndex + 1 : 0;
        if (burstIndex === 0)
            burstAt = now;
        const uid = nextUid++;
        // Later alerts in a burst stay up a little longer, so they're read
        const ev = Object.assign({ uid: uid, delay: burstIndex * 110, expires: now + (ms ?? 3000) + burstIndex * 600 }, e);
        byUid = Object.assign({}, byUid, { [uid]: ev });
        alerts.append({ uid: uid });
        while (alerts.count > maxAlerts)
            removeAt(0);
        newestUid = uid;
        count = alerts.count;
        sweep.restart();
    }
    // Change a live alert in place (e.g. the temperature as it moves)
    function update(uid, patch) {
        if (!byUid[uid])
            return;
        byUid = Object.assign({}, byUid, { [uid]: Object.assign({}, byUid[uid], patch) });
    }
    function remove(uid) {
        for (let i = 0; i < alerts.count; i++)
            if (alerts.get(i).uid === uid)
                return removeAt(i);
    }
    function removeAt(i) {
        const u = alerts.get(i).uid;
        alerts.remove(i);
        const b = Object.assign({}, byUid);
        delete b[u];
        byUid = b;
        count = alerts.count;
        if (count > 0)
            newestUid = alerts.get(alerts.count - 1).uid;
    }
    // Close everything (an alert's action ran, or the island was clicked)
    function dismiss() {
        alerts.clear();
        byUid = {};
        count = 0;
    }
    // Drop alerts whose time is up
    Timer {
        id: sweep
        interval: 250
        repeat: true
        running: root.count > 0
        onTriggered: {
            const now = Date.now();
            for (let i = root.alerts.count - 1; i >= 0; i--)
                if ((root.byUid[root.alerts.get(i).uid]?.expires ?? 0) <= now)
                    root.removeAt(i);
        }
    }
    Timer {
        id: started
        property bool elapsed: false
        running: true
        interval: 4000
        onTriggered: elapsed = true
    }

    // Kinds added after the setting existed count as on until the user has
    // touched them (cfg.known), so upgrades get the new alerts by default
    readonly property var newKinds: ["usb", "displays", "audio", "temperature"]
    function wants(kind) {
        return (cfg.events ?? []).includes(kind) || (newKinds.includes(kind) && !(cfg.known ?? []).includes(kind));
    }

    // Music: a new track
    property string lastTrack: ""
    readonly property string track: player ? (player.trackTitle ?? "") + "\n" + (player.trackArtist ?? "") : ""
    onTrackChanged: {
        if (track === lastTrack || !player || !player.trackTitle)
            return;
        lastTrack = track;
        if (wants("music") && player.isPlaying)
            post({ kind: "music", icon: "music-notes", title: player.trackTitle, subtitle: player.trackArtist ?? "", art: player.trackArtUrl ?? "", color: Theme.accent }, 3500);
    }

    // Charger and battery
    readonly property var battery: UPower.displayDevice
    readonly property bool hasBattery: battery?.isLaptopBattery ?? false
    readonly property real level: !battery ? 0 : battery.percentage > 1 ? battery.percentage / 100 : battery.percentage
    readonly property bool charging: hasBattery && (battery.state === UPowerDeviceState.Charging || battery.state === UPowerDeviceState.FullyCharged)
    onChargingChanged: {
        if (hasBattery && wants("charging"))
            post({ kind: "charging", icon: charging ? "lightning" : "plug", title: charging ? "Charging" : "Charger removed", subtitle: Math.round(level * 100) + "%" + (charging && level < 1 ? " · on power" : ""), value: level, color: charging ? Theme.accents.green : Qt.rgba(1, 1, 1, 0.6) });
    }
    property int lowStep: 100
    onLevelChanged: {
        if (!hasBattery || charging || !wants("battery"))
            return;
        const pct = Math.round(level * 100);
        for (const step of [20, 10, 5])
            if (pct <= step && lowStep > step) {
                lowStep = step;
                post({ kind: "battery", icon: "battery-warning", title: "Low battery", subtitle: pct + "% left · plug in soon", value: level, color: Theme.accents.red }, 4500);
                return;
            }
        if (pct > 25)
            lowStep = 100;
    }

    // Timers: a phase ended (not a reset)
    Connections {
        target: Timers
        function onPhaseEnded(next) {
            if (root.wants("timers"))
                root.post({ kind: "timer", icon: next === "focus" ? "timer" : "coffee", title: next === "focus" ? "Break's over" : "Focus session done", subtitle: next === "focus" ? "Time to focus" : "Take a break", color: Theme.accents.orange }, 4000);
        }
    }

    // Recording
    Connections {
        target: ScreenTools
        function onRecordingChanged() {
            if (root.wants("recording"))
                root.post({ kind: "recording", icon: "record", title: ScreenTools.recording ? "Recording" : "Recording saved", subtitle: ScreenTools.recording ? "Press Super+Shift+R to stop" : "In Videos", color: Theme.color.danger });
        }
    }

    // Focus
    Connections {
        target: Focus
        function onActiveChanged() {
            if (root.wants("focus"))
                root.post({ kind: "focus", icon: Focus.mode?.icon ?? "moon", title: Focus.on ? Focus.mode.name : "Focus off", subtitle: Focus.on ? Focus.statusText : "", color: Theme.accents.purple });
        }
    }

    // Bluetooth, USB, displays and audio output alerts live in Devices

    // Resources: only when they stay high (sampled every 5 s by Resources)
    Binding {
        target: Resources
        property: "islandActive"
        value: root.enabled && (root.wants("resources") || root.wants("temperature"))
    }
    // Temperature: hot after two readings at or above the threshold, cool
    // again 5 °C below it. One alert per hot spell (its reading kept live
    // while it shows); after that the closed island stays red until the CPU
    // cools. Unless switched off, the Low Power profile is used meanwhile;
    // the previous profile comes back after.
    readonly property int hotTemp: cfg.hotTemp ?? 90
    property int hotTicks: 0
    property bool hot: false
    property bool hotPreview: false // `qs ipc call island hot true`
    readonly property bool showHot: hot || hotPreview
    property int hotUid: 0
    // When temperature isn't watched any more (alert switched off, island
    // off), end the hot state so Low Power is given back
    readonly property bool tempWatch: enabled && wants("temperature")
    onTempWatchChanged: if (!tempWatch) hot = false
    property var savedProfile: null // the profile before throttling
    function hotText(t) {
        return "CPU " + Math.round(t) + " °C · " + (savedProfile !== null ? (System.sandboxed ? "would switch to Low Power (sandbox)" : "switched to Low Power to cool down") : PowerProfiles.profile === PowerProfile.PowerSaver ? "already on Low Power; close heavy apps" : "close heavy apps or let it cool");
    }
    function throttle(on) {
        if (on && cfg.hotThrottle && savedProfile === null && PowerProfiles.profile !== PowerProfile.PowerSaver) {
            savedProfile = PowerProfiles.profile;
            if (!System.sandboxed) // the sandbox shares the real machine's power daemon
                PowerProfiles.profile = PowerProfile.PowerSaver;
        } else if (!on && savedProfile !== null) {
            // Only if it's still the profile we set (you may have changed it)
            if (!System.sandboxed && PowerProfiles.profile === PowerProfile.PowerSaver)
                PowerProfiles.profile = savedProfile;
            savedProfile = null;
        }
    }
    onHotChanged: {
        throttle(hot);
        if (!hot)
            hotUid = 0;
    }
    Connections {
        target: Resources
        function onTemperatureChanged() {
            const t = Resources.temperature;
            if (!root.wants("temperature")) {
                root.hot = false;
                return;
            }
            if (t <= 0)
                return; // a failed reading: ignore it rather than "cooling down"
            root.hotTicks = t >= root.hotTemp ? root.hotTicks + 1 : 0;
            if (!root.hot && root.hotTicks >= 2) {
                root.hot = true;
                root.post({ kind: "temperature", icon: "thermometer-hot", title: "Running hot", subtitle: root.hotText(t), value: Math.min(1, t / 105), color: Theme.accents.red }, 6000);
                root.hotUid = root.newestUid;
            } else if (root.hot && t < root.hotTemp - 5) {
                root.hot = false;
            } else if (root.hot && root.hotUid) {
                root.update(root.hotUid, { subtitle: root.hotText(t), value: Math.min(1, t / 105) });
            }
        }
    }

    property int hotCpu: 0
    property int hotMem: 0
    Connections {
        target: Resources
        function onCpuChanged() {
            root.hotCpu = Resources.cpu > 0.9 ? root.hotCpu + 1 : 0;
            if (root.hotCpu === 4)
                root.post({ kind: "resources", icon: "cpu", title: "CPU is busy", subtitle: Math.round(Resources.cpu * 100) + "% for a while", value: Resources.cpu, color: Theme.color.warning }, 4500);
        }
        function onMemoryChanged() {
            root.hotMem = Resources.memory > 0.9 ? root.hotMem + 1 : 0;
            if (root.hotMem === 3)
                root.post({ kind: "resources", icon: "memory", title: "Memory is nearly full", subtitle: Resources.memUsedGb.toFixed(1) + " of " + Resources.memTotalGb.toFixed(0) + " GB", value: Resources.memory, color: Theme.color.warning }, 4500);
        }
    }

    // Volume, brightness and mic, when the HUD lives in the island
    function hud(kind) {
        const v = kind === "brightness" ? Brightness.value : kind === "mic" ? (Audio.micMuted ? 0 : 1) : (Audio.muted ? 0 : Audio.volume);
        post({ kind: "hud", hud: kind, icon: kind === "brightness" ? "sun" : kind === "mic" ? (Audio.micMuted ? "microphone-slash" : "microphone") : Audio.icon, title: kind === "brightness" ? "Display" : kind === "mic" ? (Audio.micMuted ? "Microphone off" : "Microphone on") : "Sound", value: v, color: Theme.color.label }, 1600);
    }

    // qs ipc call island post <icon> <title> <subtitle>  (for scripts)
    IpcHandler {
        target: "island"
        function post(icon: string, title: string, subtitle: string): void {
            root.post({ kind: "custom", icon: icon || "bell", title: title, subtitle: subtitle, color: Theme.accent });
        }
        // qs ipc call island next | previous | pin  (switch live activity, keep open)
        function next(): void { root.cycle(1); }
        function previous(): void { root.cycle(-1); }
        function pin(): void { root.togglePin(); }
        // qs ipc call island hot true|false  (preview the hot look)
        function hot(on: bool): void { root.hotPreview = on; }
        // qs ipc call island demo <device|usb|display|audio|charging|battery|notification|music>
        function demo(kind: string): void {
            const was = started.elapsed;
            started.elapsed = true;
            const d = {
                device: { kind: "device", icon: "headphones", title: "AirPods Pro", subtitle: "Connected", color: Theme.accents.blue, battery: 0.85 },
                usb: { kind: "usb", icon: "hard-drive", title: "SanDisk Ultra", subtitle: "Drive connected", color: Theme.accents.orange, action: "Open" },
                display: { kind: "display", icon: "monitor", title: "Dell U2723QE", subtitle: "Display connected · DP-1", color: Theme.accents.green },
                audio: { kind: "audio", icon: "headphones", title: "Headphones", subtitle: "Built-in Audio", color: Theme.accents.purple },
                charging: { kind: "charging", icon: "lightning", title: "Charging", subtitle: "64% · on power", value: 0.64, color: Theme.accents.green },
                battery: { kind: "battery", icon: "battery-warning", title: "Low battery", subtitle: "10% left · plug in soon", value: 0.1, color: Theme.accents.red },
                temperature: { kind: "temperature", icon: "thermometer-hot", title: "Running hot", subtitle: "CPU 94 °C · close heavy apps or let it cool", value: 0.9, color: Theme.accents.red },
                notification: { kind: "notification", headline: "Here, I found the door code in your messages from Mac.", card: { name: "Mac Tyler", time: "Thursday", text: "There's a spare key on the hook by the coat rack! The front door code is #1997", initials: "MT", appIcon: "org.gnome.Polari" } },
                music: { kind: "music", icon: "music-notes", title: "Midnight City", subtitle: "M83", color: Theme.accent, art: "" }
            }[kind] ?? { kind: "custom", icon: "bell", title: "Hello", subtitle: "A custom alert", color: Theme.accent };
            if (kind === "burst") { // several at once, as when headphones connect while charging
                for (const k of ["device", "charging", "audio"])
                    demo(k);
                return;
            }
            root.post(d, 5000);
            started.elapsed = was;
        }
    }
}
