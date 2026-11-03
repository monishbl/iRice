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
// After a quiet spell this asks Hyprland directly which space is active. If
// that differs from what the events said, events were missed: the state is
// read again at once. Only if it has drifted again by the next check (the
// feed really is gone) is the shell replaced by a fresh one (repair-shell.sh,
// which reconnects): at most once every ten minutes, counted across restarts
// (a stamp in the runtime folder), never in the sandbox. (Comparing windows
// too restarted the shell every minute: the space's last window isn't the
// focused one when nothing is focused.)
Singleton {
    id: root

    property bool linked: true
    property real lastEvent: Date.now()
    property bool drifted: false // the last check found a mismatch (and re-read the state)
    property bool asking: false // (the stamp's answers count only when a check asked)
    readonly property int quiet: 30000 // no events for this long: check
    readonly property int repairEvery: 600000
    readonly property string stamp: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-hyprland-link-repair"

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            root.lastEvent = Date.now();
            root.linked = true;
            root.drifted = false;
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
                if ((Hyprland.focusedWorkspace?.id ?? null) === ws.id)
                    return;
                Hyprland.refreshMonitors();
                Hyprland.refreshWorkspaces();
                Hyprland.refreshToplevels();
                if (!root.drifted) {
                    root.drifted = true; // (read again; a second drift means the feed is gone)
                    console.warn("HyprlandLink: space " + ws.id + " was missed: reading the state again");
                    return;
                }
                root.linked = false;
                root.asking = true;
                stampFile.reload();
            }
        }
    }

    // When this session last replaced the shell for this (kept across the
    // restarts it causes)
    FileView {
        id: stampFile
        path: root.stamp
        blockLoading: false
        onLoaded: root.repairIfDue(parseFloat(text()) || 0)
        onLoadFailed: root.repairIfDue(0)
    }
    function repairIfDue(last) {
        if (!asking)
            return; // (the stamp loading as the shell starts)
        asking = false;
        if (Quickshell.env("IRICE_SANDBOX") === "1" || Date.now() - last < repairEvery)
            return;
        console.warn("HyprlandLink: Hyprland's events stopped arriving: restarting the shell");
        stampFile.setText(String(Date.now()));
        Quickshell.execDetached(["setsid", "-f", Quickshell.shellDir + "/assets/scripts/repair-shell.sh"]);
    }
}
