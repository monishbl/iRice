import QtQuick
import Quickshell.Bluetooth
import qs.common
import qs.services

// Batteries: this computer and connected Bluetooth devices that report a
// level. Small: up to four rings. Medium: rings with names.
Item {
    id: root

    property string size: "s"
    readonly property var items: {
        const l = [];
        if (Island.hasBattery)
            l.push({ icon: Island.charging ? "lightning" : "laptop", value: Island.level, name: "This computer" });
        for (const d of Bluetooth.devices.values)
            if (d.connected && d.batteryAvailable)
                l.push({ icon: Devices.btIcon(d), value: d.battery, name: d.name || d.deviceName });
        return l.slice(0, 4);
    }
    function colorFor(it) {
        return it.icon === "lightning" ? Theme.accents.green : it.value <= 0.2 ? Theme.accents.red : Theme.accents.green;
    }

    Text {
        anchors.centerIn: parent
        visible: root.items.length === 0
        text: "No batteries"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    // Small, one battery: a big ring and the level
    Column {
        visible: root.size === "s" && root.items.length === 1
        anchors.centerIn: parent
        spacing: 8
        GaugeRing {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 76
            height: 76
            value: root.items[0]?.value ?? 0
            icon: root.items[0]?.icon ?? ""
            color: root.items[0] ? root.colorFor(root.items[0]) : Theme.accents.green
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Math.round((root.items[0]?.value ?? 0) * 100) + "%"
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold; features: { "tnum": 1 } }
        }
    }

    // Small: a 2×2 grid of rings
    Grid {
        visible: root.size === "s" && root.items.length > 1
        anchors.centerIn: parent
        columns: 2
        spacing: 12
        Repeater {
            model: root.size === "s" ? root.items : []
            GaugeRing {
                required property var modelData
                width: 52
                height: 52
                value: modelData.value
                icon: modelData.icon
                color: root.colorFor(modelData)
            }
        }
    }

    // Medium: a row of rings with level and name
    Row {
        visible: root.size !== "s"
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        Repeater {
            model: root.size !== "s" ? root.items : []
            Column {
                id: cell
                required property var modelData
                width: parent.width / Math.max(2, root.items.length)
                spacing: 6
                GaugeRing {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 56
                    height: 56
                    value: cell.modelData.value
                    icon: cell.modelData.icon
                    color: root.colorFor(cell.modelData)
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Math.round(cell.modelData.value * 100) + "%"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
                Text {
                    width: parent.width - 8
                    anchors.horizontalCenter: parent.horizontalCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: cell.modelData.name
                    elide: Text.ElideRight
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption }
                }
            }
        }
    }
}
