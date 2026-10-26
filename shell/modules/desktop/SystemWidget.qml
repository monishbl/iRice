import QtQuick
import qs.common
import qs.services

// CPU and memory (plus temperature and swap when medium), sampled slowly
// while the widget is on the desktop.
Item {
    id: root

    property string size: "s"
    Component.onCompleted: Resources.desktopUsers++
    Component.onDestruction: Resources.desktopUsers--

    readonly property var gauges: [
        { label: "CPU", value: Resources.cpu, text: Math.round(Resources.cpu * 100) + "%", color: Theme.accents.blue },
        { label: "Memory", value: Resources.memory, text: Math.round(Resources.memory * 100) + "%", color: Theme.accents.green },
        ...(size !== "s" ? [
            { label: "Temp", value: Resources.temperature / 100, text: Resources.temperature > 0 ? Math.round(Resources.temperature) + "°" : "—", color: Theme.accents.orange },
            { label: "Swap", value: Resources.swap, text: Math.round(Resources.swap * 100) + "%", color: Theme.accents.purple }
        ] : [])
    ]
    function warn(g) {
        return g.value > 0.9 ? Theme.accents.red : g.color;
    }

    Row {
        anchors.centerIn: parent
        spacing: root.size === "s" ? 10 : 18
        Repeater {
            model: root.gauges
            Column {
                id: g
                required property var modelData
                spacing: 6
                GaugeRing {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: root.size === "s" ? 58 : 60
                    height: width
                    value: g.modelData.value
                    text: g.modelData.text
                    color: root.warn(g.modelData)
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: g.modelData.label
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                }
            }
        }
    }
}
