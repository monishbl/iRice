pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Where windows are, so glass can tell whether it floats over bare wallpaper
// (liquid refraction) or over windows (compositor-blurred frost). Also Show
// Desktop: the space's windows step aside, and come back on the next call.
Singleton {
    id: root

    // Bumped whenever window geometry may have changed
    property int revision: 0

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            // Leaving the space while the desktop shows brings the windows back
            if (root.showingDesktop && (event.name === "workspace" || event.name === "openwindow"))
                Qt.callLater(root.restoreDesktop);
            if (["openwindow", "closewindow", "movewindow", "movewindowv2", "changefloatingmode", "fullscreen", "workspace", "workspacev2", "activewindow", "resizewindow"].includes(event.name)) {
                Hyprland.refreshToplevels();
                refreshTimer.restart();
            }
        }
    }
    // Geometry arrives with the refresh; recheck shortly after
    Timer {
        id: refreshTimer
        interval: 120
        onTriggered: root.revision++
    }

    // Show Desktop: move the focused space's windows to a hidden special
    // workspace; calling again (or switching spaces) puts them back
    property var stashed: [] // [{ address, ws }]
    readonly property bool showingDesktop: stashed.length > 0
    function addr(t) {
        return t.address.startsWith("0x") ? t.address : "0x" + t.address;
    }
    function showDesktop() {
        if (showingDesktop)
            return restoreDesktop();
        const ws = Hyprland.focusedMonitor?.activeWorkspace?.id ?? 0;
        if (ws <= 0)
            return;
        const wins = Windows.on(ws);
        stashed = wins.map(t => ({ address: addr(t), ws: ws }));
        for (const s of stashed)
            Hyprland.dispatch("hl.dsp.window.move({ workspace = \"special:irice-desktop\", follow = false, window = \"address:" + s.address + "\" })");
    }
    function restoreDesktop() {
        const list = stashed;
        stashed = [];
        for (const s of list)
            Hyprland.dispatch("hl.dsp.window.move({ workspace = " + s.ws + ", follow = false, window = \"address:" + s.address + "\" })");
    }

    // True if the wallpaper is mostly hidden: a full-screen window, or windows
    // on the visible workspace covering at least 70% of the screen
    function mostlyCovered(screen) {
        revision; // dependency
        const mon = Hyprland.monitorFor(screen);
        if (!mon)
            return false;
        const ws = mon.activeWorkspace?.id;
        let area = 0;
        for (const t of Windows.on(ws)) {
            const o = t.lastIpcObject;
            if (!o?.at || o.hidden)
                continue;
            if (o.fullscreen > 0)
                return true;
            area += o.size[0] * o.size[1];
        }
        return area >= 0.7 * screen.width * screen.height;
    }

    // True if any window on the screen's visible workspace overlaps rect
    // (screen-local coordinates).
    function covered(screen, rect) {
        revision; // dependency
        const mon = Hyprland.monitorFor(screen);
        if (!mon)
            return false;
        const ws = mon.activeWorkspace?.id;
        for (const t of Windows.on(ws)) {
            const o = t.lastIpcObject;
            if (!o?.at || o.hidden)
                continue;
            const x = o.at[0] - mon.x, y = o.at[1] - mon.y;
            if (x < rect.x + rect.width && x + o.size[0] > rect.x && y < rect.y + rect.height && y + o.size[1] > rect.y)
                return true;
        }
        return false;
    }
}
