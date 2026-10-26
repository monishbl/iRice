pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// Talks to the irice-glass Hyprland plugin (hypr/plugin/irice-glass), which
// draws Liquid Glass inside the compositor. `available` says whether it's
// loaded; when it isn't, glass falls back to Frosted (Hyprland's blur).
// Shapes go over Hyprland's request socket as `irice-glass set ...`: one
// request at a time, carrying every window's latest pending update together
// (a Hyprland batch), so an animation costs one request per frame. Requests go
// through a small helper (assets/scripts/glass-ipc.py), never a socket of our
// own: Hyprland blocks while waiting for a request on a connection it has
// accepted, and if the shell is waiting on Hyprland at that moment (creating
// a popup), both stall for Hyprland's 5-second timeout.
Singleton {
    id: root

    property bool available: false
    property int version: 0 // 2+: shapes carry gloss (a 19th number)
    readonly property string socketPath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/hypr/" + Quickshell.env("HYPRLAND_INSTANCE_SIGNATURE") + "/.socket.sock"

    // key "<monitor>\n<namespace>" -> command, waiting to be sent
    property var pending: ({})
    property var order: []
    property string inFlight: ""
    signal ready // answered a ping: (re)loaded plugins start blank, so resend state

    function update(monitor, ns, shapes) {
        const parts = [];
        for (const s of shapes)
            parts.push(s.map(v => (Math.round(v * 1000) / 1000).toString()).join(" "));
        command(monitor + "\n" + ns, "irice-glass set " + monitor + " " + ns + " " + shapes.length + (parts.length ? " " + parts.join(" ") : ""));
    }

    // Queue a request; a newer one with the same key replaces it
    function command(key, cmd) {
        const p = Object.assign({}, pending);
        if (p[key] === undefined)
            order = order.concat([key]);
        p[key] = cmd;
        pending = p;
        Qt.callLater(pump); // after this frame's other windows have queued theirs
    }

    function pump() {
        if (inFlight !== "" || order.length === 0 || !ipc.running)
            return; // (a restarted helper pumps when it's up)
        const cmds = order.map(k => pending[k]).filter(c => c !== undefined);
        order = [];
        pending = {};
        if (cmds.length === 1)
            send(cmds[0]);
        else if (cmds.length > 1)
            send("[[BATCH]]" + cmds.join(";"));
    }

    function finish() {
        watchdog.stop();
        inFlight = "";
        Qt.callLater(pump);
    }

    function send(cmd) {
        inFlight = cmd;
        watchdog.restart();
        ipc.write(cmd.replace(/\n/g, " ") + "\n");
    }
    // A request that never gets an answer mustn't block every later update
    Timer {
        id: watchdog
        interval: 2000
        onTriggered: {
            if (root.inFlight === "")
                return;
            // The helper is stuck: start it again (answers from the old one
            // can't arrive late, it's gone)
            root.inFlight = "";
            ipc.running = false;
            restart.restart();
        }
    }

    Process {
        id: ipc
        running: true
        stdinEnabled: true
        command: ["setpriv", "--pdeathsig", "TERM", "--", "python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/glass-ipc.py").toString().replace("file://", "")), root.socketPath]
        onStarted: Qt.callLater(root.pump)
        onExited: {
            root.inFlight = "";
            watchdog.stop();
            restart.restart();
        }
        stdout: SplitParser {
            splitMarker: "\x1e"
            onRead: data => {
                if (root.isPing) {
                    root.available = data.startsWith("irice-glass");
                    root.version = root.available ? (parseInt(data.split(" ")[1]) || 1) : 0;
                    if (root.available)
                        Qt.callLater(root.ready);
                } else if (!data.startsWith("ok") && !data.startsWith("error")) {
                    // Not the plugin answering (it was unloaded): fall back
                    // to Frosted until a ping finds it again
                    root.available = false;
                    root.version = 0;
                }
                root.isPing = false;
                root.finish(); // one answer per request
            }
        }
    }
    Timer {
        id: restart
        interval: 1000
        onTriggered: ipc.running = true
    }
    property bool isPing: false

    // Is the plugin there? Asked at start, after config reloads (plugins load
    // then), and now and then while it's missing.
    function ping() {
        if (inFlight !== "") {
            pingLater.restart(); // busy: try again shortly
            return;
        }
        isPing = true;
        send("irice-glass ping");
    }
    Timer {
        running: true
        interval: 1500
        onTriggered: root.ping()
    }
    Timer {
        running: !root.available
        repeat: true
        interval: 30000
        onTriggered: root.ping()
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "configreloaded")
                pingLater.restart();
        }
    }
    Timer {
        id: pingLater
        interval: 1000
        onTriggered: root.ping()
    }
}
