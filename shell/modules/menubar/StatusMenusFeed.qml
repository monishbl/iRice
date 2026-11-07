import QtQuick
import QtQml
import Quickshell
import Quickshell.Services.UPower
import qs.common
import qs.services

// While tessera-ui draws the menu bar's status menus (rust.surfaces:
// statusmenus: Bluetooth, Sound, privacy, battery, resources, weather, Now
// Playing), what only this shell knows reaches it here, one line on its stdin
// (RustUi.tell("statusmenus", …)): BatteryHealth, the energy mode (Backends,
// PowerProfiles), Privacy, Resources and AudioProfiles' outputs. What it asks
// (RustUi's "statusmenus" call) is done here: stop sharing, the energy mode,
// a card-switching output, fixing Bluetooth, the player Now Playing shows.
// Standalone, host/system's statusmenus_feed.rs does the same.
// (native/tessera-ui/surfaces/statusmenus reads what this sends)
Scope {
    id: root

    readonly property bool here: RustUi.drawing("statusmenus")
    readonly property bool open: here && RustUi.statusPanels.includes(Panels.open)
    readonly property string menu: open ? Panels.open : ""

    readonly property string profile: !Backends.ppd ? "" : PowerProfiles.profile === PowerProfile.PowerSaver ? "power-saver" : PowerProfiles.profile === PowerProfile.Performance ? "performance" : "balanced"
    readonly property var state: ({
            health: { capacity: BatteryHealth.capacity, cycles: BatteryHealth.cycles, watts: BatteryHealth.watts, condition: BatteryHealth.condition },
            power: { ppd: Backends.ppd, hasPerformance: PowerProfiles.hasPerformanceProfile, profile: root.profile, short: Backends.powerShort ?? "" },
            privacy: {
                micApps: Privacy.micApps, cameraApps: Privacy.cameraApps, locationApps: Privacy.locationApps,
                shares: Privacy.shares.map(x => ({ app: x.app, target: x.target, window: x.window === true })),
                screenShared: Privacy.screenShared, screenApp: Privacy.screenApp, stopping: Privacy.stopping, stopFailed: Privacy.stopFailed, any: Privacy.any
            },
            resources: { cpu: Resources.cpu, memory: Resources.memory, swap: Resources.swap, memUsedGb: Resources.memUsedGb, memTotalGb: Resources.memTotalGb, temperature: Resources.temperature },
            profiles: AudioProfiles.alternatives.map(a => ({ name: a.name, kind: a.kind, busy: AudioProfiles.switching === a.profile }))
        })
    function send() {
        RustUi.tell("statusmenus", root.state);
    }
    // (a few lines a second at most while one shows)
    onStateChanged: if (open) later.restart()
    Timer {
        id: later
        interval: 250
        onTriggered: root.send()
    }
    // Opening (or another menu): fresh readings, said at once, before the
    // open itself reaches tessera-ui
    onMenuChanged: {
        if (menu === "batteryMenu")
            BatteryHealth.refresh();
        if (menu === "soundMenu")
            AudioProfiles.refresh();
        if (menu !== "")
            send();
    }
    // Resources samples once a second while its menu shows
    readonly property bool sampling: menu === "resourcesMenu"
    onSamplingChanged: Resources.graphUsers = Math.max(0, Resources.graphUsers + (sampling ? 1 : -1))
    Component.onDestruction: if (sampling) Resources.graphUsers = Math.max(0, Resources.graphUsers - 1)

    Connections {
        target: RustUi
        function onStarted() {
            if (root.open)
                root.send();
        }
        function onFeedCall(surface, what, a) {
            if (surface !== "statusmenus")
                return;
            switch (what) {
            case "stopSharing":
                Privacy.stopSharing();
                break;
            case "mode":
                if (["power-saver", "balanced", "performance"].includes(a[0]))
                    Power.setMode(a[0]);
                break;
            case "profile": {
                const alt = AudioProfiles.alternatives[Number(a[0] ?? -1)];
                if (alt)
                    AudioProfiles.pick(alt);
                break;
            }
            case "btFix":
                Backends.fixBluetooth();
                break;
            case "pin": {
                const p = Media.players.find(x => x.dbusName === a[0] || ("org.mpris.MediaPlayer2." + x.desktopEntry) === a[0]);
                if (p)
                    Media.pinned = p;
                break;
            }
            }
        }
    }
}
