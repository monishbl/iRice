import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.common
import qs.services

// While tessera-ui draws the menu bar (Settings' rust.surfaces, RustUi), what
// this shell still owns that the bar shows goes to it, and what the bar
// asks of this shell comes back (native/tessera-ui/src/services/bridge.rs):
//  - to it, on its stdin: a "state" line whenever something it shows
//    changes (the open panel, Focus, Caffeine, unread notifications, the
//    sound, privacy, weather…), and the island's ongoing activities each
//    second while there are any (Island.qml forwards the alerts itself);
//  - from it: `qs ipc call rustui act <what> <arg>` (shell-ipc.sh), for what
//    only this shell can do (open a panel under its item, an alert's button).
// The Rust bar does the rest itself (spaces, battery, Wi-Fi, Bluetooth,
// the tray, Now Playing, the clock).
Scope {
    id: root

    readonly property bool on: RustUi.drawing("menubar")

    // (the bar asked for CPU and memory: Resources samples them meanwhile)
    Binding {
        target: Resources
        property: "menubarActive"
        value: Config.options.menubar.resources
        when: root.on
    }

    readonly property string profile: !Backends.ppd ? "" : PowerProfiles.profile === PowerProfile.PowerSaver ? "power-saver" : PowerProfiles.profile === PowerProfile.Performance ? "performance" : "balanced"
    readonly property var state: ({
            src: "shell",
            kind: "state",
            open: Panels.open,
            screen: Panels.screen?.name ?? "",
            dark: Theme.dark,
            locked: System.locked,
            focus: { on: Focus.on, icon: Focus.mode?.icon ?? "moon", name: Focus.mode?.name ?? "Focus", status: Focus.statusText },
            caffeine: { on: System.keepAwake, status: System.keepAwakeStatus },
            unread: Notifications.unread,
            recording: { on: ScreenTools.recording, started: ScreenTools.recordingStarted },
            shelf: { on: Config.options.shelf.menubar, count: Shelf.count, opened: Shelf.opened, zone: Shelf.zone },
            audio: { icon: Audio.icon, volume: Audio.volume, muted: Audio.muted, sink: Audio.sink?.description || Audio.sink?.nickname || "Sound", micMuted: Audio.micMuted },
            privacy: { any: Privacy.any, camera: Privacy.camera, mic: Privacy.mic, location: Privacy.location, share: Privacy.shareDot },
            weather: { ready: Weather.ready, icon: Weather.icon, temp: Weather.temp, condition: Weather.condition, high: Weather.high, low: Weather.low, place: Weather.place },
            resources: { cpu: Resources.cpu, memory: Resources.memory },
            keyboard: { multiple: KeyboardLayout.multiple, short: KeyboardLayout.short, name: KeyboardLayout.name ?? "" },
            updates: Updates.count,
            battery: { summary: BatteryHealth.summary(), lowPower: root.profile === "power-saver" },
            power: { ppd: Backends.ppd, profile: root.profile, performance: PowerProfiles.hasPerformanceProfile },
            tools: { hyprpicker: Tools.has("hyprpicker") },
            wifi: { elsewhere: Backends.wifiUnmanaged && Backends.hasWifiHardware, short: Backends.wifiShort, linkUp: Backends.wifiLinkUp },
            island: { pinned: Island.pinned, hot: Island.showHot, fsMode: Island.fsMode, history: Island.history.length }
        })
    property string sent: ""
    function push() {
        if (!on)
            return;
        const line = JSON.stringify(state);
        if (line === sent)
            return;
        sent = line;
        RustUi.send(state);
    }
    onStateChanged: Qt.callLater(push)
    onOnChanged: {
        sent = "";
        Qt.callLater(push);
    }
    Connections {
        target: RustUi
        function onStarted() {
            root.sent = "";
            root.liveSent = "";
            Qt.callLater(root.push);
        }
    }

    // The island's ongoing activities, as LiveInfo says them (the Rust bar
    // draws music from its own player reading)
    property string liveSent: ""
    function pushLive() {
        if (!on)
            return;
        const acts = Island.activities.map(a => ({
                    kind: a.kind,
                    icon: a.kind === "music" ? a.icon : LiveInfo.icon(a.kind),
                    color: String(a.color),
                    title: a.kind === "music" ? "" : LiveInfo.title(a.kind),
                    time: a.kind === "music" ? "" : LiveInfo.time(a.kind),
                    controls: a.kind === "music" ? [] : LiveInfo.controls(a.kind).filter(b => Island.shows(a.kind, b.piece)).map(b => ({ icon: b.icon, fill: !!b.fill, on: b.on !== false, danger: !!b.danger }))
                }));
        const line = JSON.stringify(acts);
        if (line === liveSent)
            return;
        liveSent = line;
        RustUi.send({ src: "shell", kind: "live", activities: acts });
    }
    Connections {
        target: Island
        function onActivitiesChanged() {
            Qt.callLater(root.pushLive);
        }
    }
    Timer {
        running: root.on && Island.activities.length > 0
        interval: 1000
        repeat: true
        triggeredOnStart: true
        onTriggered: root.pushLive()
    }

    // What the Rust bar asks for
    function screenNamed(n) {
        return Quickshell.screens.find(s => s.name === n) ?? Panels.screen ?? Quickshell.screens[0];
    }
    function act(what, arg) {
        const a = String(arg ?? "").split(" ");
        switch (what) {
        case "toggleAt": // <panel> <screen> <x>: a panel under its item
            return Panels.toggle(a[0], screenNamed(a[1]), Number(a[2] ?? 0));
        case "page": // <ccPage> <screen>: Control Center on a page
            Panels.show("controlCenter", screenNamed(a[1]));
            Panels.ccPage = a[0];
            return;
        case "about":
            return Panels.show("about", screenNamed(a[0]));
        case "session":
            return Panels.show("session", screenNamed(a[0]));
        case "settings":
            return Panels.openSettings();
        case "installUpdates":
            return Updates.install();
        case "repairShell":
            return System.repairShell();
        case "power": // sleep | lock
            return System.power(a[0]);
        case "darkMode":
            return System.toggleDarkMode();
        case "mic":
            return Audio.toggleMic(true);
        case "record":
            return ScreenTools.recording ? ScreenTools.stopRecording() : ScreenTools.select("record");
        case "stopRecording":
            return ScreenTools.stopRecording();
        case "shelf":
            return Shelf.toggle(screenNamed(a[0]));
        case "keyboardLayout":
            return KeyboardLayout.next();
        case "powerProfile": {
            const p = PowerProfiles.profile;
            return Power.setMode(p === PowerProfile.PowerSaver ? "balanced" : p === PowerProfile.Balanced && PowerProfiles.hasPerformanceProfile ? "performance" : "power-saver");
        }
        case "launch": // <desktop entry id>
            return Apps.launch(Apps.byId(a[0]));
        case "quit": // <app id>
            return Apps.quit(a[0]);
        case "mergeTabs":
            return Apps.mergeIntoTabs();
        case "islandAct": { // <uid> <1 | 2 | more index>: an alert's button
            const e = Island.byUid[Number(a[0])];
            const since = Date.now();
            let f = null;
            if (a[1] === "1")
                f = e?.onAction ?? null;
            else if (a[1] === "2")
                f = e?.onAction2 ?? null;
            else if (a[1] === "more")
                f = e?.more?.[Number(a[2])]?.act ?? null;
            if (f)
                f();
            return Island.dismiss(since);
        }
        case "islandRemove": // <uid>: dismissed there (Esc, swipe, close)
            return Island.remove(Number(a[0]));
        case "islandHold": // 1 | 0: the pointer rests on its alerts
            return Island.hold("rust", a[0] === "1");
        case "islandPin":
            return Island.togglePin();
        case "live": { // <kind> <index>: a live activity's control
            const c = LiveInfo.controls(a[0]).filter(b => Island.shows(a[0], b.piece))[Number(a[1])];
            if (c && c.on !== false)
                c.act();
            return;
        }
        case "hud": // volume | brightness: the island's level row after a scroll
            return Island.hud(a[0]);
        }
        console.warn("RustBridge: nothing called " + what);
    }
    IpcHandler {
        target: "rustui"
        function act(what: string, arg: string): void {
            root.act(what, arg);
        }
    }
}
