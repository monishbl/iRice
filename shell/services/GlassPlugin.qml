pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// Talks to the irice-glass Hyprland plugin (hypr/plugin/irice-glass), which
// draws Liquid Glass inside the compositor. `available` says whether it's
// loaded; when it isn't, glass falls back to Frosted (Hyprland's blur).
// Shapes go over Hyprland's request socket as `irice-glass set ...`, one
// request at a time; only the latest pending update per window is kept.
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
        pump();
    }

    function pump() {
        if (inFlight !== "" || order.length === 0)
            return;
        const key = order[0];
        order = order.slice(1);
        const cmd = pending[key];
        const p = Object.assign({}, pending);
        delete p[key];
        pending = p;
        if (cmd === undefined)
            return pump();
        send(cmd);
    }

    function finish() {
        if (sock.connected)
            sock.connected = false; // the state change continues with pump()
        else if (inFlight !== "") {
            inFlight = "";
            pump();
        }
    }

    function send(cmd) {
        inFlight = cmd;
        sock.connected = true;
    }

    Socket {
        id: sock
        path: root.socketPath
        onConnectionStateChanged: {
            if (connected) {
                write(root.inFlight);
            } else if (root.inFlight !== "") {
                root.inFlight = "";
                Qt.callLater(root.pump); // reconnect outside this callback
            }
        }
        // Never change `connected` inside the socket's own callbacks (Qt is
        // still using the socket there; Quickshell crashes). Hyprland closes
        // the connection after answering, which also arrives as an error.
        onError: if (!connected) Qt.callLater(root.finish)
        parser: SplitParser {
            splitMarker: ""
            onRead: data => {
                if (sock.isPing) {
                    root.available = data.startsWith("irice-glass");
                    root.version = root.available ? (parseInt(data.split(" ")[1]) || 1) : 0;
                    if (root.available)
                        Qt.callLater(root.ready);
                }
                sock.isPing = false;
                Qt.callLater(root.finish); // one answer per connection
            }
        }
        property bool isPing: false
    }

    // Is the plugin there? Asked at start, after config reloads (plugins load
    // then), and now and then while it's missing.
    function ping() {
        if (inFlight !== "") {
            pingLater.restart(); // busy: try again shortly
            return;
        }
        sock.isPing = true;
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
