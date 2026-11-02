import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass

// A small glass tip under something pointed at for a moment (the menu bar's
// battery, the island's): one line of text. It takes no input, so the
// pointer stays on what it's over. Like ResourceHover, the target's window
// sits at the screen's top left, so window coordinates are screen ones.
//   HoverTip { target: item; hovered: item.hovered; screen: bar.screen; text: "84% · Balanced" }
Item {
    id: root

    property Item target: parent
    property bool hovered: false
    property var screen
    property string text
    property int gap: 8 // below the target
    visible: false // takes no room; the tip is its own window

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
        GlassWindow {
            id: tip
            readonly property int m: 24 // shadow margin
            screen: root.screen
            name: "hover-tip"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            anchors { top: true; left: true }
            margins {
                left: Math.round(Math.max(0, Math.min((screen?.width ?? 0) - width, root.at.x - width / 2)))
                top: Math.round(Math.max(0, root.at.y - m))
            }
            implicitWidth: label.implicitWidth + 24 + 2 * m
            implicitHeight: label.implicitHeight + 14 + 2 * m
            mask: Region {}

            // Fades in and drops 4px
            property real shown: 0
            Component.onCompleted: shown = 1
            Behavior on shown { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }

            Item {
                anchors.fill: parent
                opacity: tip.shown
                transform: Translate { y: (tip.shown - 1) * 4 }
                Glass {
                    anchors.fill: parent
                    radius: Theme.radius.control
                    shadowMargin: tip.m
                    lensScale: 0.35
                }
                Text {
                    id: label
                    anchors.centerIn: parent
                    text: root.text
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                }
            }
        }
    }
}
