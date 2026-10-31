pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Io
import qs.common

// Android's Quick Share (Nearby Share), built in: iRice speaks the protocol
// itself (assets/scripts/quickshare, see its nearby/__init__.py), so phones
// send to this computer and it sends to them from their own Quick Share
// sheet, with nothing else installed. Like LocalSend: you're asked in the
// island before anything is saved (with the PIN the phone shows too), and
// transfers show as live activities.
// Visibility (Settings → Sharing): Off | Everyone for 10 minutes | Everyone,
// as on Android. On the same network (Wi-Fi or Ethernet) it goes over that;
// without one, phones find it over Bluetooth LE and the transfer moves to a
// Wi-Fi link: through NetworkManager without a password (this computer's
// Wi-Fi pauses for the transfer: a hotspot of its own, or the phone's group;
// Settings → Sharing can turn that off), else Wi-Fi Direct through the root
// helper (a password prompt), else it stays on slow Bluetooth with a warning. Bluetooth is never turned on from here except by the
// user's own "Turn on Bluetooth".
// The helper only runs while visible, while the share sheet looks for
// devices, or while a transfer is going.
Singleton {
    id: root

    readonly property var cfg: Config.options.sharing
    readonly property bool available: Tools.has("python3")
    property real everyoneUntil: 0 // "Everyone for 10 minutes" turns itself off
    readonly property bool visible: cfg.quickShare === "everyone" || (cfg.quickShare === "10min" && everyoneUntil > Date.now())
    Connections {
        target: root.cfg
        function onQuickShareChanged() {
            if (root.cfg.quickShare === "10min")
                root.everyoneUntil = Date.now() + 10 * 60000;
        }
        function onQuickShareHotspotChanged() {
            root.configure();
        }
    }
    Timer {
        running: root.cfg.quickShare === "10min"
        interval: Math.max(1000, root.everyoneUntil - Date.now())
        onTriggered: Config.options.sharing.quickShare = "off"
    }
    // One name for this computer, LocalSend's too (Settings → Sharing → Your name)
    readonly property string name: cfg.name || (Quickshell.env("USER") ?? "iRice") + "'s computer"
    readonly property string folder: cfg.quickShareFolder || ""

    property bool looking: false // the share sheet is open: find devices
    property var devices: [] // [{ id, name, type }]
    property var active: ({}) // session -> transfer item (Transfers shows them)
    readonly property var items: Object.values(active)
    property bool sendPending: false // asked to send, no word back yet
    Timer {
        running: root.sendPending
        interval: 180000
        onTriggered: root.sendPending = false
    }
    property string error: ""
    // Bluetooth: phones without a shared network find this computer through it
    readonly property var btAdapter: Bluetooth.defaultAdapter
    readonly property bool bluetoothAvailable: !!btAdapter
    readonly property bool bluetoothOn: btAdapter?.enabled ?? false
    readonly property bool bluetoothNeeded: bluetoothAvailable && !bluetoothOn && (cfg.quickShare !== "off" || looking)
    property bool bluetoothEngine: false // the helper could reach BlueZ (python-gobject, bluetoothd)
    function turnOnBluetooth() { // (the user's button only)
        if (btAdapter)
            btAdapter.enabled = true;
    }
    // session -> "Wi-Fi" | "Wi-Fi Direct" | "Bluetooth": the link it's on now
    property var transports: ({})
    function transportOf(session) {
        return transports[session] ?? "";
    }
    readonly property bool needed: available && (visible || looking || sendPending || items.length > 0)
    readonly property bool running: helper.running

    function tell(obj) {
        if (helper.running)
            helper.write(JSON.stringify(obj) + "\n");
    }
    function configure() {
        tell({ cmd: "config", visible: visible, name: name, folder: folder || "~/Downloads", pause_wifi: cfg.quickShareHotspot !== false });
    }
    onVisibleChanged: configure()
    onNameChanged: configure()
    onFolderChanged: configure()
    onLookingChanged: tell({ cmd: "discover", on: looking })

    property string sendingName: ""
    function send(deviceId, paths) {
        const first = paths[0]?.split("/").pop() ?? "";
        sendingName = paths.length > 1 ? first + " and " + (paths.length - 1) + " more" : first;
        sendPending = true;
        tell({ cmd: "send", to: deviceId, files: paths });
    }
    function cancel(session) {
        tell({ cmd: "cancel", session: session });
        finish(session);
    }
    function track(ev) {
        const prev = active[ev.session];
        const now = Date.now();
        const incoming = ev.dir === "in";
        const speed = prev && now > prev.at ? Math.max(0, (ev.done - prev.done) * 1000 / (now - prev.at)) : 0;
        active = Object.assign({}, active, { [ev.session]: {
            id: ev.session, kind: incoming ? "receive" : "send", via: "quickshare",
            name: prev?.name ?? (incoming ? ev.name : sendingName), peer: incoming ? (prev?.peer ?? ev.name) : ev.name,
            done: ev.done, total: ev.total, speed: speed, at: now, waiting: !!ev.waiting, transport: transportOf(ev.session)
        } });
    }
    function finish(session) {
        const a = Object.assign({}, active);
        delete a[session];
        active = a;
        const t = Object.assign({}, transports);
        delete t[session];
        transports = t;
    }
    // "photo.jpg", "3 photos", "a link and 2 files"…
    function describe(ev) {
        const parts = [];
        const files = ev.files ?? [];
        if (files.length === 1)
            parts.push(files[0].name);
        else if (files.length > 1)
            parts.push(files.length + (files.every(f => (f.mime ?? "").startsWith("image/")) ? " photos" : " files"));
        for (const t of ev.texts ?? [])
            parts.push(t.kind === "url" ? "a link" : t.kind === "phone" ? "a phone number" : t.kind === "address" ? "an address" : "some text");
        for (const w of ev.wifi ?? [])
            parts.push("the Wi-Fi network " + w);
        return parts.length <= 1 ? (parts[0] ?? "something") : parts.slice(0, -1).join(", ") + " and " + parts[parts.length - 1];
    }

    // Started and stopped by hand, not bound to `needed`: a process that
    // exits by itself (a missing library, a crash) clears `running`, which
    // broke such a binding, and the helper never started again until the
    // shell restarted. If it exits while still needed, it's tried again a
    // few seconds later (then less often).
    onNeededChanged: sync()
    Component.onCompleted: sync()
    function sync() {
        if (needed && !helper.running)
            helper.running = true;
        else if (!needed && helper.running)
            helper.running = false;
    }
    property int retries: 0
    Timer {
        id: retry
        interval: Math.min(60000, 3000 * Math.pow(2, root.retries))
        onTriggered: {
            root.retries++;
            root.sync();
        }
    }
    Process {
        id: helper
        stdinEnabled: true
        // (The helper ends when its stdin closes: shell quit or restarted, or
        // no longer needed; pdeathsig covers the shell being killed outright)
        command: ["setpriv", "--pdeathsig", "TERM", "--", "python3", "-I",
            decodeURIComponent(Qt.resolvedUrl("../assets/scripts/quickshare/qs-daemon.py").toString().replace("file://", ""))]
        onRunningChanged: {
            if (running) {
                root.error = "";
                root.configure();
                if (root.looking)
                    root.tell({ cmd: "discover", on: true });
            } else {
                root.devices = [];
                root.active = {};
                root.transports = {};
                root.bluetoothEngine = false;
                if (root.needed)
                    retry.restart();
            }
        }
        stdout: SplitParser {
            onRead: line => {
                let ev;
                try {
                    ev = JSON.parse(line);
                } catch (e) {
                    return;
                }
                switch (ev.ev) {
                case "status":
                    root.retries = 0; // up and answering
                    break;
                case "devices":
                    root.devices = ev.devices;
                    break;
                case "incoming": {
                    const what = root.describe(ev);
                    root.active = Object.assign({}, root.active, { [ev.session]: { id: ev.session, kind: "receive", via: "quickshare", name: what, peer: ev.from, done: 0, total: ev.total, speed: 0, at: Date.now(), waiting: true } });
                    Island.post({
                        kind: "custom", icon: "share-network", title: ev.from,
                        subtitle: "Wants to share " + what + (ev.total > 0 ? " · " + Transfers.size(ev.total) : "") + " · PIN " + ev.pin,
                        color: Theme.accents.blue,
                        action: "Accept", onAction: () => root.tell({ cmd: "answer", session: ev.session, accept: true }),
                        action2: "Decline", onAction2: () => {
                            root.tell({ cmd: "answer", session: ev.session, accept: false });
                            root.finish(ev.session);
                        }
                    }, 60000);
                    break;
                }
                case "connected":
                    // The PIN the phone shows too, while it decides
                    Island.post({ kind: "custom", icon: "share-network", title: "Waiting for " + ev.to, subtitle: "PIN " + ev.pin + " · they're asked to accept", color: Theme.accents.blue }, 8000);
                    break;
                case "progress":
                    root.track(ev);
                    if (ev.dir === "out")
                        root.sendPending = false;
                    break;
                case "finished":
                    root.finish(ev.session);
                    if (ev.dir === "in")
                        root.received(ev);
                    else
                        Island.post({ kind: "custom", icon: "paper-plane-tilt", title: "Sent to " + ev.to, subtitle: ev.paths.length === 1 ? ev.paths[0].split("/").pop() : ev.paths.length + " files", color: Theme.accents.green }, 3000);
                    break;
                case "declined":
                    root.sendPending = false;
                    root.finish(ev.session);
                    Island.post({ kind: "custom", icon: "x-circle", title: ev.reason === "declined" ? ev.to + " declined" : "Not sent to " + ev.to, subtitle: ev.reason === "declined" ? "Nothing was sent" : ev.reason.charAt(0).toUpperCase() + ev.reason.slice(1), color: Qt.rgba(1, 1, 1, 0.6) }, 4000);
                    break;
                case "ended":
                    root.finish(ev.session);
                    break;
                case "failed":
                    if (String(ev.session).startsWith("out-"))
                        root.sendPending = false;
                    root.finish(ev.session);
                    if (ev.message !== "Cancelled")
                        Island.post({ kind: "custom", icon: "warning-circle", title: "Quick Share stopped", subtitle: ev.message, color: Theme.accents.orange }, 6000);
                    break;
                case "error":
                    root.error = ev.message;
                    break;
                case "bluetooth":
                    root.bluetoothEngine = ev.available && ev.powered;
                    break;
                case "transport":
                    root.transports = Object.assign({}, root.transports, { [ev.session]: ev.name });
                    break;
                case "warning": // a transfer stays on slow Bluetooth: why, and how to fix it
                    Island.post({ kind: "custom", icon: "warning-circle", title: "Slow transfer", subtitle: ev.message, color: Theme.accents.orange }, 8000);
                    break;
                case "note": // this computer's Wi-Fi pauses for a fast link
                    Island.post({ kind: "custom", icon: "wifi-high", title: "Switching to Wi-Fi", subtitle: ev.message, color: Theme.accents.blue }, 5000);
                    break;
                case "auth": // pkexec is about to ask for the password: say what for
                    Island.post({ kind: "custom", icon: "lock", title: "Password for a faster link", subtitle: ev.message, color: Theme.accents.blue }, 10000);
                    break;
                }
            }
        }
    }

    // What arrived: files (Show), a link (Open), text (Copy), a Wi-Fi network
    // (Copy password; joining is up to you)
    function received(ev) {
        Sounds.play("received");
        const paths = ev.paths ?? [];
        const texts = ev.texts ?? [];
        const wifi = ev.wifi ?? [];
        if (paths.length > 0) {
            const first = paths[0];
            const where = root.folder ? root.folder.split("/").pop() : "Downloads";
            Island.post({ kind: "custom", icon: "check-circle", title: "Received from " + ev.from,
                subtitle: paths.length === 1 ? first.split("/").pop() : paths.length + " files in " + where, color: Theme.accents.green,
                action: paths.length === 1 ? "Open" : "Show in Folder", onAction: () => paths.length === 1 ? Qt.openUrlExternally("file://" + first) : Files.reveal(first),
                action2: paths.length === 1 ? "Show in Folder" : "", onAction2: () => Files.reveal(first) }, 8000);
        }
        for (const t of texts) {
            const link = t.kind === "url" && /^https?:\/\//i.test(t.text);
            Island.post({ kind: "custom", icon: link ? "link" : "text-aa", title: (link ? "Link from " : "Text from ") + ev.from,
                subtitle: t.text.length > 80 ? t.text.slice(0, 79) + "…" : t.text, color: Theme.accents.green,
                action: link ? "Open" : "Copy", onAction: () => link ? Qt.openUrlExternally(t.text) : Quickshell.execDetached(["wl-copy", "--", t.text]),
                action2: link ? "Copy" : "", onAction2: () => Quickshell.execDetached(["wl-copy", "--", t.text]) }, 10000);
        }
        for (const w of wifi)
            Island.post({ kind: "custom", icon: "wifi-high", title: "Wi-Fi network from " + ev.from, subtitle: w.ssid, color: Theme.accents.green,
                action: "Copy Password", onAction: () => Quickshell.execDetached(["wl-copy", "--", w.password]) }, 10000);
    }
}
