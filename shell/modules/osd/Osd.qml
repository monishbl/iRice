import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Volume / brightness HUD: a small glass capsule under the menu bar, shown for
// a moment whenever either changes. Also owns the media-key shortcuts.
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
    GlobalShortcut { appid: "irice"; name: "brightnessUp"; onPressed: Brightness.set(Brightness.value + 0.05) }
    GlobalShortcut { appid: "irice"; name: "brightnessDown"; onPressed: Brightness.set(Brightness.value - 0.05) }
}
