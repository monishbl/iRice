import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.services

// Moving to another display (from the keyboard, or the pointer crossing
// over): the display you moved to shows it for a moment, so it's clear where
// you are. Settings → Displays → "Show which display you moved to"
// (workspaces.switchCue): glow (a soft glow at its edges, in a light shade of
// the accent), dim (it dims briefly) or off. Nothing happens to the display
// you left. Not while a display is being plugged in or unplugged (focus
// moving then is the hotplug's), and not for moves made from the Overview or
// Mission Control (moving a window or a space there never takes you along;
// Hyprland focuses the other display on the way, see Spaces.hush). Reduce
// motion: the glow as it is, the dim a plain short fade.
// Also applies whether Hyprland may move the pointer to the display a
// shortcut takes you to (workspaces.warpPointer).
Scope {
    id: root

    readonly property var cfg: Config.options.workspaces
    readonly property bool still: Config.ready && Config.options.appearance.reduceMotion === true
    property string target: "" // display you moved to
    // glow | dim | off (from the older on/off setting until it's set)
    readonly property string mode: {
        const m = cfg.switchCue;
        return m === "glow" || m === "dim" || m === "off" ? m : (cfg.dimOthers === false ? "off" : "glow");
    }
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
    property string shownMode: "glow" // the mode of the cue on screen

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
            if (now === was || was === "" || Quickshell.screens.length < 2 || root.mode === "off")
                return;
            // A display just came or went: focus moving now is the hotplug's
            if (DisplayFx.settling())
                return;
            // A move from the Overview or Mission Control (or just made
            // there): you didn't go anywhere
            if (Spaces.quiet() || Panels.open === "overview" || Panels.open === "missionControl")
                return;
            root.target = now;
            root.shownMode = root.mode;
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

    // Glow: in 150 ms (ease-out), held a moment, out in 450 ms (ease-in).
    // Dim: the same curve, lighter; with reduce motion a plain short fade
    // (in 100 ms, out 250 ms)
    SequentialAnimation {
        id: cue
        readonly property bool plainDim: root.shownMode === "dim" && root.still
        ScriptAction { script: { root.glow = 0; root.dim = 0; } }
        NumberAnimation { target: root; property: root.shownMode === "dim" ? "dim" : "glow"; to: 1; duration: root.still ? 100 : 150; easing.type: cue.plainDim ? Easing.Linear : Easing.OutCubic }
        PauseAnimation { duration: cue.plainDim ? 150 : 250 }
        NumberAnimation { target: root; property: root.shownMode === "dim" ? "dim" : "glow"; to: 0; duration: cue.plainDim ? 250 : root.still ? 300 : 450; easing.type: cue.plainDim ? Easing.Linear : Easing.InCubic }
        ScriptAction { script: root.showing = false }
    }

    // A layer on the display you moved to, made only while the cue shows
    Variants {
        model: Quickshell.screens

        Scope {
            id: perScreen
            required property var modelData
            readonly property bool isTarget: perScreen.modelData.name === root.target

            LazyLoader {
                active: root.showing && perScreen.isTarget

                PanelWindow {
                    screen: perScreen.modelData
                    anchors { top: true; bottom: true; left: true; right: true }
                    exclusionMode: ExclusionMode.Ignore
                    WlrLayershell.layer: WlrLayer.Overlay
                    WlrLayershell.namespace: "irice:dim"
                    mask: Region {} // clicks go through
                    color: "transparent"

                    // Dim: the display you moved to darkens briefly
                    Rectangle {
                        anchors.fill: parent
                        visible: root.shownMode === "dim"
                        color: "black"
                        opacity: 0.18 * root.dim
                    }
                    // The display you moved to: the accent along its edges,
                    // fading inward
                    Item {
                        id: edges
                        anchors.fill: parent
                        visible: root.shownMode === "glow"
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
