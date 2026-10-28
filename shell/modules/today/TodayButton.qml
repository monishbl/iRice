import QtQuick
import qs.common

// A Today button: 36 px, verb + object, primary (accent) or secondary.
// Click, Enter or Space; a visible focus ring for the keyboard.
Rectangle {
    id: root

    property string text
    property string icon
    property bool primary: false
    signal activated

    implicitWidth: row.implicitWidth + 32
    implicitHeight: 36
    radius: Theme.radius.control
    color: primary ? (mouse.pressed ? Qt.darker(Theme.accent, 1.2) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent) : (mouse.pressed ? Theme.color.fillHover : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill)
    scale: mouse.pressed ? 0.97 : 1
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    Behavior on scale { NumberAnimation { duration: Theme.motion.press } }

    activeFocusOnTab: true
    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    // focus ring: 2 px accent, 2 px off the button
    Rectangle {
        anchors { fill: parent; margins: -4 }
        visible: root.activeFocus
        radius: root.radius + 4
        color: "transparent"
        border.width: 2
        border.color: Theme.accent
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Icon {
            visible: root.icon !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            size: 16
            color: root.primary ? Theme.color.onAccent : Theme.color.label
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            color: root.primary ? Theme.color.onAccent : Theme.color.label
            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
