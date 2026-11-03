import QtQuick
import qs.common

// Quick Settings' brightness and volume (the Material look): Material 3
// Expressive's tall slider. The done part (primary) and the rest
// (secondaryContainer) are split by a thin handle with a gap on each side;
// the icon sits inside the track at the start, and moves past the handle
// when the done part is too short to hold it, as Android's does. Drag, click
// or scroll to change; right-click or press and hold for its page (hasMore).
// CCSlider uses it at the medium size in the Material look.
Item {
    id: root

    property string icon
    property real value: 0
    property bool hasMore: false
    property real trackHeight: 52 // Expressive's large slider (medium: 40)
    property string accessibleName: icon
    property real glyphRotation: 0 // (a standing slider is this one turned: its icon turns back)
    signal moved(real value)
    signal more

    implicitWidth: 240
    implicitHeight: trackHeight + 8 // (the handle stands out of the track)

    // Keyboard: arrows by 5%, Page Up/Down by 20%, Home/End to the ends,
    // Enter (with hasMore) for its page
    activeFocusOnTab: true
    function nudge(d) {
        root.moved(Math.max(0, Math.min(1, d)));
    }
    Keys.onPressed: event => {
        const k = event.key;
        const v = k === Qt.Key_Left || k === Qt.Key_Down ? value - 0.05 : k === Qt.Key_Right || k === Qt.Key_Up ? value + 0.05
            : k === Qt.Key_PageDown ? value - 0.2 : k === Qt.Key_PageUp ? value + 0.2
            : k === Qt.Key_Home ? 0 : k === Qt.Key_End ? 1 : NaN;
        if (!isNaN(v)) {
            nudge(v);
            event.accepted = true;
        } else if (hasMore && (k === Qt.Key_Return || k === Qt.Key_Enter)) {
            root.more();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.Slider
    Accessible.name: accessibleName
    Accessible.description: Math.round(v * 100) + "%"
    Accessible.onIncreaseAction: nudge(value + 0.05)
    Accessible.onDecreaseAction: nudge(value - 0.05)
    FocusRing {
        radius: root.corner + 5
    }

    readonly property real v: Math.max(0, Math.min(1, value))
    // What's drawn: follows the value, gliding when it changes from elsewhere
    // (a key, another app), straight under the pointer while dragging
    property real shown: v
    Behavior on shown {
        enabled: !drag.pressed
        NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic }
    }

    // Track corners: large 16, medium 12; 2 where it meets the handle's gap
    readonly property real corner: trackHeight >= 52 ? 16 : 12
    readonly property real gap: 6
    property real handleWidth: drag.pressed || root.activeFocus ? 2 : 4
    Behavior on handleWidth { NumberAnimation { duration: Theme.motion.press } }
    readonly property real hx: shown * (width - handleWidth) // the handle's left edge
    readonly property real trackY: (height - trackHeight) / 2

    // The done part
    Rectangle {
        visible: width > 0
        y: root.trackY
        width: Math.max(0, root.hx - root.gap)
        height: root.trackHeight
        topLeftRadius: root.corner
        bottomLeftRadius: root.corner
        topRightRadius: 2
        bottomRightRadius: 2
        color: root.enabled ? Theme.m3.primary : Qt.alpha(Theme.m3.onSurface, 0.38)
    }
    // The rest, with its stop dot
    Rectangle {
        visible: width > 0
        x: root.hx + root.handleWidth + root.gap
        y: root.trackY
        width: Math.max(0, root.width - x)
        height: root.trackHeight
        topLeftRadius: 2
        bottomLeftRadius: 2
        topRightRadius: root.corner
        bottomRightRadius: root.corner
        color: root.enabled ? Theme.m3.secondaryContainer : Qt.alpha(Theme.m3.onSurface, 0.12)
        Rectangle {
            visible: parent.width > root.trackHeight
            anchors { right: parent.right; rightMargin: (root.trackHeight - width) / 2; verticalCenter: parent.verticalCenter }
            width: 4
            height: 4
            radius: 2
            color: Theme.m3.onSecondaryContainer
        }
    }
    // The handle
    Rectangle {
        x: root.hx
        width: root.handleWidth
        height: root.height
        radius: width / 2
        color: root.enabled ? Theme.m3.primary : Qt.alpha(Theme.m3.onSurface, 0.38)
    }

    // The icon: in the done part while it fits, else just past the handle
    Icon {
        id: glyph
        size: Math.round(root.trackHeight * 0.42)
        readonly property real inset: Math.round((root.trackHeight - size) / 2)
        readonly property bool inside: root.hx - root.gap >= inset + size + 6
        x: inside ? inset : root.hx + root.handleWidth + root.gap + inset * 0.6
        Behavior on x {
            enabled: !drag.pressed
            NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic }
        }
        anchors.verticalCenter: parent.verticalCenter
        name: root.icon
        fill: true
        rotation: root.glyphRotation
        color: inside ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }

    MouseArea {
        id: drag
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        property bool held: false // pressed and held: opened the page instead
        property point start
        property real before // the value before the press (a hold puts it back)
        function update(m) {
            root.moved(Math.max(0, Math.min(1, m.x / width)));
        }
        preventStealing: true // a scrolling page around it mustn't take the drag when the pointer drifts up or down
        onPressed: m => {
            if (m.button === Qt.RightButton) {
                if (root.hasMore)
                    root.more();
                return;
            }
            held = false;
            start = Qt.point(m.x, m.y);
            before = root.value;
            if (root.hasMore)
                holdTimer.restart();
            update(m);
        }
        onReleased: holdTimer.stop()
        onPressedChanged: Panels.holdPull += pressed ? 1 : -1 // the panel holds still while dragging
        Component.onDestruction: if (pressed) Panels.holdPull = Math.max(0, Panels.holdPull - 1)
        onPositionChanged: m => {
            if (!pressed || held || !(m.buttons & Qt.LeftButton))
                return;
            if (Math.abs(m.x - start.x) + Math.abs(m.y - start.y) > 6)
                holdTimer.stop();
            update(m);
        }
        onWheel: w => root.moved(Math.max(0, Math.min(1, root.value + (w.angleDelta.y > 0 ? 0.05 : -0.05))))
        Timer {
            id: holdTimer
            interval: 550
            onTriggered: {
                drag.held = true;
                root.moved(drag.before);
                root.more();
            }
        }
    }
}
