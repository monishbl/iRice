pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Pipewire
import qs.common

// Device connections, shown as live alerts in the dynamic island:
//   bluetooth  devices connecting or disconnecting, with their battery
//   usb        USB devices and drives plugged in or removed (drives: Open)
//   displays   monitors connected or disconnected
//   audio      sound moving to another output (wired headphones, HDMI...)
// The charger has its own alert in Island. Nothing is posted for the first
// seconds after start, so devices already present don't announce themselves.
Singleton {
    id: root

    readonly property bool live: Island.enabled && Island.armed

    // ── Bluetooth ───────────────────────────────────────────────────────
    function btIcon(d) {
        const i = (d.icon ?? "").toLowerCase();
        return i.includes("headset") || i.includes("headphone") ? "headphones"
            : i.includes("audio") || i.includes("speaker") ? "speaker-hifi"
            : i.includes("mouse") ? "mouse"
            : i.includes("keyboard") ? "keyboard"
            : i.includes("gaming") || i.includes("joystick") ? "game-controller"
            : i.includes("phone") ? "device-mobile"
            : i.includes("watch") ? "watch"
            : i.includes("computer") ? "desktop"
            : "bluetooth";
    }
    property var btState: ({}) // address -> connected
    property real lastBtAudio: 0 // when a Bluetooth audio device connected (the output switch that follows stays quiet)
    function scanBluetooth() {
        const next = {};
        for (const d of Bluetooth.devices.values) {
            next[d.address] = d.connected;
            const was = btState[d.address] ?? false;
            if (d.connected === was || !live || !Island.wants("bluetooth"))
                continue;
            const icon = btIcon(d);
            if (d.connected && (icon === "headphones" || icon === "speaker-hifi"))
                lastBtAudio = Date.now();
            postBt(d, icon);
            // Headphones often report their battery seconds after connecting:
            // show it then (once), if it wasn't known yet
            if (d.connected && !d.batteryAvailable)
                awaitingBattery = Object.assign({}, awaitingBattery, { [d.address]: Date.now() });
        }
        btState = next;
    }
    function postBt(d, icon) {
        Island.post(Object.assign({
            kind: "device",
            icon: icon,
            title: d.name || d.deviceName || d.address,
            subtitle: d.connected ? "Connected" : "Disconnected",
            color: d.connected ? Theme.accents.blue : Qt.rgba(1, 1, 1, 0.55)
        }, d.connected && d.batteryAvailable ? { battery: d.battery } : {}), 3500);
    }
    property var awaitingBattery: ({}) // address -> when it connected without a battery reading
    function batteryArrived(d) {
        const t = awaitingBattery[d.address];
        if (!t || !d.connected || !d.batteryAvailable)
            return;
        const next = Object.assign({}, awaitingBattery);
        delete next[d.address];
        awaitingBattery = next;
        if (Date.now() - t < 20000 && live && Island.wants("bluetooth"))
            postBt(d, btIcon(d));
    }
    // Battery often arrives a moment after the connection; scan on any change
    Instantiator {
        model: Bluetooth.devices
        delegate: Connections {
            required property var modelData
            target: modelData
            function onConnectedChanged() { btScan.restart(); }
            function onBatteryAvailableChanged() { root.batteryArrived(modelData); }
            function onBatteryChanged() { root.batteryArrived(modelData); }
        }
    }
    Connections {
        target: Bluetooth.devices
        function onValuesChanged() { btScan.restart(); }
    }
    Timer {
        id: btScan
        interval: 700 // let the name and battery settle
        onTriggered: root.scanBluetooth()
    }
    Component.onCompleted: scanBluetooth()

    // ── USB devices and drives (udev) ───────────────────────────────────
    // `udevadm monitor --property` prints one block of KEY=value lines per
    // event. A drive fires both a usb_device and a block event: the device
    // alert waits a moment and gives way to the drive alert.
    property var usbNames: ({}) // devpath -> name, to name removals
    property var pendingUsb: null
    property bool driveSeen: false
    property var block: ({})
    property var driveSeenAt: ({}) // "add:/dev/sdb" -> when it was announced
    function usbIcon(p) {
        const ifs = p.ID_USB_INTERFACES ?? "";
        const name = ((p.ID_MODEL_FROM_DATABASE ?? "") + " " + (p.ID_MODEL ?? "")).toLowerCase();
        return ifs.includes(":0e") ? "webcam"
            : ifs.includes(":0301") ? "keyboard"
            : ifs.includes(":0302") ? "mouse"
            : ifs.includes(":01") ? "headphones"
            : ifs.includes(":07") ? "printer"
            : ifs.includes(":ff42") || ifs.includes(":0601") || name.includes("phone") || name.includes("android") ? "device-mobile"
            : ifs.includes(":08") ? "hard-drive"
            : "usb";
    }
    function usbName(p) {
        const model = (p.ID_MODEL_FROM_DATABASE || p.ID_MODEL || "").replace(/_/g, " ");
        const vendor = (p.ID_VENDOR_FROM_DATABASE || p.ID_VENDOR || "").replace(/_/g, " ");
        return model ? (vendor && !model.startsWith(vendor.split(" ")[0]) ? vendor.split(" ")[0] + " " + model : model) : vendor || "USB device";
    }
    function onUdev(p) {
        if (p.SUBSYSTEM === "usb" && p.DEVTYPE === "usb_device") {
            if ((p.ID_USB_INTERFACES ?? "").startsWith(":09")) // hubs
                return;
            if (p.ACTION === "add") {
                const name = usbName(p);
                usbNames = Object.assign({}, usbNames, { [p.DEVPATH]: name });
                if (live)
                    Sounds.play("usb");
                if (live && Island.wants("usb")) {
                    driveSeen = false;
                    pendingUsb = { kind: "usb", icon: usbIcon(p), title: name, subtitle: "Connected", color: Theme.accents.orange };
                    usbWait.restart();
                }
            } else if (p.ACTION === "remove") {
                const name = usbNames[p.DEVPATH];
                if (name && live)
                    Sounds.play("usbOff");
                if (name && live && Island.wants("usb") && !driveSeen)
                    Island.post({ kind: "usb", icon: "usb", title: name, subtitle: "Disconnected", color: Qt.rgba(1, 1, 1, 0.55) }, 2500);
                driveSeen = false;
            }
        } else if (p.SUBSYSTEM === "block" && p.ID_BUS === "usb" && (p.DEVTYPE === "partition" || (p.DEVTYPE === "disk" && p.ID_FS_TYPE))) {
            if (!live || !Island.wants("usb"))
                return;
            driveSeen = true;
            usbWait.stop();
            pendingUsb = null;
            const label = p.ID_FS_LABEL ? p.ID_FS_LABEL.replace(/\\x20/g, " ") : usbName(p);
            const dev = p.DEVNAME;
            // One alert per drive, not per partition (sdb1, sdb2… or nvme0n1p1…)
            const disk = (dev ?? "").replace(/p?\d+$/, "");
            const seenKey = p.ACTION + ":" + disk;
            if (Date.now() - (root.driveSeenAt[seenKey] ?? 0) < 8000)
                return;
            root.driveSeenAt = Object.assign({}, root.driveSeenAt, { [seenKey]: Date.now() });
            if (p.ACTION === "add")
                Island.post({
                    kind: "usb", icon: "hard-drive", title: label, subtitle: "Drive connected", color: Theme.accents.orange, action: "Open",
                    onAction: () => Launch.run(["sh", "-c", "m=$(findmnt -nro TARGET \"$1\" | head -1); [ -z \"$m\" ] && m=$(udisksctl mount -b \"$1\" 2>/dev/null | sed -n 's/.* at //p'); ${2:-xdg-open} \"${m:-$HOME}\"", "sh", dev, Files.manager])
                }, 6000);
            else if (p.ACTION === "remove")
                Island.post({ kind: "usb", icon: "hard-drive", title: label, subtitle: "Drive removed", color: Qt.rgba(1, 1, 1, 0.55) }, 2500);
        }
    }
    Timer {
        id: udevRetry
        interval: 10000
    }
    Timer {
        id: usbWait
        interval: 1200
        onTriggered: {
            if (root.pendingUsb && !root.driveSeen)
                Island.post(root.pendingUsb, 3000);
            root.pendingUsb = null;
        }
    }
    Process {
        id: udev
        running: Island.enabled && Tools.has("udevadm") && !udevRetry.running
        stdinEnabled: true
        command: [Tools.watcher, "udevadm", "monitor", "--udev", "--property", "--subsystem-match=usb", "--subsystem-match=block"]
        onExited: udevRetry.start()
        property var cur: ({})
        stdout: SplitParser {
            onRead: line => {
                const p = udev.cur;
                if (line.trim() === "") {
                    if (p.ACTION)
                        root.onUdev(p);
                    udev.cur = {};
                    return;
                }
                const eq = line.indexOf("=");
                if (eq > 0)
                    p[line.slice(0, eq)] = line.slice(eq + 1);
            }
        }
    }

    // ── Displays ────────────────────────────────────────────────────────
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (!root.live || !Island.wants("displays"))
                return;
            if (event.name === "monitoraddedv2" || event.name === "monitorremovedv2") {
                const [, name, ...desc] = event.data.split(",");
                if (name.startsWith("HEADLESS") || name.startsWith("IRICE-"))
                    return;
                const added = event.name === "monitoraddedv2";
                Island.post({ kind: "display", icon: name.startsWith("eDP") ? "desktop" : "monitor", title: desc.join(",").trim() || name, subtitle: added ? "Display connected · " + name : "Display disconnected", color: added ? Theme.accents.green : Qt.rgba(1, 1, 1, 0.55) }, 3500);
            }
        }
    }

    // ── Audio output ────────────────────────────────────────────────────
    // The default sink and its active port (wired headphones on the same
    // card only change the port), polled through pactl when PulseAudio events
    // arrive; without pactl, only default-sink changes are seen.
    property string outputKey: ""
    function outputIcon(text) {
        const t = text.toLowerCase();
        return t.includes("headphone") || t.includes("headset") || t.includes("bluez") || t.includes("airpods") || t.includes("buds") ? "headphones"
            : t.includes("hdmi") || t.includes("displayport") ? "television"
            : "speaker-hifi";
    }
    function outputChanged(key, title, detail) {
        if (key === outputKey)
            return;
        const first = outputKey === "";
        outputKey = key;
        if (first || !live || !Island.wants("audio"))
            return;
        if (key.includes("bluez") && Date.now() - lastBtAudio < 5000)
            return; // the Bluetooth alert already said it
        Island.post({ kind: "audio", icon: outputIcon(key + " " + title), title: title, subtitle: detail, color: Theme.accents.purple }, 3000);
    }
    Process {
        id: subscribe
        running: Island.enabled && Tools.has("pactl") && !pactlRetry.running
        stdinEnabled: true
        command: [Tools.watcher, "pactl", "subscribe"]
        onExited: pactlRetry.start()
        stdout: SplitParser {
            onRead: line => {
                if (line.includes("on sink") || line.includes("on server") || line.includes("on card"))
                    sinkPoll.restart();
            }
        }
    }
    Timer {
        id: pactlRetry
        interval: 10000 // the sound server restarted, or isn't up yet
    }
    Timer {
        id: sinkPoll
        interval: 350
        running: subscribe.running // once at start, to learn the current output
        onTriggered: sinkQuery.running = true
    }
    Process {
        id: sinkQuery
        command: ["sh", "-c", "pactl get-default-sink; pactl -f json list sinks"]
        stdout: StdioCollector {
            onStreamFinished: {
                const nl = text.indexOf("\n");
                const def = text.slice(0, nl).trim();
                try {
                    const sink = JSON.parse(text.slice(nl + 1)).find(s => s.name === def);
                    if (!sink)
                        return;
                    const port = (sink.ports ?? []).find(p => p.name === sink.active_port);
                    const title = sink.description || def;
                    root.outputChanged(def + "|" + (sink.active_port ?? ""), port && (sink.ports ?? []).length > 1 ? port.description : title, port && (sink.ports ?? []).length > 1 ? title : "Sound output");
                } catch (e) {}
            }
        }
    }
    // Fallback without pactl
    readonly property var sink: Pipewire.defaultAudioSink
    onSinkChanged: if (!Tools.has("pactl") && sink) outputChanged(sink.name, sink.description || sink.nickname || sink.name, "Sound output")
}
