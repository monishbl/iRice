import QtQuick
import qs.common
import qs.glass

// A Control Center module background: a lighter rounded panel on the glass.
// As a chip (a panel without its own background, like Notification Center's
// option) it's a piece of liquid glass of its own instead.
Rectangle {
    id: root

    property bool chip: false
    radius: Theme.radius.module
    color: chip ? Qt.alpha("white", Theme.dark ? 0.07 : 0.28) : Theme.color.platter
    border.width: chip ? 1 : 0
    border.color: Qt.alpha("white", Theme.dark ? 0.06 : 0.3)

    Glass {
        visible: root.chip
        z: -1
        anchors.fill: parent
        anchors.margins: -shadowMargin
        shadowMargin: 12
        radius: root.radius
    }
}
