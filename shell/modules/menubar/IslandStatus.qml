import QtQuick
import Quickshell
import Quickshell.Services.UPower
import qs.common
import qs.modules.resources
import qs.services

// The quiet last line of the expanded island: battery, CPU (with the live
// package temperature, reddish when running hot) and memory. (Today's events
// are in Today: the morning screen and its widget.) Staggers in after the activities. Hovering CPU, memory or
// the temperature shows its last minute in a glass popover (ResourceHover).
Item {
    id: root

    property int delay: 0
    property bool shown: false
    implicitHeight: 22

    // Make sure CPU and memory are sampled while this shows them
    property bool counted: false
    onShownChanged: {
        if (shown !== counted) {
            Resources.desktopUsers += shown ? 1 : -1;
            counted = shown;
        }
    }
    Component.onDestruction: if (counted) Resources.desktopUsers--

    property real appear: shown ? 1 : 0
    Behavior on appear {
        SequentialAnimation {
            PauseAnimation { duration: root.shown ? root.delay : 0 }
            NumberAnimation { duration: 280; easing.type: Easing.OutCubic }
        }
    }
    opacity: appear
    transform: Translate { y: (1 - root.appear) * 8 }

    Row {
        id: stats
        anchors.verticalCenter: parent.verticalCenter
        spacing: 14
        // By name, so the figures (and an open popover) stay put while the
        // numbers change; each figure reads its own value
        readonly property string figures: (Island.hasBattery ? "battery," : "") + "cpu," + (Resources.temperature > 0 ? "temperature," : "") + "memory"
        Repeater {
            model: stats.figures.split(",")
            Row {
                id: figure
                required property string modelData
                readonly property bool isStat: modelData !== "battery"
                readonly property bool hot: modelData === "temperature" && Island.hot
                readonly property color tint: hot ? Qt.rgba(1, 0.55, 0.5, 0.85) : Qt.rgba(1, 1, 1, 0.6)
                spacing: 4
                HoverHandler {
                    id: figureHover
                }
                ResourceHover {
                    target: figure
                    key: figure.isStat ? figure.modelData : "cpu"
                    hovered: figureHover.hovered && root.shown && figure.isStat
                    screen: figure.QsWindow.window?.screen ?? null
                    gap: 14 // clear of the island's rim
                }
                // The battery: its level, what it's doing, the power mode
                HoverTip {
                    target: figure
                    hovered: figureHover.hovered && root.shown && !figure.isStat
                    screen: figure.QsWindow.window?.screen ?? null
                    gap: 14
                    text: BatteryHealth.summary()
                }
                BatteryGlyph {
                    visible: figure.modelData === "battery"
                    anchors.verticalCenter: parent.verticalCenter
                    size: 10
                    level: Island.level
                    charging: Island.charging && Island.level < 1
                    plugged: Island.charging && Island.level >= 1
                    color: figure.tint
                }
                Icon {
                    visible: figure.modelData !== "battery"
                    anchors.verticalCenter: parent.verticalCenter
                    name: figure.modelData === "temperature" ? "thermometer-simple" : figure.modelData
                    size: 13
                    color: figure.tint
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: figure.modelData === "battery" ? Math.round(Island.level * 100) + "%" : figure.modelData === "temperature" ? Math.round(Resources.temperature) + "°" : Math.round((figure.modelData === "cpu" ? Resources.cpu : Resources.memory) * 100) + "%"
                    color: figure.tint
                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                }
            }
        }
    }
}
