import QtQuick
import QtQml
import Quickshell
import Quickshell.Networking
import qs.common
import qs.services

// While tessera-ui draws the menu bar's Wi-Fi menu (rust.surfaces: network),
// what only this shell knows reaches it here, one line on its stdin
// (RustUi.tell("network", …)): Wi-Fi (the switch, the networks as WifiList's
// snapshot with their status lines, the notice without NetworkManager), the
// speed test (SpeedTest's state, with the strings it formats) and where the
// menu goes (Panels.anchorX). What it asks (RustUi's "network" call) is done
// here: the speed test, the switch, rescanning, joining, forgetting, Settings.
// (native/tessera-ui/src/surfaces/network reads what this sends)
Scope {
    id: root

    readonly property bool here: RustUi.drawing("network")
    readonly property bool open: here && Panels.open === "wifiMenu"
    // Rust says: the pointer is over the menu (or its row menu is open), and
    // whether a password is being typed: the rows hold still meanwhile
    property bool hovered: false
    property bool asking: false

    // ── Wi-Fi (WifiList's) ──────────────────────────────────────────────
    readonly property var device: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    property var networks: []
    property string failed: "" // the network that last failed to connect
    function enterprise(n) {
        return [WifiSecurityType.Wpa2Eap, WifiSecurityType.WpaEap, WifiSecurityType.Wpa3SuiteB192, WifiSecurityType.Leap, WifiSecurityType.DynamicWep].includes(n?.security);
    }
    function secured(n) {
        return n.security !== WifiSecurityType.Open && n.security !== WifiSecurityType.Unknown;
    }
    function sortedNow() {
        return (device?.networks.values ?? []).slice().sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 30);
    }
    // A snapshot, not a live sort (WifiList): refreshed every few seconds, not
    // while the pointer is over the menu or a password is being typed
    function refresh(force) {
        const next = sortedNow();
        if (!force && next.length === networks.length && next.every((n, i) => n === networks[i]))
            return;
        networks = next;
    }
    function live(name) {
        return (device?.networks.values ?? []).find(x => x.name === name) ?? null;
    }
    Timer {
        running: root.open
        interval: 4000
        repeat: true
        onTriggered: if (!root.hovered && !root.asking) root.refresh(false)
    }
    Connections {
        target: root.device?.networks ?? null
        ignoreUnknownSignals: true
        function onValuesChanged() {
            if (!root.open || root.asking)
                return;
            const now = root.device?.networks.values ?? [];
            if (root.networks.length === 0 || root.networks.some(n => !n || !now.includes(n)))
                root.refresh(false);
        }
    }
    // Scan only while the menu shows
    Binding {
        target: root.device
        property: "scannerEnabled"
        value: true
        when: root.open && root.device !== null
        restoreMode: Binding.RestoreBindingOrValue
    }
    Instantiator {
        model: root.networks
        delegate: Connections {
            required property var modelData
            target: modelData
            ignoreUnknownSignals: true // (connectionFailed is newer than Quickshell 0.2)
            function onConnectionFailed() {
                root.failed = modelData.name;
            }
            function onConnectedChanged() {
                if (modelData.connected && root.failed === modelData.name)
                    root.failed = "";
            }
        }
    }
    readonly property var wifi: ({
            adapter: root.device !== null,
            enabled: Networking.wifiEnabled,
            unmanaged: Backends.wifiUnmanaged,
            notice: {
                title: Backends.wifiTitle,
                text: Backends.wifiReason,
                action: Backends.canStartNm ? "Start NetworkManager" : Backends.wifiApp !== "" ? "Open " + Backends.wifiAppName : "",
                busy: Health.fixing === "networkmanager"
            },
            networks: root.networks.filter(n => !!n).map(n => ({
                        name: n.name,
                        signal: n.signalStrength,
                        known: !!n.known,
                        connected: !!n.connected,
                        open: n.security === WifiSecurityType.Open,
                        secured: root.secured(n),
                        enterprise: root.enterprise(n),
                        status: n.connected ? "Connected" : n.state === ConnectionState.Connecting || WifiJoin.joining(n.name) ? "Connecting…" : root.failed === n.name || WifiJoin.failedFor(n.name) !== "" ? "Couldn't connect" : n.known ? "Known network" : "",
                        busy: !!n.stateChanging || WifiJoin.joining(n.name),
                        failed: WifiJoin.failedFor(n.name)
                    }))
        })

    // ── The speed test (SpeedTestCard's) ────────────────────────────────
    readonly property bool hasRun: SpeedTest.stage !== "idle" && SpeedTest.stage !== "error"
    readonly property var shownResult: hasRun ? SpeedTest : SpeedTest.last
    readonly property string phaseText: ({ latency: "Measuring ping…", download: "Testing download…", upload: "Testing upload…" })[SpeedTest.stage] ?? ""
    readonly property var speed: {
        const s = root.shownResult;
        const parts = [];
        if (s && (s.jitter ?? -1) >= 0)
            parts.push("Jitter " + SpeedTest.formatMs(s.jitter));
        if (s && ((s.loadedDown ?? -1) >= 0 || (s.loadedUp ?? -1) >= 0))
            parts.push("Under load " + SpeedTest.formatMs(s.loadedDown ?? -1) + " ↓ " + SpeedTest.formatMs(s.loadedUp ?? -1) + " ↑");
        const srv = root.hasRun ? [[SpeedTest.server.name, SpeedTest.server.location].filter(x => x).join(", "), SpeedTest.client.isp].filter(x => x).join(" · ")
            : [SpeedTest.last?.server, SpeedTest.last?.isp].filter(x => x).join(" · ");
        return {
            running: SpeedTest.running,
            stage: SpeedTest.stage,
            sub: SpeedTest.running ? root.phaseText : SpeedTest.stage === "done" ? "Just now · " + SpeedTest.formatBytes(SpeedTest.usedBytes) + " used" : "",
            lastAt: !SpeedTest.running && SpeedTest.stage !== "done" && SpeedTest.stage !== "error" && SpeedTest.last ? SpeedTest.last.at : 0,
            live: SpeedTest.stage === "latency" ? SpeedTest.formatMs(SpeedTest.ping) : SpeedTest.format(SpeedTest.live),
            progress: SpeedTest.progress,
            showStats: s !== null && s !== undefined && SpeedTest.stage !== "error",
            stats: [
                { label: "Download", icon: "arrow-down", value: SpeedTest.format(s?.download ?? -1), active: SpeedTest.stage === "download" },
                { label: "Upload", icon: "arrow-up", value: SpeedTest.format(s?.upload ?? -1), active: SpeedTest.stage === "upload" },
                { label: "Ping", icon: "timer", value: SpeedTest.formatMs(s?.ping ?? -1), active: SpeedTest.stage === "latency" }
            ],
            resp: parts.join(" · "),
            verdict: root.hasRun ? SpeedTest.verdict(SpeedTest) : SpeedTest.verdict(SpeedTest.last),
            server: srv,
            error: SpeedTest.stage === "error" ? SpeedTest.error : "",
            action: SpeedTest.running ? "Cancel" : SpeedTest.stage === "done" || SpeedTest.stage === "error" ? "Test again" : "Test speed"
        };
    }

    // ── Sending ─────────────────────────────────────────────────────────
    function send() {
        RustUi.tell("network", { wifi: root.wifi, speed: root.speed, anchorX: Panels.anchorX });
    }
    // (while it shows: the live numbers come often, a few lines a second at most)
    onWifiChanged: if (open) later.restart()
    onSpeedChanged: if (open) later.restart()
    Timer {
        id: later
        interval: 60
        onTriggered: root.send()
    }
    // Opening: fresh rows and Ookla's tools looked up (as the card does), and
    // said at once, before the open itself reaches tessera-ui
    onOpenChanged: {
        if (open) {
            refresh(true);
            SpeedTest.probe();
            send();
        } else {
            hovered = false;
            asking = false;
        }
    }
    Connections {
        target: Panels
        function onAnchorXChanged() {
            if (root.here)
                root.send();
        }
    }
    Connections {
        target: RustUi
        // (said again when tessera-ui starts, or starts drawing the menu)
        function onDrawnChanged() {
            if (root.here)
                later.restart();
        }
        function onStarted() {
            later.restart();
        }
        function onFeedCall(surface, what, a) {
            if (surface !== "network")
                return;
            root.call(what, a);
        }
    }

    function call(what, a) {
        switch (what) {
        case "hold":
            hovered = !!a[0];
            asking = !!a[1];
            break;
        case "speedStart":
            SpeedTest.start();
            break;
        case "speedCancel":
            SpeedTest.cancel();
            break;
        case "wifi":
            Networking.wifiEnabled = !!a[0];
            break;
        case "rescan":
            if (device) {
                device.scannerEnabled = false;
                device.scannerEnabled = true;
            }
            refresh(true);
            break;
        case "connect": {
            const n = live(String(a[0] ?? ""));
            if (n) {
                if (failed === n.name)
                    failed = "";
                n.connect();
            }
            break;
        }
        case "disconnect":
            live(String(a[0] ?? ""))?.disconnect();
            break;
        case "join": { // (the password: only here, never logged)
            const n = live(String(a[0] ?? ""));
            if (n && typeof a[1] === "string") {
                failed = "";
                n.connectWithPsk(a[1]);
            }
            break;
        }
        case "forget": {
            const n = live(String(a[0] ?? ""));
            const name = String(a[0] ?? "");
            if (name === "")
                break;
            if (n && typeof n.forget === "function")
                n.forget();
            else
                WifiJoin.forget(name);
            WifiJoin.loadKnown();
            refresh(true);
            break;
        }
        case "copy":
            if (String(a[0] ?? "") !== "")
                Quickshell.execDetached(["wl-copy", "--", String(a[0])]);
            break;
        case "settings":
            Panels.openSettings("network");
            break;
        case "fix":
            if (Backends.canStartNm)
                Backends.startNm();
            else
                Backends.openWifiApp();
            break;
        }
    }
}
