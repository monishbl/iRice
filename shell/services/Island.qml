pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.common
import "IslandCycle.js" as Cycle

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
            a.push({ kind: "recording", icon: Recorder.paused ? "pause-circle" : "record", color: Recorder.paused ? Qt.rgba(1, 1, 1, 0.55) : Theme.accents.red }); // grey while paused
        if (timerStarted)
            a.push({ kind: "timer", icon: "timer", color: Theme.accents.orange });
        if (musicPlaying || (musicLingers && !!player))
            a.push({ kind: "music", icon: "music-notes", color: Theme.accent });
        if (swStarted)
            a.push({ kind: "stopwatch", icon: "timer", color: Theme.accents.green });
        if (Calls.active)
            a.push({ kind: "call", icon: Privacy.camera ? "video-camera" : "phone", color: Theme.accents.green, appIcon: Calls.call?.appIcon ?? "" });
        if (Privacy.shareInIsland)
            a.push({ kind: "screenShare", icon: "screencast", color: "#BF5AF2" });
        if (Updates.running && wants("sysupdate"))
            a.push({ kind: "sysupdate", icon: "arrow-circle-up", color: Theme.accents.blue });
        if (Transfers.active)
            a.push({ kind: "transfer", icon: Transfers.first?.kind === "copy" ? "copy" : "download-simple", color: Theme.accents.blue });
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
    // A wheel or touchpad scroll over the island (angleDelta): a notch, or
    // a whole swipe, moves one activity (IslandCycle.js). Returns the step
    // taken (-1, 0 or 1) for the switch animation's direction.
    property var scrollState: null
    function scroll(dx, dy) {
        if (activities.length < 2)
            return 0;
        const r = Cycle.wheel(scrollState, dx, dy, Date.now());
        scrollState = r.state;
        if (r.step !== 0)
            cycle(r.step);
        return r.step;
    }

    // Is this piece of a collapsed live activity switched on? ("music", "bars")
    // Kinds added later start with their defaults until changed in Settings.
    readonly property var liveDefaults: ({ transfer: ["icon", "name", "time", "open", "cancel"], call: ["icon", "name", "time", "mute"], screenShare: ["icon", "name", "time", "stop"], sysupdate: ["icon", "name", "time"] })
    function customised(kind) {
        return !liveDefaults[kind] || (cfg.known ?? []).includes("live." + kind) || (cfg.livePieces ?? []).some(x => x.startsWith(kind + "."));
    }
    function shows(kind, piece) {
        return customised(kind) ? (cfg.livePieces ?? []).includes(kind + "." + piece) : liveDefaults[kind].includes(piece);
    }
    function setShows(kind, piece, on) {
        const pieces = liveDefaults[kind] ?? [];
        let list = (cfg.livePieces ?? []).slice();
        // First change to a newer kind: write its defaults out, then change
        if (!customised(kind))
            list = list.concat(pieces.map(x => kind + "." + x));
        list = list.filter(x => x !== kind + "." + piece).concat(on ? [kind + "." + piece] : []);
        Config.options.island.livePieces = list;
        if (liveDefaults[kind] && !(cfg.known ?? []).includes("live." + kind))
            Config.options.island.known = (cfg.known ?? []).concat(["live." + kind]);
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
    // (a card, or a ringing call: the island opens wider)
    readonly property bool hasCard: { byUid; count; for (let i = 0; i < alerts.count; i++) { const e = byUid[alerts.get(i).uid]; if (e?.card || e?.kind === "ringing") return true; } return false; }
    readonly property int maxAlerts: 4
    // Alerts posted within a moment of each other stagger in as one burst
    property real burstAt: 0
    property int burstIndex: 0

    readonly property bool armed: started.elapsed // ignore initial values
    // What the island said lately (kind, title, subtitle, when), so a desktop
    // notification about the same thing can stay quiet (Notifications.qml)
    property var recent: []
    function note(e, uid) {
        const now = Date.now();
        const r = { kind: e.kind ?? "", title: e.title ?? "", subtitle: e.subtitle ?? "", at: now, uid: uid };
        recent = recent.filter(x => now - x.at < 20000).concat([r]);
        return r;
    }
    function post(e, ms) {
        if (!enabled || !armed)
            return;
        const now = Date.now();
        // HUD and music update their row in place
        if (e.kind === "hud" || e.kind === "music")
            for (let i = 0; i < alerts.count; i++) {
                const u = alerts.get(i).uid;
                if (byUid[u]?.kind === e.kind) {
                    const ev = Object.assign({ uid: u, delay: 0, at: now, expires: now + (ms ?? 3000) }, e);
                    byUid = Object.assign({}, byUid, { [u]: ev });
                    note(e, u);
                    return u;
                }
            }
        // The same thing again while its alert is up (headphones whose battery
        // arrives a moment after "Connected", a device that reconnects): that
        // alert changes in place and stays a little longer, no second one
        if (e.title)
            for (let i = 0; i < alerts.count; i++) {
                const u = alerts.get(i).uid;
                const old = byUid[u];
                if (old && old.kind === e.kind && old.title === e.title && old.expires > now) {
                    const ev = Object.assign({ uid: u, delay: 0, at: now, expires: Math.max(old.expires, now + (ms ?? 3000)), hid: record(e) }, e);
                    byUid = Object.assign({}, byUid, { [u]: ev });
                    note(e, u);
                    return u;
                }
            }
        burstIndex = now - burstAt < 400 ? burstIndex + 1 : 0;
        if (burstIndex === 0)
            burstAt = now;
        const uid = nextUid++;
        // Later alerts in a burst stay up a little longer, so they're read
        const ev = Object.assign({ uid: uid, delay: burstIndex * 110, at: now, expires: now + (ms ?? 3000) + burstIndex * 600, hid: record(e) }, e);
        byUid = Object.assign({}, byUid, { [uid]: ev });
        alerts.append({ uid: uid });
        // (the oldest go first; a ringing call (sticky) stays)
        while (alerts.count > maxAlerts) {
            let i = 0;
            while (i < alerts.count - 1 && byUid[alerts.get(i).uid]?.sticky)
                i++;
            removeAt(i);
        }
        newestUid = uid;
        count = alerts.count;
        sweep.restart();
        // (a notification about the same thing may have arrived just before)
        const said = note(e, uid);
        if (said.kind !== "notification" && said.kind !== "ringing")
            Qt.callLater(() => Notifications.islandSaid(said));
        return uid;
    }
    // Change a live alert in place (e.g. the temperature as it moves); its
    // line in the history follows
    function update(uid, patch) {
        if (!byUid[uid])
            return;
        byUid = Object.assign({}, byUid, { [uid]: Object.assign({}, byUid[uid], patch) });
        const hid = byUid[uid].hid;
        if (hid) {
            const shown = {};
            for (const k of ["title", "subtitle", "icon", "battery", "value"])
                if (patch[k] !== undefined)
                    shown[k] = patch[k];
            if (Object.keys(shown).length > 0)
                setHistory(history.map(h => h.hid === hid ? Object.assign({}, h, shown) : h));
        }
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
    // Close everything (an alert's action ran, or the island was clicked),
    // except a ringing call (sticky: only Answer or Decline end it)
    function dismiss() {
        for (let i = alerts.count - 1; i >= 0; i--)
            if (!byUid[alerts.get(i).uid]?.sticky)
                removeAt(i);
    }
    // ── History ─────────────────────────────────────────────────────────
    // Every alert that isn't a notification (those are in Notification Center
    // already), newest first, for Notification Center's iRice group and the
    // open island's Recent list. The same thing again within a minute (a
    // device that reconnects) updates its line. Kept across shell reloads
    // like the notifications are; buttons only while the shell runs.
    readonly property var unrecorded: ["notification", "hud", "music", "ringing"] // (an unanswered call is recorded as "missedCall" when it ends: Calls)
    readonly property int historyMax: 50
    property var history: [] // { hid, kind, icon, title, subtitle, color, battery?, at, times }
    property int nextHid: 1
    property var historyActs: ({}) // hid -> { text, act }
    function setHistory(list) {
        history = list;
        kept.items = JSON.stringify(list);
        kept.nextHid = nextHid;
    }
    function record(e) {
        if (unrecorded.includes(e.kind) || !e.title)
            return 0;
        const now = Date.now();
        // (missed calls from different people stay apart)
        const old = history.find(h => h.kind === e.kind && h.title === e.title && now - h.at < 60000 && (e.kind !== "missedCall" || h.subtitle === (e.subtitle ?? "")));
        const hid = old ? old.hid : nextHid++;
        const entry = { hid: hid, kind: e.kind, icon: e.icon ?? "bell", title: e.title, subtitle: e.subtitle ?? "", color: String(e.color ?? "#FFFFFF"), at: now, times: old ? old.times + 1 : 1 };
        if (e.battery !== undefined)
            entry.battery = e.battery;
        setHistory([entry].concat(history.filter(h => h.hid !== hid)).slice(0, historyMax));
        // Its button (Open a download or a drive, What's new), while it means something
        const a = Object.assign({}, historyActs);
        if (e.action && e.onAction && ["transfer", "usb", "sysupdate", "missedCall"].includes(e.kind))
            a[hid] = { text: e.action, act: e.onAction };
        else
            delete a[hid];
        for (const k in a)
            if (!history.some(h => String(h.hid) === k))
                delete a[k];
        historyActs = a;
        return hid;
    }
    function forget(hid) {
        setHistory(history.filter(h => h.hid !== hid));
    }
    function clearHistory() {
        setHistory([]);
        historyActs = {};
    }
    // "now", "5m", "2h", then the time of day
    function ago(at, now) {
        const mins = Math.floor((now - at) / 60000);
        return mins < 1 ? "now" : mins < 60 ? mins + "m" : mins < 360 ? Math.floor(mins / 60) + "h" : Qt.formatTime(new Date(at), "h:mm AP");
    }
    PersistentProperties {
        id: kept
        reloadableId: "islandHistory"
        property string items: "[]"
        property int nextHid: 1
        onReloaded: {
            try {
                const list = JSON.parse(items);
                if (Array.isArray(list))
                    root.history = list;
            } catch (err) {}
            root.nextHid = Math.max(nextHid, 1 + root.history.reduce((m, h) => Math.max(m, h.hid ?? 0), 0));
        }
    }

    // ── Hover hold ──────────────────────────────────────────────────────
    // While the pointer rests on the alerts (any screen's island), nothing
    // times out; when it leaves, each alert gets back the time it had left,
    // plus a second
    property var holders: ({}) // screen name -> true
    readonly property bool held: Object.keys(holders).length > 0
    property real heldAt: 0
    function hold(key, on) {
        if (!!holders[key] === on)
            return;
        const was = held;
        const h = Object.assign({}, holders);
        if (on)
            h[key] = true;
        else
            delete h[key];
        holders = h;
        const now = Date.now();
        if (!was && held) {
            heldAt = now;
        } else if (was && !held) {
            const b = Object.assign({}, byUid);
            for (const u in b)
                b[u] = Object.assign({}, b[u], { expires: b[u].expires + (now - Math.max(heldAt, b[u].at ?? heldAt)) + 1000 });
            byUid = b;
        }
    }
    // Long alerts: a finished download or a drive stays up for 10 minutes
    // (low battery and running hot until their condition ends, below)
    readonly property int longMs: 600000
    readonly property int untilOver: 86400000

    // Drop alerts whose time is up
    Timer {
        id: sweep
        interval: 250
        repeat: true
        running: root.count > 0 && !root.held
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
    readonly property var newKinds: ["usb", "displays", "audio", "temperature", "network", "transfers", "calls", "sysupdate"]
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
        if (charging && lowUid) {
            remove(lowUid);
            lowUid = 0;
        }
        if (hasBattery && wants("charging"))
            post({ kind: "charging", icon: charging ? "lightning" : "plug", title: charging ? "Charging" : "Charger removed", subtitle: Math.round(level * 100) + "%" + (charging && level < 1 ? " · on power" : ""), value: level, color: charging ? Theme.accents.green : Qt.rgba(1, 1, 1, 0.6) });
    }
    property int lowStep: 100
    property int lowUid: 0 // the low battery alert, up until the charger goes in
    onLevelChanged: {
        if (!hasBattery || charging || !wants("battery"))
            return;
        const pct = Math.round(level * 100);
        for (const step of [20, 10, 5])
            if (pct <= step && lowStep > step) {
                lowStep = step;
                // (a lower step updates the one showing)
                post({ kind: "battery", icon: "battery-warning", title: "Low battery", subtitle: pct + "% left · plug in soon", value: level, color: Theme.accents.red }, untilOver);
                lowUid = 0;
                for (let i = 0; i < alerts.count; i++)
                    if (byUid[alerts.get(i).uid]?.kind === "battery")
                        lowUid = alerts.get(i).uid;
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

    // Recording: started, then saved (Open, Show in Folder), discarded
    // (Undo, for as long as the file is kept) or failed
    Connections {
        target: Recorder
        function onRecordingChanged() {
            if (Recorder.recording && root.wants("recording"))
                root.post({ kind: "recording", icon: "record", title: "Recording", subtitle: "Stop it here or with Super+Shift+R", color: Theme.color.danger });
        }
        function onSaved(file) {
            if (root.wants("recording"))
                root.post({ kind: "recording", icon: "record", title: "Recording saved", subtitle: file.replace(/^.*\//, ""), color: Theme.color.danger, more: [{ text: "Open", act: () => Recorder.open(file) }, { text: "Show in Folder", act: () => Recorder.showInFolder(file) }] }, 6000);
        }
        function onDropped(file) {
            if (root.wants("recording"))
                root.post({ kind: "recording", icon: "trash", title: "Recording discarded", subtitle: file.replace(/^.*\//, ""), color: Theme.color.danger, more: [{ text: "Undo", act: () => Recorder.undoDiscard() }] }, 8000);
        }
        function onFailed(why) {
            if (root.wants("recording"))
                root.post({ kind: "recording", icon: "warning-circle", title: "Recording didn't work", subtitle: why, color: Theme.color.danger }, 6000);
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
    // Temperature: hot once the CPU has averaged at or above the threshold
    // for 30 s (a turbo burst of a few seconds isn't "running hot"); cool
    // again once it has averaged 5 °C below it for a minute. One alert per
    // hot spell (its reading kept live while it shows); after that the closed
    // island stays red until the CPU cools. Unless switched off, the Low
    // Power profile is used meanwhile; the previous profile comes back after.
    // With cooling.hotAction "ask", the alert asks before cooling down and
    // stays until answered (or the spell ends).
    readonly property int hotTemp: cfg.hotTemp ?? 90
    readonly property int hotAfter: 30000 // ms averaging at/above hotTemp
    readonly property int coolAfter: 60000 // ms averaging 5 °C below it
    property var tempSamples: [] // [{ t: °C, at: ms }], the last coolAfter
    // The average over the last `ms`, or -1 when the readings don't cover
    // that long yet (just started, or a gap)
    function averageOver(ms, now) {
        const from = now - ms;
        const inside = tempSamples.filter(x => x.at >= from);
        if (!inside.length || tempSamples[0].at > from + 5000)
            return -1;
        return inside.reduce((a, x) => a + x.t, 0) / inside.length;
    }
    readonly property bool asking: Config.options.cooling.whenHot && Config.options.cooling.hotAction === "ask"
    property bool hot: false
    property bool hotPreview: false // `qs ipc call island hot true`
    readonly property bool showHot: hot || hotPreview
    property int hotUid: 0
    // When temperature isn't watched any more (alert switched off, island
    // off), end the hot state so Low Power is given back
    readonly property bool tempWatch: enabled && wants("temperature")
    onTempWatchChanged: if (!tempWatch) {
        hot = false;
        tempSamples = [];
    }
    // Low Power while hot goes through Power (it decides the energy mode for
    // everyone; Cool-down wins over the rest while it lasts)
    readonly property bool throttled: Power.has("cooldown")
    function hotText(t) {
        return "CPU " + Math.round(t) + " °C · " + (throttled ? (System.sandboxed ? "would switch to Low Power (sandbox)" : "switched to Low Power to cool down") : PowerProfiles.profile === PowerProfile.PowerSaver ? "already on Low Power; close heavy apps" : "close heavy apps or let it cool");
    }
    function throttle(on) {
        if (on && cfg.hotThrottle)
            Power.ask("cooldown", "power-saver", "cooling down: CPU at " + Math.round(Resources.temperature) + " °C");
        else
            Power.release("cooldown");
    }
    onHotChanged: {
        CoolDown.hot = hot;
        throttle(hot && (!asking || CoolDown.accepted));
        if (!hot) {
            if (hotUid)
                remove(hotUid); // the alert (or an unanswered question) about a spell that's over
            hotUid = 0;
        }
    }
    // Ask mode: Low Power comes with the yes
    Connections {
        target: CoolDown
        function onAcceptedChanged() {
            if (CoolDown.accepted && root.hot)
                root.throttle(true);
        }
    }
    Connections {
        target: Resources
        function onTemperatureChanged() {
            const t = Resources.temperature;
            if (!root.wants("temperature")) {
                root.hot = false;
                root.tempSamples = [];
                return;
            }
            if (t <= 0)
                return; // a failed reading: ignore it rather than "cooling down"
            const now = Date.now();
            root.tempSamples = root.tempSamples.filter(x => x.at >= now - root.coolAfter - 5000).concat([{ t: t, at: now }]);
            const warm = root.averageOver(root.hotAfter, now);
            const cool = root.averageOver(root.coolAfter, now);
            if (!root.hot && warm >= root.hotTemp) {
                root.hot = true;
                if (root.asking)
                    root.post({
                        kind: "temperature", icon: "thermometer-hot", title: "Running hot", subtitle: "CPU " + Math.round(t) + " °C", value: Math.min(1, t / 105), color: Theme.accents.red,
                        action: "Cool down", onAction: () => { CoolDown.accepted = true; root.remove(root.hotUid); },
                        action2: "Not now", onAction2: () => { CoolDown.declined = true; root.remove(root.hotUid); }
                    }, 86400000); // stays until answered, or the spell ends
                else
                    root.post({ kind: "temperature", icon: "thermometer-hot", title: "Running hot", subtitle: root.hotText(t), value: Math.min(1, t / 105), color: Theme.accents.red }, root.untilOver); // until it cools, or dismissed
                root.hotUid = root.newestUid;
            } else if (root.hot && cool >= 0 && cool <= root.hotTemp - 5) {
                root.hot = false;
            } else if (root.hot && root.hotUid) {
                root.update(root.hotUid, { subtitle: root.asking ? "CPU " + Math.round(t) + " °C" : root.hotText(t), value: Math.min(1, t / 105) });
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
        function hot(on: bool): void {
            root.hotPreview = on;
            if (!on && !root.hot && root.hotUid) { // a demo spell ends
                root.remove(root.hotUid);
                root.hotUid = 0;
            }
        }
        // qs ipc call island dismissNewest  (as a swipe up, the close button or Esc)
        function dismissNewest(): void {
            if (root.count > 0 && root.byUid[root.newestUid]?.kind === "ringing")
                Calls.decline(); // (Esc on a ringing call declines it)
            else if (root.count > 0)
                root.remove(root.newestUid);
        }
        // qs ipc call island hold true|false  (as the pointer resting on the alerts)
        function holdAlerts(on: bool): void { root.hold("ipc", on); }
        // qs ipc call island history  (what Recent and Notification Center list, as JSON)
        function history(): string { return JSON.stringify(root.history); }
        function clearHistory(): void { root.clearHistory(); }
        // Seconds left on each live alert, newest last
        function timeLeft(): string {
            const now = Date.now(), out = [];
            for (let i = 0; i < root.alerts.count; i++) {
                const ev = root.byUid[root.alerts.get(i).uid];
                out.push((ev?.title ?? "") + ": " + Math.round(((ev?.expires ?? now) - now) / 1000) + " s");
            }
            return out.join("\n");
        }
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
                wifi: { kind: "network", icon: "wifi-high", title: "Home Wi-Fi", subtitle: "Wi-Fi connected", color: Theme.accents.blue },
                vpn: { kind: "network", icon: "shield-check", title: "Work VPN", subtitle: "VPN connected", color: Theme.accents.green, ripple: true },
                temperature: { kind: "temperature", icon: "thermometer-hot", title: "Running hot", subtitle: "CPU 94 °C · close heavy apps or let it cool", value: 0.9, color: Theme.accents.red },
                notification: { kind: "notification", headline: "Here, I found the door code in your messages from Mac.", card: { name: "Mac Tyler", time: "Thursday", text: "There's a spare key on the hook by the coat rack! The front door code is #1997", initials: "MT", appIcon: "org.gnome.Polari" } },
                music: { kind: "music", icon: "music-notes", title: "Midnight City", subtitle: "M83", color: Theme.accent, art: "" },
                download: { kind: "transfer", icon: "download-simple", title: "Downloaded report.pdf", subtitle: "In ~/Downloads", color: Theme.accents.green, action: "Open", onAction: () => {}, action2: "Show in Folder", onAction2: () => {} }
            }[kind] ?? { kind: "custom", icon: "bell", title: "Hello", subtitle: "A custom alert", color: Theme.accent };
            // As long as the real ones stay up
            const ms = kind === "battery" || kind === "temperature" ? root.untilOver : kind === "usb" || kind === "download" ? root.longMs : 5000;
            if (kind === "burst") { // several at once, as when headphones connect while charging
                for (const k of ["device", "charging", "audio"])
                    demo(k);
                return;
            }
            root.post(d, ms);
            if (kind === "temperature")
                root.hotUid = root.newestUid; // (ends with `island hot false`)
            if (kind === "battery")
                root.lowUid = root.newestUid;
            started.elapsed = was;
        }
    }
}
