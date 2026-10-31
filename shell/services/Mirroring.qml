pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Mirror the screen to a TV (assets/scripts/screen-mirror.py, through GNOME
// Network Displays): Miracast TVs and sticks, Windows' Miracast over the
// network, and Chromecasts. Control Center's Screen Mirroring tile and
// Settings → Displays pick one; the island says while it's going.
// TVs are looked for only while something is showing them, or a mirror runs.
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/screen-mirror.py"
    readonly property bool available: Tools.has("gnome-network-displays-daemon")

    property var displays: [] // [{ uuid, name, protocol, state }]
    property bool p2p: true
    property string error: ""
    property int lookers: 0
    function look(on) {
        lookers = Math.max(0, lookers + (on ? 1 : -1));
    }
    readonly property bool looking: available && (lookers > 0 || stream !== "")

    Process {
        id: watcher
        running: root.looking
        command: [Tools.watcher, "python3", "-I", root.script, "watch"]
        stdinEnabled: true // (Tools.watcher: it, and the daemon it started, end with it)
        stdout: SplitParser {
            onRead: line => {
                try {
                    const m = JSON.parse(line);
                    root.displays = m.displays ?? [];
                    root.p2p = m.p2p ?? true;
                    if (m.error)
                        root.error = m.error;
                } catch (e) {}
            }
        }
    }

    // The mirror: which display, and its stream (a user unit)
    property string stream: ""
    property string target: "" // uuid
    readonly property var current: displays.find(d => d.uuid === target) ?? null
    readonly property bool active: stream !== ""
    readonly property string targetName: current?.name ?? lastName
    property string lastName: ""
    readonly property string state_: current?.state ?? (active ? "connecting" : "")
    onState_Changed: if (state_ === "error" && active) {
        Island.post({ kind: "custom", icon: "screencast", title: "Couldn't mirror to " + targetName, subtitle: !p2p && current?.protocol === "miracast" ? "This computer's Wi-Fi can't do Wi-Fi Direct, which Miracast needs." : "Check the TV is on and showing its screen-mirroring page, then try again.", color: Theme.color.danger }, 8000);
        stop();
    }

    function start(d) {
        if (active)
            stop();
        error = "";
        target = d.uuid;
        lastName = d.name;
        starter.command = ["python3", "-I", script, "start", d.uuid];
        starter.running = true;
    }
    Process {
        id: starter
        stdout: StdioCollector {
            onStreamFinished: {
                let r = {};
                try {
                    r = JSON.parse(text);
                } catch (e) {}
                if (r.ok) {
                    root.stream = r.stream;
                } else {
                    root.target = "";
                    root.error = r.error || "The TV didn't answer.";
                }
            }
        }
    }
    function stop() {
        if (stream !== "")
            Quickshell.execDetached(["python3", "-I", script, "stop", stream]);
        stream = "";
        target = "";
    }

    function protocolName(p) {
        return p === "chromecast" ? "Chromecast" : p === "mice" ? "Miracast (network)" : "Miracast";
    }

    // gnome-network-displays, through Software Update's job
    readonly property bool installing: Updates.running && installAsked
    property bool installAsked: false
    function install() {
        if (Updates.running)
            return;
        installAsked = true;
        Updates.update(["--install", "gnome-network-displays"]);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (root.installAsked) {
                root.installAsked = false;
                Tools.recheck();
            }
        }
    }

    // qs ipc call mirroring stop
    IpcHandler {
        target: "mirroring"
        function stop(): void {
            root.stop();
        }
    }
}
