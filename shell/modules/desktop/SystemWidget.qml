import QtQuick
import qs.common
import qs.modules.resources
import qs.services

// The last minute of CPU and memory as graphs; medium adds temperature (or
// swap) and network beside them, large lists everything Resources samples
// (swap, upload, the GPU where the driver reports it). Sampled once a second
// while the widget is on the desktop.
Item {
    id: root

    property string size: "s"

    // Which stats, as a string so the Repeater only rebuilds when the set
    // changes (Resources.stats is rewritten every sample)
    readonly property string keys: {
        const have = Resources.stats.map(s => s.key);
        if (size === "s")
            return "cpu,memory";
        if (size === "m")
            return ["cpu", "memory", have.includes("temperature") ? "temperature" : "swap", "netDown"].join(",");
        return have.join(",");
    }
    readonly property var list: keys.split(",")
    readonly property int columns: size === "s" ? 1 : 2
    readonly property int rows: Math.ceil(list.length / columns)
    readonly property int spacing: 14

    Grid {
        anchors.fill: parent
        columns: root.columns
        columnSpacing: 20
        rowSpacing: root.spacing
        Repeater {
            model: root.list
            ResourceStat {
                required property string modelData
                key: modelData
                width: (root.width - (root.columns - 1) * 20) / root.columns
                height: (root.height - (root.rows - 1) * root.spacing) / root.rows
                graphHeight: height - 22
            }
        }
    }
}
