pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common

// Each space remembers the display it belongs on. Unplugging a display moves
// its spaces to the others (Hyprland does that); plugging it back in returns
// them. A space's home is where it was made, or where you moved it (menu bar:
// right-click a space). Kept in state.json, so it survives restarts.
Singleton {
    id: root

    readonly property var homes: UiState.data.workspaceHomes ?? ({}) // id -> display name
    property real removedAt: 0 // moves right after a display goes are Hyprland's, not yours

    function setHome(id, mon) {
        if (!(id > 0) || !mon || homes[id] === mon)
            return;
        const next = Object.assign({}, homes);
        next[id] = mon;
        UiState.data.workspaceHomes = next;
    }
    function homeAll() { // spaces without a home yet: where they are now
        for (const w of Hyprland.workspaces.values)
            if (w.id > 0 && w.monitor && !homes[w.id])
                setHome(w.id, w.monitor.name);
    }
    function bringBack(mon) {
        for (const w of Hyprland.workspaces.values)
            if (w.id > 0 && homes[w.id] === mon && w.monitor?.name !== mon)
                Hyprland.dispatch("hl.dsp.workspace.move({ workspace = " + w.id + ", monitor = \"" + mon + "\" })");
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            const f = event.data.split(",");
            switch (event.name) {
            case "monitorremovedv2":
            case "monitorremoved":
                root.removedAt = Date.now();
                break;
            case "monitoraddedv2": // id,name,description
                returnLater.mon = f[1];
                returnLater.restart();
                break;
            case "moveworkspacev2": // id,name,monitor
                if (Date.now() - root.removedAt > 4000 && !returnLater.running)
                    root.setHome(Number(f[0]), f[2]);
                break;
            case "createworkspacev2":
                Qt.callLater(root.homeAll);
                break;
            }
        }
    }
    // The new display needs a moment to be set up (and its saved settings applied)
    Timer {
        id: returnLater
        property string mon: ""
        interval: 1500
        onTriggered: root.bringBack(mon)
    }
    Connections {
        target: UiState
        function onLoadedChanged() {
            if (UiState.loaded)
                root.homeAll();
        }
    }
    // (state may have loaded before this started; spaces arrive a moment after start)
    Timer {
        running: true
        interval: 2000
        onTriggered: if (UiState.loaded) root.homeAll()
    }
}
