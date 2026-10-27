import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// "Installing update": a full-screen screen on every display while
// update-run.sh stops the shell, updates, rebuilds and starts the new shell.
// A config of its own (run from a copy, with qs -p), so it stays up while the
// shell restarts. It reads ../status, written by update-run.sh:
//   state \t percent \t text \t detail[\x1f backup folder]
// phase: run | restart | done (fade out, quit) | error (wait for Close)
ShellRoot {
    id: root

    property string phase: "run"
    property real percent: 0
    property string step: "Preparing"
    property string detail: ""
    property string backup: ""
    property bool leaving: false
    property real shown: 0
    Component.onCompleted: shown = 1
    Behavior on shown { NumberAnimation { duration: root.leaving ? 400 : 300; easing.type: root.leaving ? Easing.InCubic : Easing.OutCubic } }
    Behavior on percent { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }

    FileView {
        id: status
        path: Quickshell.shellDir + "/../status"
        blockLoading: true
        onLoaded: {
            const f = text().trim().split("\t");
            if (f.length < 3)
                return;
            root.phase = f[0];
            root.percent = Math.max(root.percent, Math.min(100, parseFloat(f[1]) || 0));
            root.step = f[2];
            const d = (f[3] ?? "").split("\x1f");
            root.detail = d[0] ?? "";
            root.backup = (d[1] ?? "").replace(Quickshell.env("HOME"), "~");
            if (root.phase === "done" && !root.leaving)
                finish.start();
        }
    }
    // (the file is replaced on each step, so it's read again rather than watched)
    Timer {
        running: root.phase !== "done" && root.phase !== "error"
        interval: 250
        repeat: true
        onTriggered: status.reload()
    }
    // A last way out if the updater itself is gone
    Timer {
        running: root.phase !== "error"
        interval: 15 * 60 * 1000
        onTriggered: Qt.quit()
    }
    SequentialAnimation {
        id: finish
        PauseAnimation { duration: 600 } // "Update installed", briefly
        ScriptAction { script: { root.leaving = true; root.shown = 0; } }
        PauseAnimation { duration: 450 }
        ScriptAction { script: Qt.quit() }
    }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: win
            required property var modelData
            screen: modelData
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice:update"
            WlrLayershell.keyboardFocus: root.phase === "error" ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            color: "transparent"

            Rectangle {
                anchors.fill: parent
                color: "#0A0A0B"
                opacity: root.shown

                Column {
                    anchors.centerIn: parent
                    width: Math.min(360, parent.width - 64)
                    spacing: 0
                    // (moves up a little as it appears)
                    transform: Translate { y: (1 - root.shown) * 8 }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "iRice"
                        color: "#EDEDEF"
                        font { family: "Inter"; pixelSize: 42; weight: Font.DemiBold; letterSpacing: -1 }
                    }
                    Item { width: 1; height: 32 }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.phase === "error" ? root.step : root.phase === "done" ? "Update installed" : root.phase === "restart" ? "Restarting iRice" : "Installing update"
                        color: "#EDEDEF"
                        font { family: "Inter"; pixelSize: 16; weight: Font.Medium }
                    }
                    Item { width: 1; height: 16 }

                    // Progress
                    Rectangle {
                        visible: root.phase !== "error"
                        width: parent.width
                        height: 6
                        radius: 3
                        color: "#2C2C31"
                        Rectangle {
                            width: Math.max(height, parent.width * root.percent / 100)
                            height: parent.height
                            radius: 3
                            color: "#EDEDEF"
                        }
                    }
                    Item { visible: root.phase !== "error"; width: 1; height: 12 }
                    Text {
                        visible: root.phase !== "error"
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.phase === "done" ? "Your settings and shortcuts are as you left them." : root.step + "…"
                        color: "#A1A1AA"
                        font { family: "Inter"; pixelSize: 13 }
                    }

                    // Something went wrong: what, that nothing was lost, and Close
                    Text {
                        visible: root.phase === "error"
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: root.detail
                        color: "#A1A1AA"
                        font { family: "Inter"; pixelSize: 13 }
                    }
                    Item { visible: root.phase === "error"; width: 1; height: 8 }
                    Text {
                        visible: root.phase === "error"
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        text: "Nothing was lost." + (root.backup !== "" ? " Your settings and shortcuts were backed up to " + root.backup + "." : "")
                        color: "#A1A1AA"
                        font { family: "Inter"; pixelSize: 13 }
                    }
                    Item { visible: root.phase === "error"; width: 1; height: 24 }
                    Rectangle {
                        id: close
                        visible: root.phase === "error"
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: closeText.implicitWidth + 40
                        height: 36
                        radius: 10
                        color: closeMouse.pressed ? "#C7C7CC" : closeMouse.containsMouse ? "#DADADF" : "#EDEDEF"
                        scale: closeMouse.pressed ? 0.97 : 1
                        Behavior on scale { NumberAnimation { duration: 100 } }
                        Text {
                            id: closeText
                            anchors.centerIn: parent
                            text: "Close"
                            color: "#18181B"
                            font { family: "Inter"; pixelSize: 14; weight: Font.Medium }
                        }
                        MouseArea {
                            id: closeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: Qt.quit()
                        }
                    }
                }

                Item {
                    anchors.fill: parent
                    focus: root.phase === "error"
                    Keys.onReturnPressed: Qt.quit()
                    Keys.onEscapePressed: Qt.quit()
                }
            }
        }
    }
}
