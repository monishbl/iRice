import QtQuick
import QtQuick.Controls as QQC
import qs.common

// A toast at the bottom of the Settings window: what just changed, with Undo
// for 8 s (5 s without one). show(text, undo) with undo a function or null;
// a new one replaces the last. Ctrl+Z undoes while it's up.
// Material: a snackbar (inverse surface, 4 px corners, the action in
// inversePrimary), rising 8 px as it fades in.
Item {
    id: root

    property string text
    property var undoAction: null
    readonly property bool shown: pop.visible

    width: 0
    height: 0

    function show(message, undo) {
        text = message;
        undoAction = undo ?? null;
        timer.interval = undo ? 8000 : 5000;
        timer.restart();
        pop.open();
    }
    function hide() {
        timer.stop();
        pop.close();
    }
    function undo() {
        const fn = undoAction;
        undoAction = null;
        hide();
        if (fn)
            fn();
    }

    Timer {
        id: timer
        onTriggered: pop.close()
    }
    Shortcut {
        sequence: "Ctrl+Z"
        enabled: pop.visible && root.undoAction !== null
        onActivated: root.undo()
    }

    QQC.Popup {
        id: pop
        parent: QQC.Overlay.overlay
        x: Math.round(((parent?.width ?? 0) - width) / 2)
        y: (parent?.height ?? 0) - height - 24 + (Theme.material ? rise : 0)
        property real rise: 0
        padding: 0
        modal: false
        focus: false
        closePolicy: QQC.Popup.NoAutoClose
        enter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
            NumberAnimation { property: "rise"; from: 8; to: 0; duration: 200; easing.type: Easing.OutCubic }
        }
        exit: Transition {
            NumberAnimation { property: "opacity"; to: 0; duration: 140; easing.type: Easing.InCubic }
        }
        // Inverse surface in light mode; in dark mode a raised one, a step
        // lighter than the (dark, translucent) window, ringed instead of shadowed
        background: Squircle {
            radius: Theme.material ? 4 : 14
            color: Theme.material ? Theme.m3.inverseSurface : Theme.dark ? "#3A3A40" : "#18181B"
            border.width: Theme.dark && !Theme.material ? 1 : 0
            border.color: Qt.rgba(1, 1, 1, 0.16)
        }
        contentItem: Item {
            implicitWidth: Theme.material ? Math.max(288, Math.min(560, message.implicitWidth + (undoButton.visible ? undoButton.width + 12 : 0) + 32)) : Math.min(520, message.implicitWidth + (undoButton.visible ? undoButton.width + 12 : 0) + 32)
            implicitHeight: Theme.material ? 48 : 44
            Accessible.role: Accessible.AlertMessage
            Accessible.name: root.text
            Text {
                id: message
                anchors { left: parent.left; leftMargin: 16; right: undoButton.visible ? undoButton.left : parent.right; rightMargin: undoButton.visible ? 12 : 16; verticalCenter: parent.verticalCenter }
                text: root.text
                elide: Text.ElideRight
                color: Theme.material ? Theme.m3.inverseOnSurface : Theme.dark ? "#EDEDEF" : "#FAFAFA"
                font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; letterSpacing: Theme.material ? 0.25 : 0 }
            }
            Squircle {
                id: undoButton
                visible: root.undoAction !== null
                anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                width: undoLabel.implicitWidth + (Theme.material ? 24 : 20)
                height: Theme.material ? 40 : 28
                radius: Theme.material ? 20 : Theme.radius.control
                color: Theme.material ? Qt.alpha(Theme.m3.inversePrimary, undoMouse.pressed ? 0.1 : undoMouse.containsMouse ? 0.08 : 0)
                    : undoMouse.pressed ? Qt.rgba(1, 1, 1, 0.2) : undoMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
                scale: undoMouse.pressed && !Theme.material ? 0.97 : 1
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                Accessible.role: Accessible.Button
                Accessible.name: "Undo"
                Accessible.onPressAction: root.undo()
                Text {
                    id: undoLabel
                    anchors.centerIn: parent
                    text: "Undo"
                    color: Theme.material ? Theme.m3.inversePrimary : Theme.dark ? "#EDEDEF" : "#FAFAFA"
                    font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
                }
                MouseArea {
                    id: undoMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.undo()
                }
            }
        }
    }
}
