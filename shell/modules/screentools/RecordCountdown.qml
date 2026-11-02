import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// The seconds before a recording starts (Options → Countdown): a glass disc
// in the middle of the focused screen, clicks go through. Esc, or Stop in
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
                Glass {
                    anchors.fill: parent
                    radius: 60
                    shadowMargin: win.m
                }
                Text {
                    id: number
                    anchors.centerIn: parent
                    text: Recorder.countdown
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: 56; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
            }
        }
    }
}
