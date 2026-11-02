import QtQuick
import qs.common

// Text button for the picker's empty and error states: primary (accent) or
// secondary (fill). 36 px tall, squishes on press. Material: a pill, filled
// (primary) or tonal (secondary container), with state layers, its corners
// tightening while pressed.
Squircle {
    id: root

    property string text
    property bool primary: false
    signal clicked

    readonly property color m3Content: Theme.material ? (primary ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer) : "transparent"
    width: label.implicitWidth + (Theme.material ? 40 : 32)
    height: Theme.material ? 40 : 36
    radius: Theme.material ? (mouse.pressed ? 12 : height / 2) : Theme.radius.control
    Behavior on radius { enabled: Theme.material; SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
    color: Theme.material ? Qt.tint(primary ? Theme.m3.primary : Theme.m3.secondaryContainer, Qt.alpha(m3Content, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
        : primary ? (mouse.pressed ? Qt.darker(Theme.accent, 1.2) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent) : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
    scale: mouse.pressed ? (Theme.material ? 1 : 0.97) : 1
    Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        color: Theme.material ? root.m3Content : root.primary ? Theme.color.onAccent : Theme.color.label
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
