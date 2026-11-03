pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common

// Shake to find the pointer (Settings → Accessibility): Hyprland notices the
// shake (hypr/irice/findpointer.lua) and calls irice:findPointer. Then, as
// findPointer.style says: a ring shrinks onto the pointer, following it
// while it moves (ring); the pointer itself is big for a moment (grow,
// hyprctl setcursor: one step up and one back, since every size change
// reloads the theme and steps in between stuttered); or both.
Singleton {
    id: root

    readonly property bool enabled: Config.ready && Config.options.findPointer.enabled && Config.options.findPointer.style !== "off"
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

    readonly property string style: Config.options.findPointer.style || "ring"
    readonly property bool ring: style !== "grow"
    property bool active: false
    property real px: 0 // the pointer, in the layout's coordinates
    property real py: 0
    function show() {
        if (!enabled || active)
            return;
        active = true;
        px = 0;
        py = 0; // (until it's been read: no ring in the corner)
        follow.running = ring;
        grow.running = style !== "ring";
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
        // (every 8 ms: at the 40 ms a drag needs, the ring trailed in jumps)
        command: ["python3", "-I", Quickshell.shellDir + "/assets/scripts/pointer-follow.py", "1.4", "0.008"]
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
    // The pointer itself, bigger for a moment (the theme Appearance chose).
    // Run on its own (setsid), so it always shrinks back, to the size it
    // had (XCURSOR_SIZE, else 24), whatever happens to the shell; one at a
    // time (a lock), and only a short hold: every size change reloads the
    // theme, which is what makes it slow
    Process {
        id: grow
        command: ["setsid", "-f", "sh", "-c", "h=$1; [ \"$1\" = macOS ] && [ -d /usr/share/icons/macOS-hypr ] && h=macOS-hypr; "
            + "s=${XCURSOR_SIZE:-24}; l=${XDG_RUNTIME_DIR:-/tmp}/irice-findpointer.lock; "
            + "exec 9>\"$l\"; flock -n 9 || exit 0; "
            + "trap 'hyprctl setcursor \"$h\" \"$s\" >/dev/null' EXIT INT TERM HUP; "
            + "hyprctl setcursor \"$h\" $((s * 5 / 2)) >/dev/null; sleep 0.75", "sh",
            AppTheme.cursorTheme]
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
                property real scale_: 1
                visible: root.ring && (root.px !== 0 || root.py !== 0)
                x: root.px - win.where.x - width / 2
                y: root.py - win.where.y - height / 2
                // (one size, scaled on the GPU: resizing redrew it every frame)
                width: 150
                height: 150
                radius: 75
                scale: scale_
                border.width: 3 / Math.max(0.5, scale_) // (the line stays 3 px thick)
                color: Qt.alpha(Theme.accent, 0.12)
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
