import QtQuick
import Quickshell
import qs.common

// Clock. Small: an analog face. Medium: day, time and date.
Item {
    id: root

    property string size: "m"
    SystemClock { id: clock; precision: SystemClock.Minutes }

    AnalogClock {
        visible: root.size === "s"
        anchors.centerIn: parent
        width: Math.min(parent.width, parent.height)
        height: width
        hours: clock.date.getHours()
        minutes: clock.date.getMinutes()
    }

    Column {
        visible: root.size !== "s"
        anchors.verticalCenter: parent.verticalCenter
        spacing: 0
        Text {
            text: Qt.formatDate(clock.date, "dddd").toUpperCase()
            color: Theme.accents.orange
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Bold; letterSpacing: 1 }
        }
        Text {
            text: Qt.formatTime(clock.date, "h:mm")
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: 64; weight: Font.DemiBold; letterSpacing: -2; features: { "tnum": 1 } }
        }
        Text {
            text: Qt.formatDate(clock.date, "d MMMM yyyy")
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
        }
    }
}
