pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common

// Shake to find the pointer (Settings → Mouse): Hyprland notices the shake
// (hypr/irice/findpointer.lua) and calls irice:findPointer; the pointer then
// grows for a moment (hyprctl setcursor, bigger and back), and a ring
// shrinks onto it, following it while it moves.
Singleton {
    id: root

    readonly property bool enabled: Config.ready && Config.options.findPointer.enabled
    // (Hyprland's side follows the setting live)
    onEnabledChanged: if (Config.ready)
        Quickshell.execDetached(["hyprctl", "eval", "IRICE_FIND_POINTER = " + (enabled ? "true" : "false")])

    GlobalShortcut {
        appid: "irice"
        name: "findPointer"
        description: "Shake to find the pointer"
        onPressed: root.show()
    }
    // qs ipc call findpointer find (to try it)
    IpcHandler {
        target: "findpointer"
        function find(): void { root.show(); }
    }

    property bool active: false
    property real px: 0 // the pointer, in the layout's coordinates
    property real py: 0
    function show() {
        if (!enabled || active)
            return;
        active = true;
        px = 0;
        py = 0; // (until it's been read: no ring in the corner)
        follow.running = true;
        grow.running = true;
        done.restart();
    }
    Timer {
        id: done
        interval: 1400
        onTriggered: root.active = false
    }
    // Where it is, while the ring shows
    Process {
        id: follow
        command: ["python3", "-I", Quickshell.shellDir + "/assets/scripts/pointer-follow.py", "1.4"]
        stdout: SplitParser {
            onRead: line => {
                const m = line.match(/^\s*(-?\d+(?:\.\d+)?),\s*(-?\d+(?:\.\d+)?)\s*$/);
                if (m) {
                    root.px = +m[1];
                    root.py = +m[2];
                }
            }
        }
    }
    // The pointer itself, bigger for a moment (the theme Appearance chose;
    // back to 24, its size, however it ends)
    Process {
        id: grow
        command: ["sh", "-c", "h=$1; [ \"$1\" = macOS ] && [ -d /usr/share/icons/macOS-hypr ] && h=macOS-hypr; "
            + "trap 'hyprctl setcursor \"$h\" 24 >/dev/null' EXIT; "
            + "for s in 44 64 80; do hyprctl setcursor \"$h\" $s >/dev/null; sleep 0.03; done; sleep 0.8; "
            + "hyprctl setcursor \"$h\" 48 >/dev/null; sleep 0.05", "sh",
            Config.options.appearance.cursor === "bibata" ? "Bibata-Modern-Classic" : "macOS"]
    }

    // The ring, on the display the pointer is on (clicks go through)
    LazyLoader {
        active: root.active
        PanelWindow {
            id: win
            readonly property var where: Quickshell.screens.find(s => root.px >= s.x && root.px < s.x + s.width && root.py >= s.y && root.py < s.y + s.height) ?? Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            screen: where
            color: "transparent"
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice-findpointer"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            mask: Region {}

            Rectangle {
                id: ring
                readonly property real size: 150 * scale_
                property real scale_: 1
                visible: root.px !== 0 || root.py !== 0
                x: root.px - win.where.x - width / 2
                y: root.py - win.where.y - height / 2
                width: size
                height: size
                radius: size / 2
                color: Qt.alpha(Theme.accent, 0.12)
                border.width: 3
                border.color: Theme.accent
                opacity: 0
                // In: shrinking onto the pointer; out: fading where it is
                SequentialAnimation {
                    running: true
                    ParallelAnimation {
                        NumberAnimation { target: ring; property: "scale_"; from: 1.6; to: 0.5; duration: 450; easing.type: Easing.OutCubic }
                        NumberAnimation { target: ring; property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
                    }
                    PauseAnimation { duration: 550 }
                    NumberAnimation { target: ring; property: "opacity"; to: 0; duration: 300; easing.type: Easing.InCubic }
                }
            }
        }
    }
}
