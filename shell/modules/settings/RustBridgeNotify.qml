import QtQuick
import Quickshell
import qs.common
import qs.services

// Settings drawn in Rust: the "notify" page group's side in this shell
// (RustBridge.qml): Notifications & Island, Focus, Windows & Spaces. While
// Settings is open (`live`) it tells the pages what only this shell knows
// (`feed`, the pages' cx.feed["notify"]) and does what they ask (`act`:
// cx.ask("notify", what, args)).
Scope {
    id: root

    property bool live: false
    // A mode Settings just added (Focus.addMode's id): the page shows it
    property string added: ""

    readonly property var feed: live ? ({
            focus: {
                on: Focus.on,
                active: Focus.active,
                statusText: Focus.statusText,
                modes: Focus.modes,
                added: root.added
            },
            notificationCount: Notifications.list.length,
            // (the CPU's reading changes every few seconds: only for the page that shows it)
            temperature: Panels.settingsPage === "notifications/island" ? Resources.temperature : 0,
            networkCanWatch: NetworkAlerts.canWatch,
            networkReason: NetworkAlerts.reason,
            decorationsPlugin: Decorations.pluginLoaded,
            pluginControls: Decorations.pluginControls,
            glassPlugin: GlassPlugin.available,
            rulesError: WindowRules.error,
            pickerBusy: FilePicker.busy
        }) : null
    onFeedChanged: if (live) RustUi.tell("settings", { feed: { notify: feed } })

    function act(what, args) {
        switch (what) {
        case "play": // [sound] (a preview, played even when that sound is off)
            Sounds.play(String(args[0] ?? ""), true);
            break;
        case "playStartup": { // [id] (a sound to log in to: "" is Tessera's own)
            const id = String(args[0] ?? "");
            if (!/^[a-z-]*$/.test(id))
                break;
            Sounds.playFile(id === "" ? Sounds.own + "startup.ogg" : Quickshell.shellDir + "/assets/sounds/startup/" + id + ".ogg", Config.options.sounds.startupVolume);
            break;
        }
        case "focusOn": // [mode id] (Do Not Disturb)
            Focus.activate(String(args[0] ?? "dnd"), 0, false);
            break;
        case "focusOff":
            Focus.deactivate();
            break;
        case "clearAll":
            Notifications.clearAll();
            break;
        case "setMode": // [id, patch]
            if (typeof args[0] === "string" && args[1] && typeof args[1] === "object")
                Focus.setMode(args[0], args[1]);
            break;
        case "addMode": // [name]
            if (typeof args[0] === "string" && args[0].trim() !== "")
                root.added = Focus.addMode(args[0].trim());
            break;
        case "removeMode": // [id]
            if (typeof args[0] === "string")
                Focus.removeMode(args[0]);
            break;
        case "restoreMode": // [mode] (Undo after deleting it)
            if (args[0] && typeof args[0] === "object" && args[0].id && !Focus.modeById(args[0].id)) {
                Focus.modes = Focus.modes.concat([args[0]]);
                Focus.save();
            }
            break;
        case "installHyprbars":
            Decorations.installPlugin();
            break;
        default:
            console.warn("settings (notify): no act " + what);
        }
    }
}
