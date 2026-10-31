pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import qs.common

// An Android phone on this computer (assets/scripts/phone.py): its screen in
// a window you control with the mouse and keyboard (scrcpy; drop files on
// it to copy them to the phone's Downloads), its camera as a webcam, in the
// Dock while it's connected, and paired without a cable by QR code.
// Settings → Phone sets it up.
Singleton {
    id: root

    readonly property var cfg: Config.options.phone
    readonly property string script: Quickshell.shellDir + "/assets/scripts/phone.py"
    readonly property bool hasAdb: Tools.has("adb")
    readonly property bool hasScrcpy: Tools.has("scrcpy")

    // ── Phones (adb track-devices) ──────────────────────────────────────
    property var devices: [] // [{ serial, state, wireless, model, android, battery, charging }]
    readonly property var phone: devices.find(d => d.state === "device") ?? null
    readonly property var waiting: devices.find(d => d.state === "unauthorized") ?? null
    Process {
        id: watcher
        running: root.cfg.enabled && root.hasAdb
        command: [Tools.watcher, "python3", "-I", root.script, "watch"]
        stdinEnabled: true // (Tools.watcher: it, and adb track-devices, end with the shell)
        stdout: SplitParser {
            onRead: line => {
                let m;
                try {
                    m = JSON.parse(line);
                } catch (e) {
                    return;
                }
                const before = root.devices.map(d => d.serial + d.state);
                root.devices = m.devices ?? [];
                for (const d of root.devices)
                    if (!before.includes(d.serial + d.state))
                        root.arrived(d);
            }
        }
        onExited: if (root.cfg.enabled && root.hasAdb) rewatch.restart()
    }
    Timer { id: rewatch; interval: 5000; onTriggered: watcher.running = root.cfg.enabled && root.hasAdb }

    // Said once per connection
    function arrived(d) {
        if (d.state === "unauthorized")
            Island.post({ kind: "custom", icon: "device-mobile", title: "Allow this computer on your phone", subtitle: "Tap Allow on the phone's USB debugging prompt", color: Theme.accent }, 8000);
        else if (d.state === "device")
            Island.post({ kind: "custom", icon: "device-mobile", title: d.model || "Phone connected", subtitle: d.battery !== undefined ? d.battery + "% battery" : "Connected",
                color: Theme.accent, action: hasScrcpy ? "Show Screen" : "", onAction: () => root.mirror(d.serial) }, 6000);
    }

    // ── Its screen (scrcpy) ─────────────────────────────────────────────
    property var mirrors: ({}) // serial -> Process
    function mirroring(serial) {
        return !!mirrors[serial];
    }
    function mirror(serial) {
        if (!hasScrcpy) {
            Panels.openSettings("phone");
            return;
        }
        const d = devices.find(x => x.serial === serial) ?? phone;
        if (!d)
            return;
        if (mirrors[d.serial]) {
            // (already up: bring it forward)
            Hyprland.dispatch("hl.dsp.focus({ window = \"title:^" + (d.model || "Phone").replace(/[\^$.*+?()[\]{}|\\"]/g, "\\$&") + "$\" })");
            return;
        }
        const args = ["scrcpy", "-s", d.serial, "--window-title", d.model || "Phone", "--stay-awake", "--shortcut-mod=lalt,ralt"];
        if (cfg.screenOff)
            args.push("--turn-screen-off");
        const p = mirrorComp.createObject(root, { serial: d.serial, command: args });
        const next = Object.assign({}, mirrors);
        next[d.serial] = p;
        mirrors = next;
        p.running = true;
    }
    Component {
        id: mirrorComp
        Process {
            id: m
            property string serial
            stderr: StdioCollector { id: err }
            onExited: code => {
                const next = Object.assign({}, root.mirrors);
                delete next[m.serial];
                root.mirrors = next;
                if (code !== 0 && code !== 2) // (2: the phone went away)
                    Island.post({ kind: "custom", icon: "device-mobile", title: "Couldn't show the phone's screen", subtitle: (err.text.trim().split("\n").pop() || "scrcpy stopped").replace(/^(ERROR|WARN): /, ""), color: Theme.color.danger }, 7000);
                m.destroy();
            }
        }
    }

    // ── Its camera as a webcam (scrcpy into /dev/video42) ───────────────
    property bool camReady: false // the virtual camera exists
    property bool camAsked: false
    readonly property bool webcam: camProc.running
    function checkCam() {
        camCheck.running = true;
    }
    Process {
        id: camCheck
        command: ["sh", "-c", "test -e /dev/video42 && echo ready; modinfo v4l2loopback >/dev/null 2>&1 && echo module; :"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.camReady = text.includes("ready");
                root.camModule = text.includes("module");
            }
        }
    }
    property bool camModule: false
    Component.onCompleted: checkCam()
    function setWebcam(on) {
        if (!on) {
            if (camProc.running)
                camProc.signal(15);
            return;
        }
        if (!phone || !camReady)
            return;
        camProc.command = ["scrcpy", "-s", phone.serial, "--video-source=camera", "--camera-facing=" + (cfg.camera || "back"),
            "--v4l2-sink=/dev/video42", "--no-playback", "--no-audio", "--no-window"];
        camProc.running = true;
    }
    Process {
        id: camProc
        stderr: StdioCollector { id: camErr }
        onExited: code => {
            if (code !== 0 && code !== 2)
                root.camError = /Android 12|camera.*not supported|API/i.test(camErr.text) ? "The phone's camera can be used from Android 12 on." : (camErr.text.trim().split("\n").pop() || "The camera stopped.").replace(/^(ERROR|WARN): /, "");
        }
        onRunningChanged: if (running) root.camError = ""
    }
    property string camError: ""
    // The virtual camera: one password (irice-root phonecam on)
    property bool camSetting: false
    function setUpCam() {
        camSetting = true;
        camError = "";
        camSetup.running = true;
    }
    Process {
        id: camSetup
        command: ["pkexec", "/usr/lib/irice/irice-root", "phonecam", "on"]
        stderr: StdioCollector { id: setupErr }
        onExited: code => {
            root.camSetting = false;
            if (code !== 0 && code !== 126)
                root.camError = setupErr.text.replace(/^irice-root: /, "").trim() || "The virtual camera couldn't be set up. Update iRice, then try again.";
            root.checkCam();
        }
    }

    // ── Pairing without a cable (Android 11+) ───────────────────────────
    property string pairStage: "" // "" | waiting (the QR is up) | pairing | connecting | done | error
    property string pairQr: ""
    property string pairError: ""
    function pair() {
        if (pairer.running)
            return;
        pairStage = "starting";
        pairQr = "";
        pairError = "";
        pairer.running = true;
    }
    function cancelPair() {
        if (pairer.running)
            pairer.signal(15);
        pairStage = "";
    }
    Process {
        id: pairer
        command: ["python3", "-I", root.script, "pair"]
        stdout: SplitParser {
            onRead: line => {
                let m;
                try {
                    m = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (m.qr) {
                    root.pairQr = m.qr;
                    root.pairStage = "waiting";
                } else if (m.stage) {
                    root.pairStage = m.stage;
                } else if (m.ok !== undefined) {
                    root.pairStage = m.ok ? "done" : "error";
                    root.pairError = m.error ?? "";
                }
            }
        }
        onExited: if (root.pairStage === "waiting" || root.pairStage === "starting") root.pairStage = ""
    }

    // ── Installing scrcpy (and adb, and the camera module) ──────────────
    readonly property bool installing: Updates.running && installAsked
    property bool installAsked: false
    property string installError: ""
    function install(withCamera) {
        if (Updates.running)
            return;
        const pkgs = [];
        if (!hasAdb)
            pkgs.push("android-tools");
        if (!hasScrcpy)
            pkgs.push("scrcpy");
        if (!Tools.has("qrencode"))
            pkgs.push("qrencode"); // (pairing by QR code)
        if (withCamera && !camModule)
            pkgs.push("v4l2loopback-dkms");
        if (pkgs.length === 0)
            return;
        installAsked = true;
        installError = "";
        Updates.update(["--install", pkgs.join(",")]);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (!root.installAsked)
                return;
            root.installAsked = false;
            root.installError = j.state === "done" ? "" : (j.error || j.text || "The install didn't finish.");
            Tools.recheck();
            root.checkCam();
        }
    }
}
