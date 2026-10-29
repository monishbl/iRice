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

    // A monitor's spaces: those living on it (and its active one), and on the
    // first screen also 1..workspaceCount unless another monitor has them
    // (ids are global in Hyprland)
    function spaceIds(monitor, primary) {
        const mine = new Set(), elsewhere = new Set();
        for (const w of Hyprland.workspaces.values) {
            if (w.id > 0)
                (w.monitor === monitor ? mine : elsewhere).add(w.id);
        }
        const active = monitor?.activeWorkspace?.id ?? 0;
        if (active > 0)
            mine.add(active);
        if (primary)
            for (let i = 1; i <= Config.options.menubar.workspaceCount; i++)
                if (!elsewhere.has(i))
                    mine.add(i);
        // No gaps: an empty space between two in use (5 and 7 with windows,
        // 6 empty) stays reachable
        const ids = [...mine];
        if (ids.length) {
            const from = primary ? 1 : Math.min(...ids), to = Math.max(...ids);
            for (let i = from; i < to; i++)
                if (!elsewhere.has(i))
                    mine.add(i);
        }
        return [...mine].sort((a, b) => a - b);
    }
}
