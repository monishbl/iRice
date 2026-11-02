import QtQuick
import qs.common
import qs.services

// Weather. Small: place, temperature, condition, high and low. Medium: adds
// the next hours.
Item {
    id: root

    property string size: "m"

    Text {
        anchors.centerIn: parent
        visible: !Weather.ready
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: Weather.error || "Loading weather…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Item {
        anchors.fill: parent
        visible: Weather.ready

        Column {
            spacing: 0
            width: root.size === "s" ? parent.width : parent.width / 2
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: Weather.place
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
            Text {
                text: Weather.temp + Weather.unit
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: 42; weight: Font.Normal; features: { "tnum": 1 } }
            }
        }

        // Small: condition at the bottom
        Column {
            visible: root.size === "s"
            anchors.bottom: parent.bottom
            spacing: 2
            Icon { name: Weather.icon; fill: true; size: 18 }
            Text {
                text: Weather.condition
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
            Text {
                text: "H:" + Weather.high + "°  L:" + Weather.low + "°"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
            }
        }

        // Medium: condition on the right, hours along the bottom
        Column {
            visible: root.size !== "s"
            anchors.right: parent.right
            spacing: 2
            Icon { anchors.right: parent.right; name: Weather.icon; fill: true; size: 26 }
            Text {
                anchors.right: parent.right
                text: Weather.condition
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
            Text {
                anchors.right: parent.right
                text: "H:" + Weather.high + "°  L:" + Weather.low + "°"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
            }
        }
        Row {
            visible: root.size !== "s"
            anchors.bottom: parent.bottom
            width: parent.width
            Repeater {
                model: Weather.hourly
                Column {
                    id: hour
                    required property var modelData
                    width: parent.width / Math.max(1, Weather.hourly.length)
                    spacing: 2
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: hour.modelData.time
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                    }
                    Icon { anchors.horizontalCenter: parent.horizontalCenter; name: hour.modelData.icon; fill: true; size: 14 }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: hour.modelData.temp + "°"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                }
            }
        }
    }
}
