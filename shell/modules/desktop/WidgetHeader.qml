import QtQuick
import qs.common

// A widget's title row: a coloured disc with an icon, and the name.
Row {
    property string icon
    property string title
    property color tint: Theme.accent
    spacing: 8
    Rectangle {
        width: 26
        height: 26
        radius: 13
        color: parent.tint
        Icon { anchors.centerIn: parent; name: parent.parent.icon; fill: true; size: 15; color: "white" }
    }
    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: parent.title
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
    }
}
