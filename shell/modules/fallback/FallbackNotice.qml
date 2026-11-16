import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.common

// The QML shell is the fallback (16 Nov: only the Rust shell, tessera-ui, is
// maintained). When launch-shell.sh or tessera-ui switched to it because the
// Rust shell crashed twice in a row or couldn't draw, they leave a note in
// $XDG_STATE_HOME/tessera/fallback: its first line is why. While it's there
// this shows a small watermark in the bottom-right corner of every screen,
// with "Try again", and sends one notification saying so. Try again clears
// the crash count and the note and starts a fresh shell (repair-shell.sh),
// which is the Rust one again.
Scope {
    id: root

    readonly property string dir: Paths.stateHome + "/tessera"
    readonly property string note: dir + "/fallback"
    property string reason: ""
    readonly property bool shown: reason !== ""

    FileView {
        path: root.note
        watchChanges: true
        printErrors: false
        onLoaded: root.reason = (text().split("\n")[0] || "the Rust shell stopped").trim()
        onLoadFailed: root.reason = ""
        onFileChanged: reload()
    }

    function tryAgain() {
        Quickshell.execDetached(["sh", "-c", "rm -f \"$1/fallback\" \"$1/standalone-starts\"; exec setsid -f \"$2\"", "sh", root.dir, Quickshell.shellDir + "/assets/scripts/repair-shell.sh"]);
    }

    // One notification per fallback (its note's time): the reason and the button
    // (a few seconds in: at start this shell's own notification server isn't
    // up yet, and notify-send would find none)
    property bool told: false
    Timer {
        running: root.shown && !root.told
        interval: 6000
        onTriggered: root.tell()
    }
    function tell() {
        told = true;
        Quickshell.execDetached(["sh", "-c", "a=$(timeout 3600 notify-send -a Tessera -u critical -i dialog-warning -A again='Try Rust again' 'Rust shell stopped, running the basic fallback' \"$1. The basic shell keeps you working; restart to go back, or try again now.\"); [ \"$a\" = again ] && { rm -f \"$2/fallback\" \"$2/standalone-starts\"; exec setsid -f \"$3\"; }", "sh", root.reason, root.dir, Quickshell.shellDir + "/assets/scripts/repair-shell.sh"]);
    }

    Variants {
        model: root.shown ? Quickshell.screens : []

        PanelWindow {
            id: mark
            required property var modelData
            screen: modelData
            color: "transparent"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "tessera:fallback"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            anchors { bottom: true; right: true }
            margins { bottom: 12; right: 12 }
            implicitWidth: pill.width
            implicitHeight: pill.height
            // (only the pill takes the pointer)
            mask: Region { item: pill }

            Rectangle {
                id: pill
                width: row.implicitWidth + 24
                height: 32
                radius: height / 2
                color: Qt.rgba(0.1, 0.1, 0.12, 0.72)
                border.width: 1
                border.color: Qt.rgba(1, 1, 1, 0.12)
                Row {
                    id: row
                    anchors.centerIn: parent
                    spacing: 10
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Rust shell stopped · basic fallback"
                        color: Qt.rgba(1, 1, 1, 0.78)
                        font { family: Theme.font.family; pixelSize: 12; weight: Font.Medium }
                    }
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: again.implicitWidth + 16
                        height: 22
                        radius: 11
                        color: tap.containsMouse ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.18)
                        Text {
                            id: again
                            anchors.centerIn: parent
                            text: "Try again"
                            color: "white"
                            font { family: Theme.font.family; pixelSize: 12; weight: Font.DemiBold }
                        }
                        MouseArea {
                            id: tap
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.tryAgain()
                        }
                    }
                }
            }
        }
    }
}
