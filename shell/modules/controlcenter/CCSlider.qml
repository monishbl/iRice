import QtQuick
import qs.common

// Tahoe-style slider: a thick rounded track that fills with the label colour,
// icon inside at the left. Drag, click or scroll to change.
Item {
    id: root

    property string label
    property string icon
    property real value: 0
    property bool hasMore: false
    signal moved(real value)
    signal more

    implicitHeight: (label !== "" ? title.implicitHeight + 8 : 0) + track.height

    Icon {
        visible: root.hasMore
        anchors { right: parent.right; top: parent.top }
        name: "caret-right"
        bold: true
        size: 12
        color: Theme.color.secondary
        MouseArea {
            anchors { fill: parent; margins: -8 }
            cursorShape: Qt.PointingHandCursor
            onClicked: root.more()
        }
    }

    Text {
        id: title
        visible: root.label !== ""
        text: root.label
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
    }

    Rectangle {
        id: track
        anchors.bottom: parent.bottom
        width: parent.width
        height: 26
        radius: height / 2
        color: Theme.color.toggleOff
        clip: true

        Rectangle {
            id: fillBar
            height: parent.height
            width: Math.max(parent.height, parent.width * Math.max(0, Math.min(1, root.value)))
            radius: height / 2
            color: Theme.dark ? Qt.rgba(1, 1, 1, 0.92) : "white"
            Behavior on width {
                enabled: !drag.pressed
                NumberAnimation { duration: Theme.motion.hover }
            }
        }

        Icon {
            x: 8
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            fill: true
            size: 14
            color: Qt.rgba(0, 0, 0, 0.6)
        }

        MouseArea {
            id: drag
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            function update(x) { root.moved(Math.max(0, Math.min(1, x / width))); }
            preventStealing: true // a scrolling page around it mustn't take the drag when the pointer drifts up or down
            onPressed: m => update(m.x)
            onPressedChanged: Panels.holdPull += pressed ? 1 : -1 // the panel holds still while dragging
            Component.onDestruction: if (pressed) Panels.holdPull = Math.max(0, Panels.holdPull - 1)
            onPositionChanged: m => { if (pressed) update(m.x); }
            onWheel: w => root.moved(Math.max(0, Math.min(1, root.value + (w.angleDelta.y > 0 ? 0.05 : -0.05))))
        }
    }
}
