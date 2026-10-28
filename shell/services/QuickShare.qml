pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Optional Android Quick Share through Packet. Its window handles device
// selection, confirmation and progress; LocalSend remains independent.
Singleton {
    id: root
    property bool available: false
    property string error: ""
    readonly property bool busy: action.running
    readonly property bool checking: probe.running
    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/quick-share.py").toString().replace("file://", ""))

    Component.onCompleted: refresh()
    function refresh() {
        if (!probe.running) {
            probe.command = Launch.wrap(["python3", "-I", script, "--status"]);
            probe.running = true;
        }
    }
    function setup() {
        Launch.open("https://flathub.org/apps/io.github.nozwock.Packet");
    }
    function run(args) {
        if (busy)
            return;
        error = "";
        action.command = Launch.wrap(["python3", "-I", script].concat(args));
        action.running = true;
    }
    function open() { run(["--open"]); }
    function send(paths) { run(["--"].concat(paths)); }

    Process {
        id: probe
        stdout: StdioCollector { id: status }
        onExited: code => {
            try {
                root.available = code === 0 && JSON.parse(status.text).available === true;
            } catch (e) {
                root.available = false;
            }
        }
    }
    Process {
        id: action
        stderr: StdioCollector { id: failure }
        onExited: code => {
            if (code !== 0) {
                root.error = failure.text.trim() || "Couldn't open Quick Share.";
                Island.post({ kind: "custom", icon: "warning-circle", title: "Quick Share", subtitle: root.error, color: Theme.accents.orange }, 6000);
                root.refresh();
            }
        }
    }
}
