pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// Active keyboard layout of the main keyboard, and switching between layouts.
Singleton {
    id: root

    property string name: "" // e.g. "English (US)"
    property var layouts: [] // configured layout codes, e.g. ["us", "de"]
    readonly property bool multiple: layouts.length > 1
    readonly property string short: {
        const n = name.toLowerCase();
        const m = n.match(/^([a-z]{2})/);
        return m ? m[1].toUpperCase() : (layouts[0] ?? "").toUpperCase();
    }

    function next() {
        Quickshell.execDetached(["hyprctl", "switchxkblayout", "all", "next"]);
    }

    function refresh() {
        proc.running = true;
    }

    Component.onCompleted: refresh()

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "activelayout")
                root.name = event.data.split(",").slice(1).join(",");
        }
    }

    Process {
        id: proc
        command: ["hyprctl", "-j", "devices"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const kbs = JSON.parse(text).keyboards ?? [];
                    const main = kbs.find(k => k.main) ?? kbs[0];
                    if (!main)
                        return;
                    root.name = main.active_keymap ?? "";
                    root.layouts = (main.layout ?? "").split(",").filter(x => x);
                } catch (e) {}
            }
        }
    }
}
