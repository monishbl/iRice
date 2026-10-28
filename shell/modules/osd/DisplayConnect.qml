import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.services

// A display connects or leaves: the screen that arrived fills with the
// wallpaper from the edge it shares with its neighbour (extend, liquid or
// warp), and the neighbours light up at that edge (a display that left: the
// light pulls back in). What and where come from DisplayFx; each screen gets
// an overlay only while its effect plays (about a second), then nothing.
// Settings → Displays → When a display connects.
Scope {
    id: root

    Variants {
        model: Quickshell.screens

        Scope {
            id: perScreen
            required property var modelData
            readonly property var run: DisplayFx.runs.find(r => r.screen === modelData.name) ?? null

            LazyLoader {
                active: perScreen.run !== null

                PanelWindow {
                    id: win
                    screen: perScreen.modelData
                    anchors { top: true; bottom: true; left: true; right: true }
                    exclusionMode: ExclusionMode.Ignore
                    WlrLayershell.layer: WlrLayer.Overlay
                    WlrLayershell.namespace: "irice:display-connect"
                    mask: Region {} // clicks go through
                    color: "transparent"

                    readonly property var run: perScreen.run
                    // Reduced motion: no travel, a fade
                    readonly property bool still: Config.options.appearance.reduceMotion === true
                    readonly property bool liquid: run?.style === "liquid"
                    readonly property int duration: Math.round((liquid ? (run?.role === 0 ? 1100 : 1000) : run?.style === "extend" && run?.role === 0 ? 1050 : run?.role === 0 ? 950 : 800) * CoolDown.motionScale)
                    // The new screen waits for its neighbour's part (the drop
                    // leaving it, the fold starting) before its own
                    readonly property int lead: Math.round((run?.lead ?? 0) * CoolDown.motionScale)
                    property real progress: 0
                    property real shown: 1

                    function start() {
                        anim.stop();
                        progress = 0;
                        shown = 1;
                        if (run && !run.pending)
                            anim.start();
                    }
                    // Restart only for a different run (or one that's just been
                    // placed): the runs list is rebuilt whenever any screen's run
                    // starts or ends, which would otherwise replay this one
                    readonly property string runKey: run ? run.id + (run.pending ? "p" : "") : ""
                    onRunKeyChanged: start()
                    Component.onCompleted: start()

                    SequentialAnimation {
                        id: anim
                        PauseAnimation { duration: win.lead }
                        NumberAnimation {
                            target: win
                            property: "progress"
                            to: 1
                            duration: win.duration
                            // liquid: linear (the shader springs by itself)
                            easing.type: win.liquid || (win.run?.style === "extend" && win.run?.role === 0) ? Easing.Linear : win.run?.role === 0 ? Easing.InOutCubic : Easing.InOutSine // (liquid and extend ease in the shader)
                        }
                        // the arrived screen's picture is the desktop's own
                        // wallpaper: fading the overlay hands it over
                        NumberAnimation {
                            target: win
                            property: "shown"
                            to: 0
                            duration: Math.round(220 * CoolDown.motionScale)
                            easing.type: Easing.InCubic
                        }
                        ScriptAction {
                            script: if (win.run) DisplayFx.finish(win.run.id)
                        }
                    }
                    // A pending screen that never gets placed doesn't stay dark
                    Timer {
                        running: win.run?.pending ?? false
                        interval: 3000
                        onTriggered: if (win.run) DisplayFx.finish(win.run.id)
                    }

                    // The wallpaper as the desktop shows it (cropped to the screen,
                    // centred): an Image as a texture would be the whole,
                    // uncropped picture, stretched to the screen
                    Item {
                        id: framed
                        visible: false
                        width: win.width
                        height: win.height
                        Image {
                            anchors.fill: parent
                            source: Wallpaper.source
                            // neighbours only draw light: a token picture for them
                            sourceSize: win.run?.role === 0 ? Qt.size(win.width, win.height) : Qt.size(16, 16)
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: false
                        }
                    }
                    ShaderEffectSource {
                        id: picture
                        sourceItem: framed
                        hideSource: true // (live: the picture loads after the run starts; this window lives ~1 s)
                        visible: false
                    }

                    ShaderEffect {
                        anchors.fill: parent
                        visible: !win.still
                        opacity: win.shown
                        property real progress: win.progress
                        property real style: ({ extend: 0, liquid: 1, warp: 2 })[win.run?.style] ?? 0
                        property real role: win.run?.role ?? 1
                        property real edge: win.run?.edge ?? 0
                        property real along: win.run?.focus ?? 0.5
                        property real reverse: win.run?.reverse ?? 0
                        property size size: Qt.size(width, height)
                        // the light: white, warmed a little by the accent
                        property color tint: Qt.rgba(0.75 + 0.25 * Theme.accent.r, 0.75 + 0.25 * Theme.accent.g, 0.75 + 0.25 * Theme.accent.b, 1)
                        property var source: picture
                        fragmentShader: Qt.resolvedUrl("displayfx.frag.qsb")
                    }

                    // Reduced motion: the arrived screen fades in from dark;
                    // neighbours show nothing
                    Rectangle {
                        anchors.fill: parent
                        visible: win.still && win.run?.role === 0
                        color: "black"
                        opacity: (1 - win.progress) * win.shown
                    }
                }
            }
        }
    }

    // qs ipc call displays preview <extend|liquid|warp|""> <left|right|top|bottom>
    IpcHandler {
        target: "displays"
        function preview(effect: string, side: string): void {
            DisplayFx.preview(effect, side);
        }
    }
}
