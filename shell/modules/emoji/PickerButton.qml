import QtQuick
import qs.common

// Text button for the picker's empty and error states: primary (accent) or
// secondary (fill). 36 px tall, squishes on press.
Squircle {
    id: root

    property string text
    property bool primary: false
    signal clicked

    width: label.implicitWidth + 32
    height: 36
    radius: Theme.radius.control
    color: primary ? (mouse.pressed ? Qt.darker(Theme.accent, 1.2) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent) : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
    scale: mouse.pressed ? 0.97 : 1
    Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        color: root.primary ? Theme.color.onAccent : Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
