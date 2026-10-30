import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Volume / brightness HUD: a small glass capsule under the menu bar, shown for
// a moment whenever either changes (or the island's HUD row, when it's set
// to show there). Over a fullscreen app (a video, a game) it's only a thin
// bar along the top edge instead. Also owns the media-key shortcuts.
Scope {
    id: root

    property string kind: "volume" // volume | brightness | mic
    property bool shown: false
    property bool armed: false // ignore the initial values at startup

    readonly property real value: kind === "brightness" ? Brightness.value : kind === "mic" ? (Audio.micMuted ? 0 : 1) : (Audio.muted ? 0 : Audio.volume)
    readonly property string icon: kind === "brightness" ? "sun" : kind === "mic" ? (Audio.micMuted ? "microphone-slash" : "microphone") : Audio.icon
    readonly property string label: kind === "brightness" ? "Display" : kind === "mic" ? (Audio.micMuted ? "Microphone off" : "Microphone on") : "Sound"

    Binding {
        target: Panels
        property: "osdVisible"
        value: root.shown
    }

    function show(k) {
        if (!armed)
            return;
        // A fullscreen window on the focused display: the thin top bar
        if (Hyprland.focusedMonitor?.activeWorkspace?.hasFullscreen ?? false) {
            barKind = k;
            barShown = true;
            barHide.restart();
            return;
        }
        if (Config.options.island.enabled && Config.options.island.hud) {
            Island.hud(k);
            return;
        }
        kind = k;
        shown = true;
        hideTimer.restart();
    }

    Timer { interval: 2000; running: true; onTriggered: root.armed = true }
    Timer { id: hideTimer; interval: 1600; onTriggered: root.shown = false }

    Connections {
        target: Audio
        function onVolumeChanged() { root.show("volume"); }
        function onMutedChanged() { root.show("volume"); }
        function onMicMutedChanged() { root.show("mic"); }
    }
    Connections {
        target: Brightness
        function onValueChanged() { root.show("brightness"); }
    }

    // Exists only while visible (plus time for the exit animation)
    property bool windowAlive: false
    readonly property bool windowWanted: root.shown
    onWindowWantedChanged: windowWanted ? (windowAlive = true, windowUnload.stop()) : windowUnload.restart()
    Timer { id: windowUnload; interval: 600; onTriggered: root.windowAlive = false }

    LazyLoader {
        active: root.windowAlive

        GlassWindow {
            id: win
            readonly property int m: 24
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => root.shown ? 1 : 0)
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: root.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            name: "osd"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            visible: progress > 0
            WlrLayershell.layer: WlrLayer.Overlay
            anchors { top: true; right: true }
            margins.top: 30 + 6 - m
            margins.right: 8 - m
            implicitWidth: 280 + 2 * m
            implicitHeight: 52 + 2 * m
            mask: Region {}

            Glass {
                id: glass
                anchors.fill: parent
                radius: 26
                bevel: 18
                shadowMargin: win.m
                opacity: win.progress
                scale: 0.94 + 0.06 * win.progress
            }

            Row {
                x: win.m + 16
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12
                opacity: win.progress

                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: root.icon
                    fill: true
                    size: 20
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 5
                    Text {
                        text: root.label
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                    }
                    Rectangle {
                        visible: root.kind !== "mic"
                        width: 216
                        height: 6
                        radius: 3
                        color: Theme.color.toggleOff
                        Rectangle {
                            width: parent.width * Math.max(0, Math.min(1, root.value))
                            height: parent.height
                            radius: 3
                            color: Theme.color.label
                            Behavior on width { NumberAnimation { duration: Theme.motion.hover } }
                        }
                    }
                }
            }
        }
    }

    // ── Over a fullscreen app: a thin bar centred along the top edge ──────
    // (a 5 px level on a faint track, a small icon before it); fades in, and
    // out after 1.2 s without a change
    property string barKind: "volume"
    property bool barShown: false
    Timer { id: barHide; interval: 1200; onTriggered: root.barShown = false }
    readonly property real barValue: barKind === "brightness" ? Brightness.value : barKind === "mic" ? (Audio.micMuted ? 0 : 1) : (Audio.muted ? 0 : Audio.volume)
    readonly property string barIcon: barKind === "brightness" ? "sun" : barKind === "mic" ? (Audio.micMuted ? "microphone-slash" : "microphone") : Audio.icon
    property bool barAlive: false
    onBarShownChanged: barShown ? (barAlive = true, barUnload.stop()) : barUnload.restart()
    Timer { id: barUnload; interval: 400; onTriggered: root.barAlive = false }

    LazyLoader {
        active: root.barAlive

        PanelWindow {
            id: fsBar
            property real fade: 0
            Component.onCompleted: fade = Qt.binding(() => root.barShown ? 1 : 0)
            Behavior on fade {
                NumberAnimation { duration: root.barShown ? 150 : 100; easing.type: root.barShown ? Easing.OutCubic : Easing.InCubic }
            }
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            visible: fade > 0
            WlrLayershell.namespace: "irice:fshud"
            WlrLayershell.layer: WlrLayer.Overlay
            exclusionMode: ExclusionMode.Ignore
            color: "transparent"
            anchors { top: true }
            implicitWidth: Math.round((screen?.width ?? 1920) * 0.3) + 32
            implicitHeight: 24
            mask: Region {}

            Row {
                id: barRow
                anchors.horizontalCenter: parent.horizontalCenter
                y: 6 - (Config.options.appearance.reduceMotion === true ? 0 : Math.round((1 - fsBar.fade) * 4))
                spacing: 8
                opacity: fsBar.fade
                // A soft dark halo so it reads over bright video too
                layer.enabled: true
                layer.effect: MultiEffect {
                    shadowEnabled: true
                    shadowColor: Qt.rgba(0, 0, 0, 0.55)
                    shadowBlur: 0.35
                    shadowVerticalOffset: 0
                    shadowHorizontalOffset: 0
                }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: root.barIcon
                    fill: true
                    size: 12
                    color: "#F5F5F7"
                }
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.round((fsBar.screen?.width ?? 1920) * 0.3) - 20
                    height: 5
                    radius: 2.5
                    color: Qt.rgba(0, 0, 0, 0.35)
                    border.width: 1
                    border.color: Qt.rgba(1, 1, 1, 0.22)
                    Rectangle {
                        width: Math.max(parent.height, parent.width * Math.max(0, Math.min(1, root.barValue)))
                        visible: root.barValue > 0
                        height: parent.height
                        radius: 2.5
                        color: "#F5F5F7"
                        Behavior on width { NumberAnimation { duration: 100; easing.type: Easing.OutCubic } }
                    }
                }
            }
            Accessible.role: Accessible.ProgressBar
            Accessible.name: root.barKind === "brightness" ? "Brightness" : root.barKind === "mic" ? "Microphone" : "Volume"
        }
    }

    // Shows the HUD without changing anything: qs ipc call osd preview volume
    IpcHandler {
        target: "osd"
        function preview(kind: string): void {
            root.armed = true;
            root.show(kind);
        }
    }

    // Media keys: bind XF86 keys to hl.dsp.global("irice:<name>")
    GlobalShortcut { appid: "irice"; name: "volumeUp"; onPressed: { Audio.setVolume(Audio.volume + 0.05); Sounds.play("volumeKeys"); } }
    GlobalShortcut { appid: "irice"; name: "volumeDown"; onPressed: { Audio.setVolume(Audio.volume - 0.05); Sounds.play("volumeKeys"); } }
    GlobalShortcut { appid: "irice"; name: "volumeMute"; onPressed: Audio.toggleMute() }
    GlobalShortcut { appid: "irice"; name: "micMute"; onPressed: Audio.toggleMic() }
    GlobalShortcut { appid: "irice"; name: "brightnessUp"; onPressed: if (Brightness.available) Brightness.set(Brightness.value + 0.05) } // (no backlight: no OSD either)
    GlobalShortcut { appid: "irice"; name: "brightnessDown"; onPressed: if (Brightness.available) Brightness.set(Brightness.value - 0.05) }
}
