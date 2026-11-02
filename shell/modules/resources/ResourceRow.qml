import QtQuick
import qs.common
import qs.services

// One group of Resources' stats on a row: an icon, the figures, and a small
// graph of the last minute on the right.
//   processor  CPU, with the temperature beside it
//   graphics   the GPU's load
//   memory     memory, with swap beside it
//   network    download and upload together (one graph, both lines)
//   ResourceRow { group: "network"; width: 280 }
// Showing one keeps sampling at once a second (Resources.graphUsers).
Item {
    id: root

    property string group: "processor"
    property bool highlighted: false // the one being pointed at (popover)

    readonly property var main: Resources.stat(({ processor: "cpu", graphics: "gpu", memory: "memory", network: "netDown" })[group] ?? "cpu")
    readonly property var second: group === "processor" ? Resources.stats.find(s => s.key === "temperature") : group === "memory" ? Resources.stat("swap") : group === "network" ? Resources.stat("netUp") : null
    readonly property color tone: Theme.accents[main.tone] ?? Theme.accent
    readonly property color tone2: second ? (Theme.accents[second.tone] ?? Theme.accent) : tone

    readonly property string title: ({ processor: "Processor", graphics: "Graphics", memory: "Memory", network: "Network" })[group] ?? ""
    readonly property string icon: ({ processor: "cpu", graphics: "graphics-card", memory: "memory", network: "arrows-down-up" })[group] ?? "cpu"
    // The figure on the right, and the line under the title
    readonly property string value: group === "network" ? "↓ " + main.text : main.text
    readonly property string valueSecond: group === "network" ? "↑ " + (second?.text ?? "") : ""
    readonly property string detail: group === "processor" ? (second ? second.text.replace("°", "°C") : "") : group === "memory" ? main.detail + (second && second.value > 0.005 ? " · swap " + second.text : "") : ""

    property bool counted: false
    function count() {
        const want = visible;
        if (want !== counted) {
            Resources.graphUsers += want ? 1 : -1;
            counted = want;
        }
    }
    onVisibleChanged: count()
    Component.onCompleted: count()
    Component.onDestruction: if (counted) Resources.graphUsers--

    implicitWidth: 280
    implicitHeight: 48

    Squircle {
        anchors.fill: parent
        radius: 10
        color: root.highlighted ? Qt.rgba(1, 1, 1, Theme.dark ? 0.08 : 0.35) : "transparent"
    }
    Rectangle {
        id: badge
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 28
        height: 28
        radius: 14
        color: Qt.alpha(root.tone, 0.18)
        Icon {
            anchors.centerIn: parent
            name: root.icon
            fill: true
            size: 15
            color: root.tone
        }
    }
    Column {
        anchors { left: badge.right; leftMargin: 10; right: figures.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
        spacing: 1
        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.title
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            visible: text !== ""
            elide: Text.ElideRight
            text: root.detail
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
        }
    }
    Column {
        id: figures
        anchors { right: graph.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
        spacing: 1
        Text {
            anchors.right: parent.right
            text: root.value
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: root.group === "network" ? Theme.font.caption : Theme.font.body; weight: Font.Medium; features: { "tnum": 1 } }
        }
        Text {
            anchors.right: parent.right
            visible: root.valueSecond !== ""
            text: root.valueSecond
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium; features: { "tnum": 1 } }
        }
    }
    // The last minute; network shares one scale between its two lines
    Item {
        id: graph
        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
        width: 76
        height: 26
        readonly property real netTop: root.group === "network" ? Math.max(65536, ...root.main.history, ...(root.second?.history ?? [])) * 1.15 : 0
        ResourceGraph {
            anchors.fill: parent
            values: root.main.history
            maximum: root.group === "network" ? graph.netTop : root.main.max
            color: root.tone
            lineWidth: 1.25
        }
        ResourceGraph {
            anchors.fill: parent
            visible: root.group === "network"
            values: root.second?.history ?? []
            maximum: graph.netTop
            color: root.tone2
            graphStyle: "line"
            lineWidth: 1.25
        }
    }
}
