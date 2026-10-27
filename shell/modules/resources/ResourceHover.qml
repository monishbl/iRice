import QtQuick
import Quickshell

// Hovering a figure for a moment shows its ResourcePopover underneath:
//   ResourceHover { key: "cpu"; target: item; hovered: hover.hovered; screen: bar.screen }
// The target's window has to sit at the screen's top left (the menu bar's
// does), so window coordinates are screen coordinates.
Item {
    id: root

    property string key: "cpu"
    property Item target: parent
    property bool hovered: false
    property var screen
    property int gap: 8 // below the target
    // Takes no room (it can sit in a Row); the popover is its own window
    visible: false

    property bool open: false
    property point at: Qt.point(0, 0)
    onHoveredChanged: hovered ? (closeDelay.stop(), openDelay.restart()) : (openDelay.stop(), closeDelay.restart())
    Timer {
        id: openDelay
        interval: 500
        onTriggered: {
            const p = root.target.mapToItem(null, root.target.width / 2, root.target.height);
            root.at = Qt.point(p.x, p.y + root.gap);
            root.open = true;
        }
    }
    Timer {
        id: closeDelay
        interval: 120
        onTriggered: root.open = false
    }

    LazyLoader {
        active: root.open && root.screen !== null && root.screen !== undefined
        ResourcePopover {
            screen: root.screen
            key: root.key
            anchorX: root.at.x
            anchorY: root.at.y
        }
    }
}
