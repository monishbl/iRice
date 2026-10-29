import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.glass

// A right-click menu at the pointer, for lists that live in different
// windows (a menu bar menu, Control Center, the Settings window): where the
// pointer is, and on which display, comes from Hyprland, so it doesn't
// matter which window asked. Items as for ContextMenu:
//   PointerMenu { id: menu }  …  menu.open([{ text: "Connect", action: … }])
// While it's open, the menu bar menu under it stays open (Panels.holdMenus).
Item {
    id: root

    visible: false
    property var items: []
    property var screen: null
    property real px: 0
    property real py: 0
    readonly property bool shown: loader.active

    function open(list) {
        items = list;
        where.running = true;
    }
    function close() {
        loader.active = false;
    }
    onShownChanged: Panels.holdMenus += shown ? 1 : -1
    Component.onDestruction: if (shown) Panels.holdMenus -= 1

    Process {
        id: where
        command: ["sh", "-c", "hyprctl -j cursorpos; hyprctl -j monitors"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    // Two JSON documents, the pointer's place then the displays
                    const cut = text.indexOf("}") + 1;
                    const p = JSON.parse(text.slice(0, cut));
                    const mons = JSON.parse(text.slice(cut));
                    const m = mons.find(m => p.x >= m.x && p.y >= m.y && p.x < m.x + m.width / (m.scale || 1) && p.y < m.y + m.height / (m.scale || 1)) ?? mons.find(m => m.focused);
                    root.screen = Quickshell.screens.find(s => s.name === m?.name) ?? Quickshell.screens[0];
                    root.px = p.x - (m?.x ?? 0);
                    root.py = p.y - (m?.y ?? 0);
                    loader.active = true;
                } catch (e) {}
            }
        }
    }

    Loader {
        id: loader
        active: false
        sourceComponent: ContextMenu {
            screen: root.screen
            pointX: root.px + 100 // (the menu centres on pointX: its left edge at the pointer)
            pointY: root.py
            upward: false
            items: root.items
            onDismissed: root.close()
        }
    }
}
