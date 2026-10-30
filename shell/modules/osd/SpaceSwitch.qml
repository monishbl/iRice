import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.services

// Moving to another display from the keyboard (Super+number to a space that
// lives there, or a focus-display shortcut): the pointer follows to that
// display, and the others dim for a moment, so it's clear where you are.
// Settings → Displays → "When switching spaces". Mouse-driven focus changes
// are left alone: the pointer is already on that display.
Scope {
    id: root

    readonly property var cfg: Config.options.workspaces
    property string target: "" // display that just took focus
    property bool dimming: false
    property real dim: 0 // 0 .. 1

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "configreloaded") // puts the file's no_warps back
                root.applyWarp();
            if (event.name !== "focusedmonv2" || Quickshell.screens.length < 2)
                return;
            if (!root.cfg.dimOthers)
                return;
            // A display just came or went: focus moving now is the hotplug's
            // (the screen you're on must not darken)
            if (DisplayFx.settling())
                return;
            root.target = event.data.split(",")[0];
            cursor.running = false;
            cursor.running = true;
        }
    }

    // Hyprland moves the pointer itself when focus jumps to another display;
    // the setting only says whether it may
    readonly property bool warp: Config.ready ? cfg.warpPointer : true
    onWarpChanged: applyWarp()
    function applyWarp() {
        Quickshell.execDetached(["hyprctl", "eval", "hl.config({ cursor = { no_warps = " + !warp + " } })"]);
    }

    // Where the pointer is decides whether the keyboard did it: crossing
    // into a display leaves it by the edge it came through, a jump from the
    // keyboard puts it well inside (Hyprland centres it, or it stays put on
    // the old display when warping is off)
    Process {
        id: cursor
        command: ["hyprctl", "cursorpos"]
        stdout: StdioCollector {
            onStreamFinished: {
                const p = text.trim().split(",").map(v => parseFloat(v));
                const mon = Hyprland.monitors.values.find(m => m.name === root.target);
                if (!mon || p.length < 2 || isNaN(p[0]) || !root.cfg.dimOthers || DisplayFx.settling())
                    return;
                const w = mon.width / (mon.scale || 1), h = mon.height / (mon.scale || 1);
                const t = mon.lastIpcObject?.transform ?? 0;
                const lw = t % 2 ? h : w, lh = t % 2 ? w : h;
                const x = p[0] - mon.x, y = p[1] - mon.y;
                const inside = x >= 0 && x < lw && y >= 0 && y < lh;
                const edge = 48;
                if (inside && (x < edge || y < edge || x > lw - edge || y > lh - edge))
                    return; // the pointer took it there
                root.dimming = true;
                fade.restart();
            }
        }
    }

    SequentialAnimation {
        id: fade
        NumberAnimation { target: root; property: "dim"; to: 1; duration: 150; easing.type: Easing.OutCubic }
        PauseAnimation { duration: 350 }
        NumberAnimation { target: root; property: "dim"; to: 0; duration: 400; easing.type: Easing.InCubic }
        ScriptAction { script: root.dimming = false }
    }

    // A dim layer per other display, made only while it shows
    Variants {
        model: Quickshell.screens

        Scope {
            id: perScreen
            required property var modelData

            LazyLoader {
                active: root.dimming && perScreen.modelData.name !== root.target

                PanelWindow {
                    screen: perScreen.modelData
                    anchors { top: true; bottom: true; left: true; right: true }
                    exclusionMode: ExclusionMode.Ignore
                    WlrLayershell.layer: WlrLayer.Overlay
                    WlrLayershell.namespace: "irice:dim"
                    mask: Region {} // clicks go through
                    color: "transparent"
                    Rectangle {
                        anchors.fill: parent
                        color: "black"
                        opacity: 0.35 * root.dim
                    }
                }
            }
        }
    }
}
