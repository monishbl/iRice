import QtQuick
import qs.common
import qs.services

// The sidebar's bar for a chat just deleted (with Undo, while Ai offers it)
// or something to say about past chats. Inverse, like Settings' toast;
// Material: a snackbar (inverse surface, 4 px corners, the action in
// inversePrimary). Fades in rising 8 px.
Squircle {
    id: root

    readonly property string message: Ai.pendingDelete ? "Deleted “" + Ai.pendingDelete.entry.title + "”" : Ai.notice
    readonly property bool shown: message !== ""
    readonly property color ink: Theme.material ? Theme.m3.inverseOnSurface : Theme.dark ? "#EDEDEF" : "#FAFAFA"

    height: Math.max(Theme.material ? 48 : 44, label.implicitHeight + 24)
    radius: Theme.material ? 4 : 14
    color: Theme.material ? Theme.m3.inverseSurface : Theme.dark ? "#3A3A40" : "#18181B"
    border.width: Theme.dark && !Theme.material ? 1 : 0
    border.color: Qt.rgba(1, 1, 1, 0.16)
    visible: opacity > 0
    opacity: shown ? 1 : 0
    property real rise: shown ? 0 : 8
    transform: Translate { y: root.rise }
    Behavior on opacity { NumberAnimation { duration: root.shown ? 200 : 140; easing.type: root.shown ? Easing.OutCubic : Easing.InCubic } }
    Behavior on rise { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
    Accessible.role: Accessible.AlertMessage
    Accessible.name: message

    Text {
        id: label
        anchors { left: parent.left; leftMargin: 16; right: undoButton.visible ? undoButton.left : parent.right; rightMargin: undoButton.visible ? 8 : 16; verticalCenter: parent.verticalCenter }
        text: root.message
        // (a deleted chat's title is cut short; a note wraps)
        elide: Ai.pendingDelete ? Text.ElideRight : Text.ElideNone
        wrapMode: Ai.pendingDelete ? Text.NoWrap : Text.Wrap
        color: root.ink
        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; letterSpacing: Theme.material ? 0.25 : 0 }
    }
    Squircle {
        id: undoButton
        visible: Ai.pendingDelete !== null
        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
        width: undoLabel.implicitWidth + (Theme.material ? 24 : 20)
        height: Theme.material ? 40 : 28
        radius: Theme.material ? 20 : Theme.radius.control
        color: Theme.material ? Qt.alpha(Theme.m3.inversePrimary, undoMouse.pressed ? 0.1 : undoMouse.containsMouse ? 0.08 : 0)
            : undoMouse.pressed ? Qt.rgba(1, 1, 1, 0.2) : undoMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
        transform: Theme.material ? pressFx.effects : [] // (Glass presses by scale)
        ElasticControl { id: pressFx; pressed: undoMouse.pressed }
        scale: undoMouse.pressed && !Theme.material ? 0.97 : 1
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        activeFocusOnTab: visible
        Keys.onPressed: e => {
            if (!e.isAutoRepeat && (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter)) {
                Ai.undoDelete();
                e.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: "Undo delete"
        Accessible.onPressAction: Ai.undoDelete()
        FocusRing { radius: undoButton.radius + 4 }
        Text {
            id: undoLabel
            anchors.centerIn: parent
            text: "Undo"
            color: Theme.material ? Theme.m3.inversePrimary : root.ink
            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
        }
        MouseArea {
            id: undoMouse
            anchors { fill: parent; topMargin: -8; bottomMargin: -8 } // (44 px to hit)
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: Ai.undoDelete()
        }
    }
}
