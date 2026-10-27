import QtQuick
import qs.common
import qs.glass

// A Control Center module background: a lighter rounded panel on the glass,
// with iOS corners (Theme.radius.tile, continuous; never more than half its
// height). As a chip (a panel without its own background, like Notification
// Center's option) it's a piece of liquid glass of its own instead.
Item {
    id: root

    property bool chip: false
    property real radius: Theme.radius.tile
    property color color: chip ? Qt.alpha("white", Theme.dark ? 0.07 : 0.28) : Theme.color.platter
    readonly property real shapeRadius: Math.min(radius, Math.min(width, height) / 2)

    Glass {
        visible: root.chip
        z: -1
        anchors.fill: parent
        anchors.margins: -shadowMargin
        shadowMargin: 12
        radius: root.shapeRadius
    }
    Squircle {
        z: -1
        anchors.fill: parent
        radius: root.shapeRadius
        color: root.color
        borderWidth: root.chip ? 1 : 0
        borderColor: Qt.alpha("white", Theme.dark ? 0.06 : 0.3)
    }
}
