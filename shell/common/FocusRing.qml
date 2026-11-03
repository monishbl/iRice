import QtQuick

// The keyboard focus ring: 2px in the accent, 2px outside its control
// (Material: 3px in the secondary colour). It
// shows only when focus came from the keyboard (Tab, arrows): a click that
// happens to focus the control doesn't ring it, as on macOS. Put it inside
// the control it rings:
//   FocusRing { radius: parent.radius + 4 }
// (Item.focusReason isn't available to QML, so a press on the control, seen
// by a passive handler that takes nothing from the control's own mouse
// handling, marks the next focus as the pointer's.)
Item {
    id: root

    property real radius: 8
    // True while the control has focus that came from the keyboard
    readonly property bool shown: keyboard && (parent?.activeFocus ?? false)

    property bool keyboard: false
    property real pressedAt: 0

    anchors { fill: parent; margins: Theme.material ? -5 : -4 } // (Material: 3px, 2px out)
    z: 100

    Connections {
        target: root.parent
        function onActiveFocusChanged() {
            root.keyboard = root.parent.activeFocus && Date.now() - root.pressedAt > 400;
        }
    }
    PointHandler {
        acceptedButtons: Qt.AllButtons
        onActiveChanged: if (active) {
            root.pressedAt = Date.now();
            root.keyboard = false;
        }
    }

    Squircle {
        anchors.fill: parent
        visible: root.shown
        radius: root.radius
        color: "transparent"
        border.width: Theme.material ? 3 : 2
        border.color: Theme.material ? Theme.m3.secondary : Theme.accent
    }
}
