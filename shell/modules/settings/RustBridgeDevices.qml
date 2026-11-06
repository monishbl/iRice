import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Bluetooth
import Quickshell.Networking
import qs.common
import qs.services

// Settings drawn in Rust: the "devices" page group's side in this shell
// (RustBridge.qml): Displays, Sound, Wi-Fi & Bluetooth, Phone & Sharing.
// While Settings is open (`live`) it tells the page on show what only this
// shell knows (`feed`, the pages' cx.feed["devices"]: the displays as
// services/Displays.qml has them, the Wi-Fi networks, the speed test, the
// phone, AirPlay, sharing…) and does what they ask (`act`:
// cx.ask("devices", what, args)): everything that changes the system goes
// through this shell's own services, as the QML pages did. Bluetooth's
// device list and the sound devices are tessera-ui's own (BlueZ, the sound
// server); this adds what only Quickshell knows about them.
Scope {
    id: root

    property bool live: false
    readonly property string page: live ? Panels.settingsPage : ""

    // ── While a page is up: look, scan, read (as the pages did) ─────────
    onPageChanged: {
        if (page === "network") {
            SpeedTest.probe();
            SpeedTest.readLink();
            WifiJoin.loadKnown();
        } else if (page === "sound") {
            AirPlay.check();
        } else if (page === "displays") {
            Displays.refresh();
        } else if (page === "phone") {
            Phone.checkCam();
            fm(["status"]);
            checkSystem();
        }
        const look = page === "displays" && Mirroring.available;
        if (look !== lookingTvs) {
            lookingTvs = look;
            Mirroring.look(look);
        }
        if (page !== "network") {
            asking = "";
            if (WifiShare.stage !== "")
                WifiShare.close();
        }
    }
    property bool lookingTvs: false
    Connections {
        target: Mirroring
        function onAvailableChanged() {
            const look = root.page === "displays" && Mirroring.available;
            if (look !== root.lookingTvs) {
                root.lookingTvs = look;
                Mirroring.look(look);
            }
        }
    }
    Timer { // (the link changes as you move around)
        running: root.page === "network"
        interval: 5000
        repeat: true
        onTriggered: SpeedTest.readLink()
    }
    readonly property var wifiDevice: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    Binding {
        target: root.wifiDevice
        property: "scannerEnabled"
        value: true
        when: root.page === "network" && root.wifiDevice !== null
        restoreMode: Binding.RestoreBindingOrValue
    }
    readonly property var adapter: Bluetooth.defaultAdapter
    Binding {
        target: root.adapter
        property: "discovering"
        value: true
        when: root.page === "network" && (root.adapter?.enabled ?? false)
        restoreMode: Binding.RestoreBindingOrValue
    }
    Timer { // (rfkill and the service change without BlueZ telling)
        running: root.page === "network" && Backends.btState !== "ok" && Backends.btState !== ""
        interval: 3000
        repeat: true
        onTriggered: Backends.recheck()
    }
    // A Wi-Fi network failed to connect (its last try)
    property string failed: ""
    property string asking: "" // (a password row is open: the list holds still)
    property bool reveal: false // (the share card shows the password)
    property var saved: null // a saved enterprise network's settings, for the join form

    // ── Sharing's helpers (sharing.qml) ──────────────────────────────────
    property var managers: []
    property string fmError: ""
    readonly property string fmScript: decodeURIComponent(Qt.resolvedUrl("../../assets/scripts/file-manager-share.py").toString().replace("file://", ""))
    function fm(args) {
        fmProc.command = ["python3", "-I", fmScript].concat(args);
        fmProc.running = true;
    }
    Process {
        id: fmProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.managers = JSON.parse(text);
                    root.fmError = "";
                } catch (e) {}
            }
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim() !== "") root.fmError = text.trim()
        }
    }
    property var system: ({})
    property string fixing: ""
    property string fixError: ""
    readonly property string checkScript: decodeURIComponent(Qt.resolvedUrl("../../assets/scripts/sharing-check.py").toString().replace("file://", ""))
    function checkSystem() {
        checkProc.command = ["python3", "-I", checkScript, "status"];
        checkProc.running = true;
    }
    function fixSystem(what) {
        fixing = what;
        fixError = "";
        fixProc.command = ["python3", "-I", checkScript, "fix", what];
        fixProc.running = true;
    }
    Process {
        id: checkProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.system = JSON.parse(text);
                } catch (e) {}
            }
        }
    }
    Process {
        id: fixProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const r = JSON.parse(text);
                    root.fixError = r.ok ? "" : r.error;
                } catch (e) {
                    root.fixError = "It didn't work; try again.";
                }
                root.fixing = "";
                root.checkSystem();
            }
        }
    }

    // ── What the pages show ──────────────────────────────────────────────
    function enterprise(n) {
        return [WifiSecurityType.Wpa2Eap, WifiSecurityType.WpaEap, WifiSecurityType.Wpa3SuiteB192, WifiSecurityType.Leap, WifiSecurityType.DynamicWep].includes(n?.security);
    }
    function secured(n) {
        return n.security !== WifiSecurityType.Open && n.security !== WifiSecurityType.Unknown;
    }
    function network(name) {
        return (wifiDevice?.networks.values ?? []).find(n => n.name === name) ?? null;
    }
    function device(address) {
        return (adapter?.devices.values ?? []).find(d => d.address === address) ?? null;
    }
    function shown(r) {
        return r ? ({ download: r.download ?? -1, upload: r.upload ?? -1, ping: r.ping ?? -1, jitter: r.jitter ?? -1, loadedDown: r.loadedDown ?? -1, loadedUp: r.loadedUp ?? -1 }) : null;
    }
    function ago(ms) {
        const m = Math.round((Date.now() - ms) / 60000);
        return m < 1 ? "just now" : m < 60 ? m + " min ago" : m < 1440 ? Math.round(m / 60) + " h ago" : Math.round(m / 1440) + " d ago";
    }
    // SpeedTestCard.qml's texts
    function speedCard() {
        const hasRun = SpeedTest.stage !== "idle" && SpeedTest.stage !== "error";
        const s = hasRun ? SpeedTest : SpeedTest.last;
        const parts = [];
        if (s && (s.jitter ?? -1) >= 0)
            parts.push("Jitter " + SpeedTest.formatMs(s.jitter));
        if (s && ((s.loadedDown ?? -1) >= 0 || (s.loadedUp ?? -1) >= 0))
            parts.push("Under load " + SpeedTest.formatMs(s.loadedDown ?? -1) + " ↓ " + SpeedTest.formatMs(s.loadedUp ?? -1) + " ↑");
        const err = SpeedTest.stage === "error";
        return {
            running: SpeedTest.running,
            sub: SpeedTest.running ? (({ latency: "Measuring ping…", download: "Testing download…", upload: "Testing upload…" })[SpeedTest.stage] ?? "")
                : SpeedTest.stage === "done" ? "Just now · " + SpeedTest.formatBytes(SpeedTest.usedBytes) + " used"
                : SpeedTest.last && !err ? "Last test " + ago(SpeedTest.last.at) : "",
            button: SpeedTest.running ? "Cancel" : SpeedTest.stage === "done" || err ? "Test again" : "Test speed",
            live: SpeedTest.stage === "latency" ? SpeedTest.formatMs(SpeedTest.ping) : SpeedTest.format(SpeedTest.live),
            progress: SpeedTest.progress,
            stats: s !== null && !err,
            download: SpeedTest.format(s?.download ?? -1),
            upload: SpeedTest.format(s?.upload ?? -1),
            ping: SpeedTest.formatMs(s?.ping ?? -1),
            stage: SpeedTest.stage,
            extra: s && !err ? parts.join(" · ") : "",
            verdict: !SpeedTest.running && !err ? (hasRun ? SpeedTest.verdict(SpeedTest) : SpeedTest.verdict(SpeedTest.last)) || "" : "",
            server: err ? "" : hasRun ? [[SpeedTest.server.name, SpeedTest.server.location].filter(x => x).join(", "), SpeedTest.client.isp].filter(x => x).join(" · ") : [SpeedTest.last?.server, SpeedTest.last?.isp].filter(x => x).join(" · "),
            error: err ? SpeedTest.error : ""
        };
    }
    readonly property var feed: {
        if (!live)
            return null;
        const p = page;
        const f = { page: p, healthFixing: Health.fixing, updatesRunning: Updates.running, needs: { checked: Needs.checked, avahi: Needs.avahi, pipewire: Needs.pipewire, networkManager: Needs.networkManager, installing: Needs.installing } };
        if (p === "network") {
            const nets = (wifiDevice?.networks.values ?? []).slice().sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 30);
            f.wifi = {
                device: wifiDevice !== null,
                enabled: Networking.wifiEnabled,
                networks: nets.map(n => ({
                    name: n.name,
                    signal: n.signalStrength,
                    connected: n.connected,
                    known: n.known,
                    secured: secured(n),
                    open: n.security === WifiSecurityType.Open,
                    enterprise: enterprise(n),
                    connecting: n.state === ConnectionState.Connecting || WifiJoin.joining(n.name),
                    busy: n.stateChanging || WifiJoin.joining(n.name),
                    failed: root.failed === n.name || WifiJoin.failedFor(n.name) !== "",
                    failedText: WifiJoin.failedFor(n.name)
                })),
                known: WifiJoin.knownNetworks,
                forgetError: WifiJoin.forgetError,
                join: { ssid: WifiJoin.ssid, stage: WifiJoin.stage, error: WifiJoin.error, busy: WifiJoin.busy },
                saved: root.saved,
                share: { stage: WifiShare.stage, ssid: WifiShare.ssid, qr: WifiShare.qr, error: WifiShare.error, hasPassword: WifiShare.password !== "", password: root.reveal ? WifiShare.password : "" }
            };
            f.backends = {
                wifiUnmanaged: Backends.wifiUnmanaged, wifiTitle: Backends.wifiTitle, wifiReason: Backends.wifiReason, canStartNm: Backends.canStartNm, wifiApp: Backends.wifiApp, wifiAppName: Backends.wifiAppName,
                btState: Backends.btState, btTitle: Backends.btTitle, btReason: Backends.btReason, btFix: Backends.btFix, btFixLabel: Backends.btFixLabel, btFixing: Backends.btFixing, btFixError: Backends.btFixError
            };
            const extras = {};
            for (const d of adapter?.devices.values ?? [])
                extras[d.address] = { trusted: d.trusted, pairing: d.pairing, busy: d.state === BluetoothDeviceState.Connecting || d.state === BluetoothDeviceState.Disconnecting || d.pairing, picture: String(Devices.pictureOf(d) ?? "").replace(/^file:\/\//, ""), name: d.name ?? "" };
            f.bt = { adapter: adapter !== null, enabled: adapter?.enabled ?? false, name: adapter?.name ?? "", devices: extras };
            f.speed = speedCard();
            f.speedTest = {
                backend: SpeedTest.backend, running: SpeedTest.running, ooklaReady: SpeedTest.ooklaReady, hasOokla: SpeedTest.hasOokla, ooklaInstall: SpeedTest.ooklaInstall,
                link: SpeedTest.link, signalWords: SpeedTest.signalWords(SpeedTest.link?.signal ?? -100),
                ip: SpeedTest.client.ip || SpeedTest.last?.ip || "", isp: SpeedTest.client.isp || SpeedTest.last?.isp || "",
                history: SpeedTest.history.map(h => ({
                    label: new Date(h.at).toLocaleString(Qt.locale(), "d MMM, HH:mm") + (h.network ? " · " + h.network : ""),
                    detail: [h.backend === "ookla" ? "Ookla" : "Cloudflare", String(h.server ?? "").replace(/^Cloudflare /, ""), h.ping >= 0 ? "ping " + SpeedTest.formatMs(h.ping) : "", h.loadedDown >= 0 ? "under load " + SpeedTest.formatMs(h.loadedDown) : ""].filter(x => x).join(" · "),
                    value: "↓ " + SpeedTest.format(h.download) + "   ↑ " + SpeedTest.format(h.upload)
                }))
            };
        } else if (p === "sound") {
            f.airplay = { checked: AirPlay.checked, installed: AirPlay.installed, installing: AirPlay.installing, installError: AirPlay.installError, error: AirPlay.error, wanted: AirPlay.wanted, speakers: AirPlay.speakers.length };
        } else if (p === "displays") {
            f.displays = {
                loaded: Displays.loaded,
                confirming: Displays.confirming,
                confirmLeft: Displays.confirmLeft,
                monitors: Displays.monitors.map(m => {
                    const s = Displays.logicalSize(m);
                    return Object.assign({}, m, { lw: s.w, lh: s.h, number: Displays.numberOf(m.name), scales: [1, 1.25, 1.5, 1.75, 2, Math.round(m.scale * 100) / 100].map(v => {
                                const z = Displays.logicalSize(m, { mode: m.mode, scale: v, transform: m.transform });
                                return { v: v, w: z.w, h: z.h };
                            }) });
                }),
                saved: (Config.options.displays.monitors ?? []).length,
                lowered: Object.keys(CoolDown.savedRates ?? {})
            };
            f.ddc = { displays: Brightness.ddcDisplays.map(d => d.output), values: Brightness.ddcValues, installed: Brightness.ddcInstalled, problem: Brightness.ddcProblem, scanned: Brightness.ddcScanned, scanning: Brightness.ddcScanning };
            f.mirroring = { available: Mirroring.available, error: Mirroring.error, p2p: Mirroring.p2p, installing: Mirroring.installing, installError: Mirroring.installError, target: Mirroring.target, state: Mirroring.state_, displays: Mirroring.displays.map(d => ({ uuid: d.uuid, name: d.name, protocol: Mirroring.protocolName(d.protocol) })) };
        } else if (p === "phone") {
            const ph = Phone.phone;
            f.phone = {
                phone: ph ? { model: ph.model ?? "", android: ph.android ?? "", battery: ph.battery ?? -1, charging: !!ph.charging, wireless: !!ph.wireless, serial: ph.serial ?? "" } : null,
                mirroring: ph ? Phone.mirroring(ph.serial ?? "") : false,
                hasAdb: Phone.hasAdb, hasScrcpy: Phone.hasScrcpy, qrencode: Tools.has("qrencode"),
                installing: Phone.installing, installError: Phone.installError, waiting: !!Phone.waiting,
                pairStage: Phone.pairStage, pairQr: Phone.pairQr, pairError: Phone.pairError,
                camReady: Phone.camReady, camModule: Phone.camModule, kernelPackage: Phone.kernelPackage, camSetting: Phone.camSetting, camError: Phone.camError, webcam: Phone.webcam
            };
            f.sharing = {
                localSend: LocalSend.available, localSendError: LocalSend.error ?? "",
                quickShare: QuickShare.available, quickShareName: QuickShare.name, quickShareError: QuickShare.error ?? "", btAvailable: QuickShare.bluetoothAvailable, btOn: QuickShare.bluetoothOn,
                portsError: SharingPorts.error, pairingRisk: root.system.pairing?.risk ?? false, passwordless: !!root.system.passwordless, fixing: root.fixing, fixError: root.fixError,
                managers: root.managers, fmError: root.fmError, fmBusy: fmProc.running, pickerBusy: FilePicker.busy, user: Quickshell.env("USER") ?? ""
            };
        }
        return f;
    }
    onFeedChanged: if (live) RustUi.tell("settings", { feed: { devices: feed } })

    // ── What the pages ask ───────────────────────────────────────────────
    Timer {
        id: forgetLater
        interval: 800
        onTriggered: WifiJoin.loadKnown()
    }
    Timer {
        id: camAgain
        interval: 800
        onTriggered: Phone.setWebcam(true)
    }
    function forgetWifi(name) {
        const n = network(name);
        if (n && n.known && typeof n.forget === "function") {
            n.forget();
            forgetLater.restart();
        } else {
            WifiJoin.forget(name);
        }
        WifiJoin.loadKnown();
    }
    function act(what, args) {
        const a0 = args[0];
        switch (what) {
        // Wi-Fi
        case "wifiEnable":
            Networking.wifiEnabled = !!a0;
            break;
        case "wifiRescan":
            if (wifiDevice) {
                wifiDevice.scannerEnabled = false;
                wifiDevice.scannerEnabled = true;
            }
            break;
        case "wifiConnect": {
            const n = network(String(a0));
            if (n) {
                root.failed = "";
                n.connect();
            }
            break;
        }
        case "wifiDisconnect":
            network(String(a0))?.disconnect();
            break;
        case "wifiJoin": { // [name, password]
            const n = network(String(a0));
            if (n) {
                root.failed = "";
                n.connectWithPsk(String(args[1] ?? ""));
            }
            root.asking = "";
            break;
        }
        case "wifiAsking":
            root.asking = String(a0 ?? "");
            break;
        case "wifiForget":
            forgetWifi(String(a0));
            break;
        case "wifiShare":
            root.reveal = false;
            WifiShare.share(String(a0));
            break;
        case "wifiShareClose":
            root.reveal = false;
            WifiShare.close();
            break;
        case "wifiShareReveal":
            root.reveal = !root.reveal;
            break;
        case "wifiShareCopy":
            Clipboard.copyText(WifiShare.password);
            break;
        case "wifiForm": // [ssid]: a saved network's settings (never its secrets)
            WifiJoin.dismiss();
            root.saved = null;
            if (a0)
                WifiJoin.saved(String(a0), r => root.saved = r);
            break;
        case "wifiJoinForm": // [request] (WifiJoinForm's submit)
            WifiJoin.join(a0);
            break;
        case "wifiDismiss":
            WifiJoin.dismiss();
            break;
        case "installTools": // [[packages]] (the share card's qrencode)
            Needs.install(a0 ?? []);
            break;
        case "startNm":
            Backends.startNm();
            break;
        case "openWifiApp":
            Backends.openWifiApp();
            break;
        case "speedTest":
            if (SpeedTest.running)
                SpeedTest.cancel();
            else
                SpeedTest.start();
            break;
        case "speedClear":
            SpeedTest.clearHistory();
            break;
        // Bluetooth
        case "btEnable":
            if (adapter)
                adapter.enabled = !!a0;
            break;
        case "btRescan":
            if (adapter?.enabled) {
                adapter.discovering = false;
                adapter.discovering = true;
            }
            break;
        case "btFix":
            Backends.fixBluetooth();
            break;
        case "btConnect":
            device(String(a0))?.connect();
            break;
        case "btDisconnect":
            device(String(a0))?.disconnect();
            break;
        case "btPair":
            device(String(a0))?.pair();
            break;
        case "btCancelPair":
            device(String(a0))?.cancelPair();
            break;
        case "btTrust": {
            const d = device(String(a0));
            if (d)
                d.trusted = !!args[1];
            break;
        }
        case "btForget":
            device(String(a0))?.forget();
            break;
        // Sound
        case "airplayInstall":
            AirPlay.install();
            break;
        case "avahiOn":
            Needs.turnOnAvahi();
            break;
        // Displays
        case "displaysSet": // [name, changes]
            Displays.set(String(a0), args[1] ?? {});
            break;
        case "displaysSetMany": // [[{ name, changes }]]
            Displays.setMany(a0 ?? []);
            break;
        case "displaysIdentify":
            Displays.identify();
            break;
        case "displaysKeep":
            Displays.keep();
            break;
        case "displaysRevert":
            Displays.revert();
            break;
        case "displaysReset":
            Displays.resetAll();
            break;
        case "ddcSet":
            Brightness.setDdc(String(a0), Number(args[1] ?? 0.5));
            break;
        case "ddcRescan":
            Brightness.rescanDdc();
            break;
        case "mirrorInstall":
            Mirroring.install();
            break;
        case "mirrorStart": {
            const d = Mirroring.displays.find(x => x.uuid === a0);
            if (d)
                Mirroring.start(d);
            break;
        }
        case "mirrorStop":
            Mirroring.stop();
            break;
        case "fxPreview": // [animation, side]
            DisplayFx.preview(String(a0), String(args[1] ?? "left"));
            break;
        // Phone
        case "phoneInstall":
            Phone.install(!!a0);
            break;
        case "phoneMirror":
            Phone.mirror(String(a0));
            break;
        case "phonePair":
            Phone.pair();
            break;
        case "phoneCancelPair":
            Phone.cancelPair();
            break;
        case "phoneSetUpCam":
            Phone.setUpCam();
            break;
        case "phoneWebcam":
            Phone.setWebcam(!!a0);
            break;
        case "phoneCameraChanged": // (again, with the other camera)
            if (Phone.webcam) {
                Phone.setWebcam(false);
                camAgain.restart();
            }
            break;
        // Sharing
        case "sendFiles":
            Panels.pickShare();
            break;
        case "quickShareBluetooth":
            QuickShare.turnOnBluetooth();
            break;
        case "sharingFix":
            fixSystem(String(a0));
            break;
        case "fileManager": // [install|remove, id]
            fm([String(a0), String(args[1] ?? "")]);
            break;
        case "receiveLink":
            LinkShare.receive();
            break;
        case "shareSelection":
            Sharing.shareSelection();
            break;
        default:
            console.warn("settings (devices): no act " + what);
        }
    }
}
