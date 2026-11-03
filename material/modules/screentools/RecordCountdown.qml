import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// The seconds before a recording starts (Options → Countdown): a glass disc
// (Material: a morphing shape) in the middle of the focused screen, clicks go through. Esc, or Stop in
// the island, cancels it (Recorder.stop()).
Scope {
    LazyLoader {
        active: Recorder.countdown > 0

        GlassWindow {
            id: win
            screen: ScreenTools.focusedScreen()
            name: "recordCountdown"
            readonly property int m: 24
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            implicitWidth: 120 + 2 * m
            implicitHeight: 120 + 2 * m
            mask: Region {}

            Item {
                id: disc
                anchors.fill: parent
                // each second: a small pop
                property int shownCount: Recorder.countdown
                onShownCountChanged: pop.restart()
                SequentialAnimation {
                    id: pop
                    NumberAnimation { target: number; property: "scale"; from: 0.82; to: 1; duration: 220; easing.type: Easing.OutBack }
                }
                // Material: a hero shape in primaryContainer that morphs to
                // the next shape with each second, as Android's timers do
                Loader {
                    active: Theme.material
                    anchors.centerIn: parent
                    sourceComponent: MaterialShape {
                        width: 120
                        height: 120
                        readonly property var shapes: ["cookie9", "sunny", "cookie12", "clover8", "softBurst"]
                        shape: shapes[Math.max(0, Recorder.countdown) % shapes.length]
                        color: Theme.m3.primaryContainer
                    }
                }
                Text {
                    id: number
                    anchors.centerIn: parent
                    text: Recorder.countdown
                    color: Theme.material ? Theme.m3.onPrimaryContainer : Theme.color.label
                    font { family: Theme.font.family; pixelSize: 56; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
            }
        }
    }
}
