pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common

// Hyprland's windows grouped once for every view that shows them (spaces,
// Overview, Mission Control, the dock's minimized items): regrouped when a
// window opens, closes or moves, instead of each view scanning all windows.
Singleton {
    id: root

    // workspace id -> [HyprlandToplevel]
    readonly property var byWorkspace: {
        const m = {};
        for (const t of Hyprland.toplevels.values) {
            const id = t.workspace?.id ?? 0;
            (m[id] = m[id] ?? []).push(t);
        }
        return m;
    }
    // Ids of the regular (not special) workspaces with windows
    readonly property var occupiedIds: Object.keys(byWorkspace).map(Number).filter(id => id > 0)
    readonly property var minimized: Hyprland.toplevels.values.filter(t => t.workspace?.name === "special:minimized")

    function on(wsId) {
        return byWorkspace[wsId] ?? [];
    }
}
