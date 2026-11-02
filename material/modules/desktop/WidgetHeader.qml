import QtQuick
import qs.common

// A widget's title row: a coloured disc with an icon, and the name.
Row {
    property string icon
    property string title
    property color tint: Theme.accent
    // Something's wrong (a low battery): Material draws the disc in error
    // colours; Glass reads the tint
    property bool alert: false
    spacing: 8
    Rectangle {
        width: 26
        height: 26
        radius: 13
        // (Material: the scheme's primary, every widget alike)
        color: Theme.material ? (parent.alert ? Theme.m3.error : Theme.m3.primary) : parent.tint
        Icon { anchors.centerIn: parent; name: parent.parent.icon; fill: true; size: 15; color: Theme.material ? (parent.parent.alert ? Theme.m3.onError : Theme.m3.onPrimary) : (0.2126 * parent.color.r + 0.7152 * parent.color.g + 0.0722 * parent.color.b) > 0.7 ? "#1C1C1E" : "white" } // dark on light tints (yellow)
    }
    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: parent.title
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
    }
}
