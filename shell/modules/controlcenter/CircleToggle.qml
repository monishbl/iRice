import QtQuick
import qs.common

// Round on/off button, like iOS and macOS 26: on is a white circle with the
// glyph in the accent colour (darkened when the accent is too light to read
// on white), off is translucent with a plain glyph.
Rectangle {
    id: root

    property string icon
    property bool checked: false
    property bool enabled_: true
    property int size: 32
    signal clicked

    width: size
    height: size
    radius: size / 2
    readonly property color ink: (0.2126 * Theme.accent.r + 0.7152 * Theme.accent.g + 0.0722 * Theme.accent.b) > 0.55 ? Qt.darker(Theme.accent, 1.7) : Theme.accent
    color: checked ? "white" : mouse.containsMouse ? Theme.color.fillHover : Theme.color.toggleOff
    opacity: enabled_ ? 1 : 0.4
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    transform: elastic.effects

    Icon {
        anchors.centerIn: parent
        name: root.icon
        fill: root.checked
        size: Math.round(root.size * 0.5)
        color: root.checked ? root.ink : Theme.color.label
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.enabled_
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    ElasticControl {
        id: elastic
        pressed: mouse.pressed
    }
}
