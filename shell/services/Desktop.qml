pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Where windows are, so glass can tell whether it floats over bare wallpaper
// (liquid refraction) or over windows (compositor-blurred frost).
Singleton {
    id: root

    // Bumped whenever window geometry may have changed
    property int revision: 0

    Connections {
        target: Hyprland
        function onRawEvent(event) {
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

    // True if the wallpaper is mostly hidden: a full-screen window, or windows
    // on the visible workspace covering at least 70% of the screen
    function mostlyCovered(screen) {
        revision; // dependency
        const mon = Hyprland.monitorFor(screen);
        if (!mon)
            return false;
        const ws = mon.activeWorkspace?.id;
        let area = 0;
        for (const t of Hyprland.toplevels.values) {
            const o = t.lastIpcObject;
            if (!o?.at || t.workspace?.id !== ws || o.hidden)
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
        for (const t of Hyprland.toplevels.values) {
            const o = t.lastIpcObject;
            if (!o?.at || t.workspace?.id !== ws || o.hidden)
                continue;
            const x = o.at[0] - mon.x, y = o.at[1] - mon.y;
            if (x < rect.x + rect.width && x + o.size[0] > rect.x && y < rect.y + rect.height && y + o.size[1] > rect.y)
                return true;
        }
        return false;
    }
}
