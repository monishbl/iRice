import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.services
import "../../common/SettingsMap.js" as SettingsMap

// Settings drawn in Rust (rust.surfaces: settings,
// native/tessera-ui/src/surfaces/settings): Settings still opens through
// Panels.openSettings(page, target) (the shortcut, IPC, the system menu,
// Quick Settings…), and while tessera-ui draws it this forwards
// Panels.settingsOpen, .settingsPage and .settingsTarget as commands on its
// stdin ({cmd: "open" | "page" | "close", surface: "settings", …}). It
// answers what the window asks (RustUi.settingsCall): "close" (Esc),
// "page" (Panels.settingsPage follows the window, so it opens there next
// time), "pick" (a file chooser for a path row) and "act" (something only
// this shell does, by page group: RustBridgeLook.qml and its siblings,
// which also tell the pages what only this shell knows while it's open:
// RustUi.tell("settings", { feed: { <group>: … } })).
Scope {
    id: root

    readonly property bool on: RustUi.drawing("settings")
    // open in Rust: the page groups say what they know
    readonly property bool live: on && Panels.settingsOpen

    function send(o) {
        RustUi.send(Object.assign({ surface: "settings" }, o));
    }
    function forwardOpen() {
        if (!root.live)
            return;
        root.send({ cmd: "open", screen: Hyprland.focusedMonitor?.name ?? "", page: Panels.settingsPage, target: Panels.settingsTarget });
        Panels.settingsTarget = "";
    }
    onLiveChanged: {
        if (live)
            Qt.callLater(forwardOpen);
        else if (on)
            send({ cmd: "close" });
    }
    Connections {
        target: Panels
        function onSettingsPageChanged() {
            if (root.live && !SettingsMap.aliases[Panels.settingsPage])
                Qt.callLater(() => {
                    root.send({ cmd: "page", page: Panels.settingsPage, target: Panels.settingsTarget });
                    Panels.settingsTarget = "";
                });
        }
        function onSettingsTargetChanged() {
            if (root.live && Panels.settingsTarget !== "")
                Qt.callLater(() => {
                    if (Panels.settingsTarget === "")
                        return;
                    root.send({ cmd: "page", page: Panels.settingsPage, target: Panels.settingsTarget });
                    Panels.settingsTarget = "";
                });
        }
    }

    // What the window asks of this shell
    readonly property var groups: ({ look: look, notify: notify, devices: devices, input: input, accounts: accounts, system: system, about: about })
    Connections {
        target: RustUi
        function onSettingsCall(what, args) {
            switch (what) {
            case "close":
                Panels.settingsOpen = false;
                break;
            case "page":
                if (typeof args[0] === "string" && args[0] !== "" && Panels.settingsPage !== args[0])
                    Panels.settingsPage = args[0];
                break;
            case "pick": { // [row id, folder, title, start, filter]
                const id = String(args[0] ?? "");
                FilePicker.pick({ folder: !!args[1], title: String(args[2] ?? ""), start: String(args[3] || "~"), filter: String(args[4] ?? "") }, p => RustUi.tell("settings", { picked: { id: id, path: p } }));
                break;
            }
            case "act": { // [group, what, …args]
                const g = root.groups[String(args[0] ?? "")];
                if (g)
                    g.act(String(args[1] ?? ""), args.slice(2));
                else
                    console.warn("settings: no page group " + args[0]);
                break;
            }
            default:
                console.warn("settings: no call " + what);
            }
        }
    }

    // ── The page groups' side ─────────────────────────────────────────────
    RustBridgeLook { id: look; live: root.live }
    RustBridgeNotify { id: notify; live: root.live }
    RustBridgeDevices { id: devices; live: root.live }
    RustBridgeInput { id: input; live: root.live }
    RustBridgeAccounts { id: accounts; live: root.live }
    RustBridgeSystem { id: system; live: root.live }
    RustBridgeAbout { id: about; live: root.live }
}
