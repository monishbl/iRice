import QtQuick
import qs.common

// Control Center's brightness and volume, like iOS: the whole module is the
// slider, a capsule that fills with white. Wide (4×1) it fills from the left
// with the icon inside at the left; tall (1×2) it rises from the bottom with
// the icon at the bottom. Drag, click or scroll to change; right-click or
// press and hold for its page (hasMore).
Item {
    id: root

    property string icon
    property real value: 0
    property bool hasMore: false
    readonly property bool vertical: height > width
    property string accessibleName: icon
    signal moved(real value)
    signal more

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
        radius: root.thick / 2 + 4
    }

    readonly property real v: Math.max(0, Math.min(1, value))
    readonly property real thick: vertical ? width : height // the capsule's round-end diameter

    Item {
        id: track
        anchors.fill: parent
        // Round ends: the fill is clipped to the capsule
        Squircle {
            id: shape
            anchors.fill: parent
            radius: root.thick / 2
            color: Theme.color.toggleOff
        }
        Squircle {
            id: fillBar
            // Never shorter than the round end, so it stays a capsule
            property real len: Math.max(root.thick, (root.vertical ? parent.height : parent.width) * root.v)
            x: 0
            y: root.vertical ? parent.height - len : 0
            width: root.vertical ? parent.width : len
            height: root.vertical ? len : parent.height
            radius: root.thick / 2
            color: Theme.dark ? Qt.rgba(1, 1, 1, 0.92) : "white"
            Behavior on len {
                enabled: !drag.pressed
                NumberAnimation { duration: Theme.motion.hover }
            }
        }
    }

    Icon {
        readonly property real inset: Math.round((root.thick - size) / 2)
        x: root.vertical ? (parent.width - width) / 2 : inset
        y: root.vertical ? parent.height - height - inset : (parent.height - height) / 2
        name: root.icon
        fill: true
        size: Math.round(Math.min(22, root.thick * 0.36))
        color: Theme.accentOnGlyph // it always sits on the white fill (the fill is never shorter than the round end): accent, like the toggles
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
            root.moved(Math.max(0, Math.min(1, root.vertical ? 1 - m.y / height : m.x / width)));
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
