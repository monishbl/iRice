import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.services

// Moving to another display (from the keyboard, or the pointer crossing
// over): the display you moved to gets a brief soft glow at its edges, in the
// accent colour, and the one you left dims a little for a moment, so it's
// clear where you are. Not while a display is being plugged in or unplugged
// (focus moving then is the hotplug's). Reduce motion: only the glow, a
// plain fade. Settings → Displays → "Show which display you moved to".
// Also applies whether Hyprland may move the pointer to the display a
// shortcut takes you to (workspaces.warpPointer).
Scope {
    id: root

    readonly property var cfg: Config.options.workspaces
    readonly property bool still: Config.ready && Config.options.appearance.reduceMotion === true
    property string target: "" // display you moved to
    property string origin: "" // display you left
    // The focused display, from Hyprland itself at start (Quickshell's
    // focusedMonitor can stay empty until focus first moves)
    property string focused: ""
    Process {
        running: true
        command: ["hyprctl", "-j", "monitors"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const m = JSON.parse(text).find(x => x.focused);
                    if (m && root.focused === "")
                        root.focused = m.name;
                } catch (e) {}
            }
        }
    }
    property bool showing: false
    property real glow: 0 // 0 .. 1
    property real dim: 0 // 0 .. 1

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "configreloaded") // puts the file's no_warps back
                root.applyWarp();
            if (event.name !== "focusedmonv2")
                return;
            const now = event.data.split(",")[0];
            const was = root.focused;
            root.focused = now;
            if (now === was || was === "" || Quickshell.screens.length < 2 || !root.cfg.dimOthers)
                return;
            // A display just came or went: focus moving now is the hotplug's
            if (DisplayFx.settling())
                return;
            root.target = now;
            root.origin = was;
            root.showing = true;
            cue.restart();
        }
    }

    // Hyprland moves the pointer itself when focus jumps to another display;
    // the setting only says whether it may
    readonly property bool warp: Config.ready ? cfg.warpPointer : true
    onWarpChanged: applyWarp()
    function applyWarp() {
        Quickshell.execDetached(["hyprctl", "eval", "hl.config({ cursor = { no_warps = " + !warp + " } })"]);
    }

    // In 150 ms (ease-out), held a moment, out in 450 ms (ease-in); the dim
    // on the display you left follows the same curve, lighter
    SequentialAnimation {
        id: cue
        ScriptAction { script: { root.glow = 0; root.dim = 0; } }
        ParallelAnimation {
            NumberAnimation { target: root; property: "glow"; to: 1; duration: root.still ? 100 : 150; easing.type: Easing.OutCubic }
            NumberAnimation { target: root; property: "dim"; to: root.still ? 0 : 1; duration: 150; easing.type: Easing.OutCubic }
        }
        PauseAnimation { duration: 250 }
        ParallelAnimation {
            NumberAnimation { target: root; property: "glow"; to: 0; duration: root.still ? 300 : 450; easing.type: Easing.InCubic }
            NumberAnimation { target: root; property: "dim"; to: 0; duration: 450; easing.type: Easing.InCubic }
        }
        ScriptAction { script: root.showing = false }
    }

    // A layer on the two displays involved, made only while the cue shows
    Variants {
        model: Quickshell.screens

        Scope {
            id: perScreen
            required property var modelData
            readonly property bool isTarget: perScreen.modelData.name === root.target
            readonly property bool isLeft: perScreen.modelData.name === root.origin

            LazyLoader {
                active: root.showing && (perScreen.isTarget || (perScreen.isLeft && !root.still))

                PanelWindow {
                    screen: perScreen.modelData
                    anchors { top: true; bottom: true; left: true; right: true }
                    exclusionMode: ExclusionMode.Ignore
                    WlrLayershell.layer: WlrLayer.Overlay
                    WlrLayershell.namespace: "irice:dim"
                    mask: Region {} // clicks go through
                    color: "transparent"

                    // The display you left: a little darker
                    Rectangle {
                        anchors.fill: parent
                        visible: perScreen.isLeft
                        color: "black"
                        opacity: 0.18 * root.dim
                    }
                    // The display you moved to: the accent along its edges,
                    // fading inward
                    Item {
                        id: edges
                        anchors.fill: parent
                        visible: perScreen.isTarget
                        opacity: root.glow
                        // A wide, faint band that eases out (no inner edge
                        // to see): strong at the edge, then soft, then nothing
                        readonly property real depth: 72
                        // (a light shade of the accent: the glow reads as light
                        // coming in from the edge, not a coloured border)
                        readonly property color tone: Theme.accentRamp[3] ?? Theme.accent
                        readonly property color strong: Qt.alpha(tone, 0.45)
                        readonly property color soft: Qt.alpha(tone, 0.22)
                        readonly property color faint: Qt.alpha(tone, 0.08)
                        readonly property color none: Qt.alpha(tone, 0)
                        Rectangle {
                            anchors { left: parent.left; right: parent.right; top: parent.top }
                            height: edges.depth
                            gradient: Gradient {
                                GradientStop { position: 0; color: edges.strong }
                                GradientStop { position: 0.22; color: edges.soft }
                                GradientStop { position: 0.55; color: edges.faint }
                                GradientStop { position: 1; color: edges.none }
                            }
                        }
                        Rectangle {
                            anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                            height: edges.depth
                            gradient: Gradient {
                                GradientStop { position: 0; color: edges.none }
                                GradientStop { position: 0.45; color: edges.faint }
                                GradientStop { position: 0.78; color: edges.soft }
                                GradientStop { position: 1; color: edges.strong }
                            }
                        }
                        Rectangle {
                            anchors { top: parent.top; bottom: parent.bottom; left: parent.left }
                            width: edges.depth
                            gradient: Gradient {
                                orientation: Gradient.Horizontal
                                GradientStop { position: 0; color: edges.strong }
                                GradientStop { position: 0.22; color: edges.soft }
                                GradientStop { position: 0.55; color: edges.faint }
                                GradientStop { position: 1; color: edges.none }
                            }
                        }
                        Rectangle {
                            anchors { top: parent.top; bottom: parent.bottom; right: parent.right }
                            width: edges.depth
                            gradient: Gradient {
                                orientation: Gradient.Horizontal
                                GradientStop { position: 0; color: edges.none }
                                GradientStop { position: 0.45; color: edges.faint }
                                GradientStop { position: 0.78; color: edges.soft }
                                GradientStop { position: 1; color: edges.strong }
                            }
                        }
                    }
                }
            }
        }
    }
}
