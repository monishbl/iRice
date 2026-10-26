import QtQuick
import qs.common

// Round on/off button: accent when on, translucent when off.
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
    color: checked ? Theme.accent : mouse.containsMouse ? Theme.color.fillHover : Theme.color.toggleOff
    opacity: enabled_ ? 1 : 0.4
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    transform: elastic.effects

    Icon {
        anchors.centerIn: parent
        name: root.icon
        fill: root.checked
        size: Math.round(root.size * 0.5)
        color: root.checked ? Theme.color.onAccent : Theme.color.label
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
