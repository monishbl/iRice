import QtQuick
import qs.common
import qs.modules.resources
import qs.services

// CPU & Memory: the last minute of everything Resources samples (CPU,
// memory, swap, temperature, network, the GPU where the driver reports it).
// Graphs sample once a second while this page shows.
DetailPage {
    id: root

    title: "CPU & Memory"

    // A speed test card (SpeedTestCard.qml) goes here, under the network
    // graphs: networkExtra: Component { SpeedTestCard {} }
    property Component networkExtra: null

    Column {
        width: parent.width
        spacing: 2
        Repeater {
            model: Resources.groups
            Column {
                id: entry
                required property string modelData
                width: parent.width
                spacing: 6
                ResourceRow {
                    width: parent.width
                    group: entry.modelData
                }
                Loader {
                    width: parent.width
                    active: entry.modelData === "network" && root.networkExtra !== null
                    visible: active
                    sourceComponent: root.networkExtra
                }
            }
        }
    }

    Item {
        width: parent.width
        height: 2
    }
    Text {
        width: parent.width
        leftPadding: 6
        rightPadding: 6
        wrapMode: Text.Wrap
        text: "The last minute, once a second. Network counts every interface but loopback, containers and VPN tunnels." + (Resources.gpuAvailable ? "" : " Your GPU driver doesn't report its load.")
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }
}
