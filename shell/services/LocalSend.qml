pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Sharing with nearby devices, like AirDrop, through the LocalSend protocol
// (phones and computers running LocalSend, on the same network). The work is
// done by assets/scripts/localsend.py; this keeps the list of nearby
// devices, asks in the island before accepting files (they go to
// Downloads), and shows transfers as a live activity.
// Receiving (Settings → Sharing): Off | Everyone for 10 minutes | Everyone.
// The helper only runs while receiving, or while you're sharing something.
Singleton {
    id: root

    readonly property var cfg: Config.options.sharing
    readonly property bool available: Tools.has("python3")
    // "Everyone for 10 minutes" turns itself off
    property real everyoneUntil: 0
    readonly property bool receiving: cfg.receiving === "everyone" || (cfg.receiving === "10min" && everyoneUntil > Date.now())
    onReceivingChanged: tell({ cmd: "mode", mode: receiving ? "everyone" : "off", alias: alias })
    Connections {
        target: root.cfg
        function onReceivingChanged() {
            if (root.cfg.receiving === "10min")
                root.everyoneUntil = Date.now() + 10 * 60000;
        }
    }
    Timer {
        running: root.cfg.receiving === "10min"
        interval: Math.max(1000, root.everyoneUntil - Date.now())
        onTriggered: Config.options.sharing.receiving = "off"
    }
    onAliasChanged: tell({ cmd: "mode", mode: receiving ? "everyone" : "off", alias: alias })
    readonly property string alias: cfg.name || (Quickshell.env("USER") ?? "iRice") + "'s computer"

    property bool sharing: false // the share sheet is open or a send is running
    property var peers: [] // [{ id, alias, type, model }]
    property var active: ({}) // session -> transfer item
    readonly property var items: Object.values(active)

    // A send was asked for and hasn't reported back yet (the sheet may be gone)
    property bool sendPending: false
    Timer {
        running: root.sendPending
        interval: 180000 // in case nothing ever answers
        onTriggered: root.sendPending = false
    }
    readonly property bool needed: available && (receiving || sharing || sendPending || items.length > 0)

    function tell(obj) {
        if (helper.running)
            helper.write(JSON.stringify(obj) + "\n");
    }
    function scan() {
        tell({ cmd: "scan" });
    }
    property string sendingName: "" // what the next outgoing transfer is called
    function send(peerId, paths) {
        const first = paths[0]?.split("/").pop() ?? "";
        sendingName = paths.length > 1 ? first + " and " + (paths.length - 1) + " more" : first;
        sendPending = true;
        tell({ cmd: "send", to: peerId, files: paths });
    }
    function cancel(session) {
        tell({ cmd: "cancel", session: session });
        const a = Object.assign({}, active);
        delete a[session];
        active = a;
    }
    // A progress report: incoming keeps the name and sender from its prompt;
    // outgoing is named after what's being sent (ev.name is the device)
    function track(ev) {
        const prev = active[ev.session];
        const now = Date.now();
        const incoming = ev.dir === "in";
        const speed = prev && now > prev.at ? Math.max(0, (ev.done - prev.done) * 1000 / (now - prev.at)) : 0;
        active = Object.assign({}, active, { [ev.session]: {
            id: ev.session, kind: incoming ? "receive" : "send",
            name: prev?.name ?? (incoming ? ev.name : sendingName), peer: incoming ? (prev?.peer ?? "") : ev.name,
            done: ev.done, total: ev.total, speed: speed, at: now, waiting: !!ev.waiting
        } });
    }
    function finish(session) {
        const a = Object.assign({}, active);
        delete a[session];
        active = a;
    }
    function size(n) {
        return Transfers.size(n);
    }

    Process {
        id: helper
        running: root.needed
        stdinEnabled: true
        command: ["python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/localsend.py").toString().replace("file://", ""))]
        onRunningChanged: {
            if (running) {
                root.tell({ cmd: "mode", mode: root.receiving ? "everyone" : "off", alias: root.alias });
                root.scan();
            } else {
                root.peers = [];
                root.active = {};
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
                case "peers":
                    root.peers = ev.peers;
                    break;
                case "incoming": {
                    const n = ev.files.length;
                    const what = n === 1 ? ev.files[0].name : ev.files[0].name + " and " + (n - 1) + " more";
                    root.active = Object.assign({}, root.active, { [ev.session]: { id: ev.session, kind: "receive", name: what, peer: ev.from, done: 0, total: ev.total, speed: 0, at: Date.now(), waiting: true } });
                    Island.post({
                        kind: "custom", icon: "paper-plane-tilt", title: ev.from, subtitle: "Wants to share " + what + " · " + root.size(ev.total), color: Theme.accents.blue,
                        action: "Accept", onAction: () => root.tell({ cmd: "answer", session: ev.session, accept: true }),
                        action2: "Decline", onAction2: () => {
                            root.tell({ cmd: "answer", session: ev.session, accept: false });
                            root.finish(ev.session);
                        }
                    }, 60000);
                    break;
                }
                case "progress":
                    root.track(ev);
                    if (ev.dir === "out")
                        root.sendPending = false; // tracked as an item now (after, or the helper would stop in between)
                    break;
                case "finished":
                    root.finish(ev.session);
                    if (ev.dir === "in") {
                        const first = ev.paths[0] ?? "";
                        Island.post({ kind: "custom", icon: "check-circle", title: "Received from " + ev.from, subtitle: ev.paths.length === 1 ? first.split("/").pop() : ev.paths.length + " files in Downloads", color: Theme.accents.green,
                            action: "Show", onAction: () => ev.paths.length === 1 ? Panels.quickLook(first) : Transfers.openDownloads() }, 6000);
                        Sounds.play("received");
                    } else {
                        Island.post({ kind: "custom", icon: "paper-plane-tilt", title: "Sent to " + (ev.to ?? "device"), subtitle: ev.paths.length === 1 ? ev.paths[0].split("/").pop() : ev.paths.length + " files", color: Theme.accents.green }, 3000);
                    }
                    break;
                case "declined":
                    root.sendPending = false;
                    root.finish(ev.session);
                    Island.post({ kind: "custom", icon: "x-circle", title: (ev.to ?? "They") + " declined", subtitle: "Nothing was sent", color: Qt.rgba(1, 1, 1, 0.6) }, 3000);
                    break;
                case "failed":
                    if (String(ev.session).startsWith("out-"))
                        root.sendPending = false;
                    root.finish(ev.session);
                    if (ev.message !== "Cancelled")
                        Island.post({ kind: "custom", icon: "warning-circle", title: "Sharing stopped", subtitle: ev.message, color: Theme.accents.orange }, 5000);
                    break;
                case "error":
                    root.error = ev.message;
                    break;
                }
            }
        }
    }
    property string error: ""
    // While looking for devices, ask again now and then
    Timer {
        running: root.sharing && helper.running
        interval: 4000
        repeat: true
        onTriggered: root.scan()
    }
}
