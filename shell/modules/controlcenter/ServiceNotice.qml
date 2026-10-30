import QtQuick
import qs.common

// In place of a list whose service isn't there (Wi-Fi without
// NetworkManager, Bluetooth stopped or blocked): what's going on, how to fix
// it, and one button when there's something to press. Shared by the
// menu bar menus, Control Center and Settings.
Column {
    id: root

    property string icon: "info"
    property string title
    property string text
    property string action // the button's label; "" = no button
    property bool busy: false
    property string error // (the button didn't work)
    signal triggered

    width: parent?.width ?? 0
    spacing: 8
    topPadding: 12
    bottomPadding: 12

    Icon {
        anchors.horizontalCenter: parent.horizontalCenter
        name: root.icon
        size: 24
        color: Theme.color.tertiary
    }
    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: root.title
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
    }
    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        visible: text !== ""
        text: root.text
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
    Item { width: 1; height: 4; visible: button.visible }
    Squircle {
        id: button
        visible: root.action !== ""
        anchors.horizontalCenter: parent.horizontalCenter
        width: label.implicitWidth + 28
        height: 30
        radius: Theme.radius.control
        opacity: root.busy ? 0.6 : 1
        color: mouse.pressed ? Qt.darker(Theme.accent, 1.15) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.07) : Theme.accent
        scale: mouse.pressed ? 0.97 : 1
        Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
        activeFocusOnTab: visible
        Keys.onPressed: event => {
            if (!root.busy && !event.isAutoRepeat && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
                root.triggered();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: root.action
        FocusRing { radius: button.radius + 4 }
        Text {
            id: label
            anchors.centerIn: parent
            text: root.busy ? "Working…" : root.action
            color: Theme.color.onAccent
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            enabled: !root.busy
            onClicked: root.triggered()
        }
    }
    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        visible: root.error !== ""
        text: root.error
        color: Theme.color.danger
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
}
