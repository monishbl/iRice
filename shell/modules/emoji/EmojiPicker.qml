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
            if (Panels.open === "emoji" && !RustUi.drawing("emoji"))
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

    // The picker's windows (EmojiSlots.qml), loaded only while tessera-ui
    // isn't to draw it (RustUi.away); the shortcut and IPC above stay here
    LazyLoader {
        active: !RustUi.away("emoji")
        source: Qt.resolvedUrl("EmojiSlots.qml")
    }
}
