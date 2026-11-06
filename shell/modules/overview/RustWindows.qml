import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.services

// While tessera-ui draws the window-management surfaces (rust.surfaces:
// overview, missioncontrol, switcher, stagemanager, launchpad, tabs), what
// this shell still owns that they show goes to them, and what they ask of
// it comes back (native/tessera-ui/src/surfaces/wm):
//  - to them (RustUi.tell, a line on its stdin): each display's block of
//    spaces and its name (Spaces), App Exposé's app (Panels.exposeApp),
//    whether the screen is locked, Stage Manager's waiting stages;
//  - from them, RustUi's "wm" call: Spaces' moves, Apps.show for a
//    minimized window, Keep in Dock, Stage Manager's clicks, the panels
//    they open (the switcher registers as the open panel; Get apps).
Scope {
    id: root

    readonly property var mine: ["overview", "missioncontrol", "switcher", "stagemanager", "launchpad", "tabs"]
    readonly property bool on: mine.some(n => RustUi.drawing(n))

    readonly property var spaces: ({ bases: Spaces.bases, labels: Spaces.labels, count: Config.options.menubar.workspaceCount })
    function sendSpaces() {
        for (const n of ["overview", "missioncontrol", "switcher"])
            RustUi.tell(n, { spaces: root.spaces });
    }
    function sendLocked() {
        for (const n of ["switcher", "stagemanager"])
            RustUi.tell(n, { locked: System.locked });
    }
    // Stage Manager's waiting stages, per space: { "<ws>": [{ id, wins }] }
    readonly property var waiting: {
        const out = {};
        for (const ws of Object.keys(StageManager.stages))
            out[ws] = StageManager.waiting(Number(ws)).map(s => ({ id: s.id, wins: s.wins }));
        return out;
    }
    function sendStages() {
        RustUi.tell("stagemanager", { stage: { waiting: root.waiting } });
    }
    function sendAll() {
        if (!on)
            return;
        sendSpaces();
        sendLocked();
        sendStages();
        RustUi.tell("missioncontrol", { expose: Panels.exposeApp });
    }

    onSpacesChanged: if (on) Qt.callLater(sendSpaces)
    onWaitingChanged: if (on) Qt.callLater(sendStages)
    onOnChanged: Qt.callLater(sendAll)
    Connections {
        target: RustUi
        function onStarted() {
            Qt.callLater(root.sendAll);
        }
        function onDrawnChanged() {
            Qt.callLater(root.sendAll);
        }
        function onWmCall(name, a) {
            root.call(name, a);
        }
    }
    Connections {
        target: System
        function onLockedChanged() {
            root.sendLocked();
        }
    }
    // (before Panels opens Mission Control: the app it's for)
    Connections {
        target: Panels
        function onExposeAppChanged() {
            RustUi.tell("missioncontrol", { expose: Panels.exposeApp });
        }
    }

    function toplevel(address) {
        const k = String(address ?? "").replace(/^0x/, "").toLowerCase();
        return Hyprland.toplevels.values.find(t => t.address.replace(/^0x/, "").toLowerCase() === k) ?? null;
    }

    function call(name, a) {
        switch (name) {
        case "spaces.go":
            Spaces.go(Number(a[0]));
            break;
        case "spaces.newSpace":
            Spaces.newSpace(String(a[0] ?? ""));
            break;
        case "spaces.sendWindow":
            Spaces.sendWindow(String(a[0] ?? ""), Number(a[1]));
            break;
        case "spaces.sendWindowToDisplay":
            Spaces.sendWindowToDisplay(String(a[0] ?? ""), String(a[1] ?? ""));
            break;
        case "spaces.moveSpaceTo":
            if (Spaces.exists(Number(a[0])))
                Spaces.moveSpaceTo(Number(a[0]), String(a[1] ?? ""));
            break;
        case "spaces.move":
            Spaces.move((a[0] ?? []).map(Number), Number(a[1]), Number(a[2]));
            break;
        case "apps.show": {
            const t = toplevel(a[0]);
            if (t)
                Apps.show(t);
            break;
        }
        case "apps.pin":
            Apps.pin(String(a[0] ?? ""));
            break;
        case "apps.unpin":
            Apps.unpin(String(a[0] ?? ""));
            break;
        case "panels.show": // (the switcher, once its strip shows)
            if (a[0] === "switcher")
                Panels.show("switcher", Quickshell.screens.find(s => s.name === a[1]) ?? undefined);
            break;
        case "panels.closeOpen":
            if (Panels.open !== "")
                Panels.close();
            break;
        case "panels.openAppStore":
            Panels.openAppStore();
            break;
        case "stage.show":
            StageManager.show(Number(a[0]), Number(a[1]));
            break;
        case "stage.merge":
            StageManager.merge(Number(a[0]), Number(a[1]));
            break;
        default:
            console.warn("tessera-ui: no wm call " + name);
        }
    }
}
