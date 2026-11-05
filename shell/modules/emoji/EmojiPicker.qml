import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common
import qs.services

// Emoji picker (Super+Period): emoji, kaomoji, text symbols and GIFs, opened
// near the pointer in the app you're typing in. Enter or click types the pick
// into that app and copies it; Shift copies only. Panel name: "emoji".
//   keybind:  hl.dsp.global("tessera:emoji")
//   IPC:      qs ipc call emoji toggle | open | close
Scope {
    id: root

    function focusedScreen() {
        return Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
    }

    // Look up the pointer and focused window whenever the picker opens, however
    // it was opened (shortcut, IPC, Panels.show); not while tessera-ui draws it
    Connections {
        target: Panels
        function onOpenChanged() {
            if (Panels.open === "emoji" && !slots.rust)
                EmojiStore.prepare();
        }
    }

    GlobalShortcut {
        appid: "tessera"
        name: "emoji"
        description: "Emoji, kaomoji, symbols and GIFs"
        onPressed: Panels.toggle("emoji", root.focusedScreen())
    }

    IpcHandler {
        target: "emoji"
        function toggle(): void {
            Panels.toggle("emoji", root.focusedScreen());
        }
        function open(): void {
            Panels.show("emoji", root.focusedScreen());
        }
        function close(): void {
            if (Panels.open === "emoji")
                Panels.close();
        }
    }

    Variants {
        id: slots
        model: Quickshell.screens
        // (tessera-ui draws it: rust.surfaces lists "emoji")
        readonly property bool rust: RustUi.drawing("emoji")

        PanelLoader {
            id: slot
            required property var modelData
            panel: "emoji"
            screen: modelData
            elsewhere: slots.rust
            // Kept for a while after closing: a new window draws every emoji
            // glyph again (Qt rasterises each colour emoji on first use), so
            // reopening within five minutes is instant
            linger: 300000

            PickerWindow {
                screen: slot.modelData
            }
        }
    }
}
