import QtQuick
import QtQuick.Controls as QQC
import qs.common

// A toast at the bottom of the Settings window: what just changed, with Undo
// for 8 s (5 s without one). show(text, undo) with undo a function or null;
// a new one replaces the last. Ctrl+Z undoes while it's up.
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
        y: (parent?.height ?? 0) - height - 24
        padding: 0
        modal: false
        focus: false
        closePolicy: QQC.Popup.NoAutoClose
        enter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
        }
        exit: Transition {
            NumberAnimation { property: "opacity"; to: 0; duration: 140; easing.type: Easing.InCubic }
        }
        // Inverse surface in light mode; in dark mode a raised one, a step
        // lighter than the (dark, translucent) window, ringed instead of shadowed
        background: Squircle {
            radius: 14
            color: Theme.dark ? "#3A3A40" : "#18181B"
            border.width: Theme.dark ? 1 : 0
            border.color: Qt.rgba(1, 1, 1, 0.16)
        }
        contentItem: Item {
            implicitWidth: Math.min(520, message.implicitWidth + (undoButton.visible ? undoButton.width + 12 : 0) + 32)
            implicitHeight: 44
            Accessible.role: Accessible.AlertMessage
            Accessible.name: root.text
            Text {
                id: message
                anchors { left: parent.left; leftMargin: 16; right: undoButton.visible ? undoButton.left : parent.right; rightMargin: undoButton.visible ? 12 : 16; verticalCenter: parent.verticalCenter }
                text: root.text
                elide: Text.ElideRight
                color: Theme.dark ? "#EDEDEF" : "#FAFAFA"
                font { family: Theme.font.family; pixelSize: Theme.font.body }
            }
            Squircle {
                id: undoButton
                visible: root.undoAction !== null
                anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                width: undoLabel.implicitWidth + 20
                height: 28
                radius: Theme.radius.control
                color: undoMouse.pressed ? Qt.rgba(1, 1, 1, 0.2) : undoMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
                scale: undoMouse.pressed ? 0.97 : 1
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                Accessible.role: Accessible.Button
                Accessible.name: "Undo"
                Accessible.onPressAction: root.undo()
                Text {
                    id: undoLabel
                    anchors.centerIn: parent
                    text: "Undo"
                    color: Theme.dark ? "#EDEDEF" : "#FAFAFA"
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
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
