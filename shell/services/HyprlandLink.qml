pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// Keeps the shell's view of Hyprland live. What the shell knows about
// windows, spaces and its own panels comes over Hyprland's event socket.
// Hyprland drops a listener that falls behind (seen on a real session: a busy
// moment, then "Hyprland event socket error: PeerClosedError"), and
// Quickshell doesn't reconnect, so the focused app, the spaces and the
// panels' state went stale until a restart: App Exposé showed the wrong app,
// Quick Settings' pages stayed blank.
// After a quiet spell this asks Hyprland directly which space and window are
// active. If that differs from what the events said, events were missed: the
// state is read again at once, and the shell is replaced by a fresh one
// (repair-shell.sh, which reconnects; at most once every few minutes, never
// in the sandbox).
Singleton {
    id: root

    property bool linked: true
    property real lastEvent: Date.now()
    property real lastRepair: 0
    readonly property int quiet: 30000 // no events for this long: check
    readonly property int repairEvery: 300000

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            root.lastEvent = Date.now();
            root.linked = true;
        }
    }

    Timer {
        interval: 20000
        running: true
        repeat: true
        onTriggered: {
            if (Date.now() - root.lastEvent > root.quiet && !probe.running)
                probe.running = true;
        }
    }

    // "0x55e0…" and "55e0…" alike
    function addr(a) {
        return String(a ?? "").replace(/^0x/, "").replace(/^0+/, "");
    }

    Process {
        id: probe
        command: ["hyprctl", "-j", "activeworkspace"]
        stdout: StdioCollector {
            onStreamFinished: {
                let ws;
                try {
                    ws = JSON.parse(text);
                } catch (e) {
                    return; // (no answer: Hyprland is busy or going; nothing to compare)
                }
                const sameSpace = (Hyprland.focusedWorkspace?.id ?? null) === ws.id;
                const lastWindow = root.addr(ws.lastwindow);
                const sameWindow = lastWindow === "" || root.addr(Hyprland.activeToplevel?.address) === lastWindow;
                if (sameSpace && sameWindow)
                    return;
                root.linked = false;
                console.warn("HyprlandLink: events were missed (space " + (Hyprland.focusedWorkspace?.id ?? "none")
                    + " vs " + ws.id + "): reading the state again" + (root.canRepair() ? " and restarting the shell" : ""));
                Hyprland.refreshMonitors();
                Hyprland.refreshWorkspaces();
                Hyprland.refreshToplevels();
                if (root.canRepair()) {
                    root.lastRepair = Date.now();
                    Quickshell.execDetached(["setsid", "-f", Quickshell.shellDir + "/assets/scripts/repair-shell.sh"]);
                }
            }
        }
    }

    function canRepair() {
        return Quickshell.env("IRICE_SANDBOX") !== "1" && Date.now() - lastRepair > repairEvery;
    }
}
