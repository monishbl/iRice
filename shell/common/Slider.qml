import QtQuick

// Thin macOS-style slider with a round knob. Emits moved() while dragging.
Item {
    id: root

    property real from: 0
    property real to: 1
    property real stepSize: 0
    property real value: 0
    readonly property alias pressed: area.pressed
    signal moved(real value)

    implicitWidth: 180
    implicitHeight: 22

    readonly property real ratio: Math.max(0, Math.min(1, (value - from) / (to - from)))

    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        height: 4
        radius: 2
        color: Theme.color.toggleOff
        Rectangle {
            width: parent.width * root.ratio
            height: parent.height
            radius: 2
            color: Theme.accent
        }
    }
    Rectangle {
        x: root.ratio * (root.width - width)
        anchors.verticalCenter: parent.verticalCenter
        width: 18
        height: 18
        radius: 9
        color: "white"
        border.width: 1
        border.color: Qt.rgba(0, 0, 0, 0.15)
    }
    MouseArea {
        id: area
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        function set(x) {
            let v = root.from + Math.max(0, Math.min(1, x / width)) * (root.to - root.from);
            if (root.stepSize > 0)
                v = Math.round(v / root.stepSize) * root.stepSize;
            root.moved(v);
        }
        onPressed: m => set(m.x)
        onPressedChanged: Panels.holdPull += pressed ? 1 : -1 // the panel holds still while dragging
        Component.onDestruction: if (pressed) Panels.holdPull = Math.max(0, Panels.holdPull - 1)
        onPositionChanged: m => { if (pressed) set(m.x); }
    }
}
