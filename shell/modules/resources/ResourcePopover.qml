import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// A small glass popover under a hovered figure (the island's CPU, memory and
// temperature, the menu bar's resources item): every stat, grouped on rows
// (processor, graphics, memory, network) with a small graph each; the one
// pointed at is highlighted. It takes no input, so the pointer stays on
// what it's hovering. Shown by ResourceHover.
GlassWindow {
    id: root

    property string key: "cpu"
    property real anchorX: 0 // screen x it centres under
    property real anchorY: 0 // screen y of its top edge
    property Component networkExtra: null
    readonly property int m: 24 // shadow margin
    readonly property int pad: 14
    readonly property int contentWidth: 300
    // The stats, grouped: the one pointed at is highlighted
    readonly property string group: key === "cpu" || key === "temperature" ? "processor" : key === "gpu" ? "graphics" : key === "memory" || key === "swap" ? "memory" : key.startsWith("net") ? "network" : "processor"

    name: "resources-hover"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    anchors { top: true; left: true }
    margins {
        left: Math.round(Math.max(0, Math.min((screen?.width ?? 0) - width, anchorX - width / 2)))
        top: Math.round(Math.max(0, anchorY - m))
    }
    implicitWidth: contentWidth + 2 * pad + 2 * m
    implicitHeight: list.implicitHeight + 2 * pad + 2 * m
    mask: Region {}

    // Fades in and drops 6px
    property real shown: 0
    Component.onCompleted: shown = 1
    Behavior on shown { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }

    Item {
        anchors.fill: parent
        opacity: root.shown
        transform: Translate { y: (root.shown - 1) * 6 }

        Glass {
            anchors.fill: parent
            radius: Theme.radius.popover
            shadowMargin: root.m
            lensScale: 0.35 // a small menu-like surface (see Glass.lensScale)
        }
        Column {
            id: list
            x: root.m + root.pad
            y: root.m + root.pad
            width: root.contentWidth
            spacing: 2
            Repeater {
                model: Resources.groups
                ResourceRow {
                    required property string modelData
                    width: parent.width
                    group: modelData
                    highlighted: modelData === root.group
                }
            }
            // Network: room for a read-only speed test summary (this window
            // takes no input): networkExtra: Component { SpeedTestCard {} }
            Loader {
                width: parent.width
                active: root.group === "network" && root.networkExtra !== null
                visible: active
                sourceComponent: root.networkExtra
            }
        }
    }
}
