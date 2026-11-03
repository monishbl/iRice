import QtQuick
import qs.common
import qs.services

// Weather. Small: place, temperature, condition, high and low. Medium: adds
// the next hours.
Item {
    id: root

    property string size: "m"

    // Material, small: a Material You weather widget, a circle in the
    // tertiary container (no card behind it) with the temperature large
    readonly property bool materialSmall: Theme.material && size === "s"
    Loader {
        anchors.fill: parent
        active: root.materialSmall
        sourceComponent: Item {
            MaterialShape {
                anchors.centerIn: parent
                width: Math.min(parent.width, parent.height)
                height: width
                shape: "circle"
                color: Theme.m3.tertiaryContainer
            }
            Column {
                anchors.centerIn: parent
                width: Math.min(parent.width, parent.height) * 0.72
                spacing: 0
                visible: Weather.ready
                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: Weather.icon
                    fill: true
                    size: 22
                    color: Theme.m3.tertiary
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Weather.temp + "°"
                    color: Theme.m3.onTertiaryContainer
                    font { family: Theme.font.family; pixelSize: 46; weight: Font.Medium; features: { "tnum": 1 } }
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: Weather.place
                    color: Theme.m3.onTertiaryContainer
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "H:" + Weather.high + "°  L:" + Weather.low + "°"
                    color: Qt.alpha(Theme.m3.onTertiaryContainer, 0.75)
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                }
            }
            Text {
                anchors.centerIn: parent
                visible: !Weather.ready
                width: Math.min(parent.width, parent.height) * 0.7
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: Weather.error || "Loading weather…"
                color: Theme.m3.onTertiaryContainer
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
    }

    Text {
        anchors.centerIn: parent
        visible: !Weather.ready && !root.materialSmall
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: Weather.error || "Loading weather…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Item {
        anchors.fill: parent
        visible: Weather.ready && !root.materialSmall

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
                // (Material: emphasized, on its secondary container pill)
                color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.material ? 44 : 42; weight: Theme.material ? Font.Medium : Font.Normal; features: { "tnum": 1 } }
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
