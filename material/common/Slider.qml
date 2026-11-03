import QtQuick

// Slider: macOS's thin track with a round knob, or Material 3 Expressive's in
// the Material look (a 16 px track split by a thin handle with a gap on each
// side, and a stop dot at the end). Emits moved() while dragging.
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
    implicitHeight: Theme.material ? 32 : 22

    readonly property real ratio: Math.max(0, Math.min(1, (value - from) / (to - from)))

    // ── Material ──
    Loader {
        anchors.fill: parent
        active: Theme.material
        sourceComponent: Item {
            id: m
            readonly property real track: 16
            readonly property real gap: 6
            property real handleWidth: area.pressed || root.activeFocus ? 2 : 4
            readonly property real hx: root.ratio * (width - handleWidth) // the handle's left edge
            Behavior on handleWidth { NumberAnimation { duration: 100 } }
            // done part
            Rectangle {
                visible: width > 0
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(0, m.hx - m.gap)
                height: m.track
                topLeftRadius: 8
                bottomLeftRadius: 8
                topRightRadius: 2
                bottomRightRadius: 2
                color: root.enabled ? Theme.m3.primary : Qt.alpha(Theme.m3.onSurface, 0.38)
            }
            // the rest, with its stop dot
            Rectangle {
                visible: width > 0
                x: m.hx + m.handleWidth + m.gap
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(0, m.width - x)
                height: m.track
                topLeftRadius: 2
                bottomLeftRadius: 2
                topRightRadius: 8
                bottomRightRadius: 8
                color: root.enabled ? Theme.m3.secondaryContainer : Qt.alpha(Theme.m3.onSurface, 0.12)
                Rectangle {
                    visible: parent.width > 12
                    anchors.right: parent.right
                    anchors.rightMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    width: 4
                    height: 4
                    radius: 2
                    color: Theme.m3.primary
                }
            }
            Rectangle {
                x: m.hx
                anchors.verticalCenter: parent.verticalCenter
                width: m.handleWidth
                height: m.height
                radius: width / 2
                color: root.enabled ? Theme.m3.primary : Qt.alpha(Theme.m3.onSurface, 0.38)
            }
        }
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
