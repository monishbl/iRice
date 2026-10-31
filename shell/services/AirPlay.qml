pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pipewire
import qs.common

// AirPlay speakers (HomePod, Apple TV, AirPlay speakers) as outputs, with
// PipeWire's own AirPlay support (RAOP, the pipewire-zeroconf package): a
// small PipeWire client of its own (assets/pipewire/irice-airplay.conf) finds
// them over mDNS and adds each as an output. It runs as a user unit
// (irice-airplay, so it outlives a shell restart), or as the shell's own
// child without systemd. Off until Settings -> Sound turns it on.
Singleton {
    id: root

    readonly property bool wanted: Config.options.audio.airplay
    readonly property string conf: Quickshell.shellDir + "/assets/pipewire/irice-airplay.conf"
    readonly property string unit: "irice-airplay"

    property bool checked: false
    property bool installed: false // pipewire-zeroconf's raop modules
    property bool running: false
    property string error: ""

    // The speakers found (PipeWire outputs made by the raop module)
    readonly property var speakers: Pipewire.nodes.values.filter(n => n.isSink && !n.isStream && isAirPlay(n))
    function isAirPlay(node) {
        return String(node?.name ?? "").startsWith("raop_sink.");
    }

    function check() {
        checker.running = true;
    }
    Process {
        id: checker
        command: ["sh", "-c", "test -e /usr/lib/pipewire-0.3/libpipewire-module-raop-discover.so && echo installed; systemctl --user is-active --quiet \"$1\" 2>/dev/null && echo running; :", "sh", root.unit]
        stdout: StdioCollector {
            onStreamFinished: {
                root.installed = text.includes("installed");
                root.running = text.includes("running") || child.running;
                root.checked = true;
                root.apply();
            }
        }
    }
    Component.onCompleted: Startup.later(() => check()) // (after the desktop is up)
    onWantedChanged: apply()

    // On when wanted and installed, off otherwise
    function apply() {
        if (!checked)
            return;
        if (wanted && installed && !running && !starter.running)
            start();
        else if (!wanted && running)
            stop();
    }
    function start() {
        error = "";
        starter.running = true;
    }
    Process {
        id: starter
        // (100: no systemd user manager. A unit left over, restarting or
        // stopping, is cleared first: one copy only)
        command: ["sh", "-c", "systemctl --user show-environment >/dev/null 2>&1 || exit 100; "
            + "systemctl --user stop \"$1\" 2>/dev/null; systemctl --user reset-failed \"$1\" 2>/dev/null; "
            + "exec systemd-run --user --quiet --collect --unit=\"$1\" -p Restart=on-failure -p RestartSec=5 pipewire -c \"$2\"", "sh", root.unit, root.conf]
        stderr: StdioCollector { id: starterErr }
        onExited: code => {
            if (code === 0) {
                root.running = true;
            } else if (code === 100) {
                // (the shell runs it itself)
                child.running = true;
                root.running = true;
            } else {
                root.error = starterErr.text.trim().split("\n").pop() || "AirPlay couldn't start.";
            }
        }
    }
    Process {
        id: child
        command: [Tools.watcher, "pipewire", "-c", root.conf]
        stdinEnabled: true // (Tools.watcher: it ends with the shell)
        property string lastErr: ""
        stderr: SplitParser { onRead: line => child.lastErr = line } // (only the last: it runs for hours)
        onExited: code => {
            if (root.wanted && code !== 0)
                root.error = (child.lastErr.trim() || "AirPlay stopped.").replace(/^\[[^\]]*\]\[[^\]]*\]\s*/, "");
            child.lastErr = "";
            root.running = false;
        }
    }
    function stop() {
        if (child.running)
            child.signal(15);
        Quickshell.execDetached(["systemctl", "--user", "stop", unit]);
        running = false;
    }

    // ── Installing pipewire-zeroconf ────────────────────────────────────
    readonly property bool installing: Updates.running && installAsked
    property bool installAsked: false
    property string installError: ""
    function install() {
        if (Updates.running || installed)
            return;
        installAsked = true;
        installError = "";
        Updates.update(["--install", "pipewire-zeroconf"]);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (!root.installAsked)
                return;
            root.installAsked = false;
            root.installError = j.state === "done" ? "" : (j.error || j.text || "The install didn't finish.");
            root.check();
        }
    }
}
