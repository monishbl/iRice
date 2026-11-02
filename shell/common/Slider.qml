import QtQuick

// Thin macOS-style slider with a round knob. Emits moved() while dragging.
Item {
    id: root

    property real from: 0
    property real to: 1
    property real stepSize: 0
    property real value: 0
    readonly property alias pressed: area.pressed
    property string accessibleName
    signal moved(real value)

    // Keyboard: arrows step (a stepSize, or 1/20 of the range), Page Up and
    // Down take bigger steps, Home and End go to the ends
    activeFocusOnTab: true
    function nudge(target) {
        let v = Math.max(Math.min(root.from, root.to), Math.min(Math.max(root.from, root.to), target));
        if (stepSize > 0)
            v = Math.round(v / stepSize) * stepSize;
        if (v !== value)
            moved(v);
    }
    readonly property real keyStep: stepSize > 0 ? stepSize : (to - from) / 20
    Keys.onPressed: event => {
        const k = event.key;
        const v = k === Qt.Key_Left || k === Qt.Key_Down ? value - keyStep : k === Qt.Key_Right || k === Qt.Key_Up ? value + keyStep
            : k === Qt.Key_PageDown ? value - keyStep * 5 : k === Qt.Key_PageUp ? value + keyStep * 5
            : k === Qt.Key_Home ? from : k === Qt.Key_End ? to : NaN;
        if (!isNaN(v)) {
            nudge(v);
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.Slider
    Accessible.name: accessibleName
    Accessible.description: String(Math.round(value * 100) / 100)
    Accessible.onIncreaseAction: nudge(value + keyStep)
    Accessible.onDecreaseAction: nudge(value - keyStep)
    FocusRing {
        radius: height / 2 + 4
    }

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
        preventStealing: true // a scrolling page around it mustn't take the drag when the pointer drifts up or down
        onPressed: m => set(m.x)
        onPressedChanged: Panels.holdPull += pressed ? 1 : -1 // the panel holds still while dragging
        Component.onDestruction: if (pressed) Panels.holdPull = Math.max(0, Panels.holdPull - 1)
        onPositionChanged: m => { if (pressed) set(m.x); }
    }
}
