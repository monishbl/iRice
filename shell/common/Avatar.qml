import QtQuick
import Quickshell.Widgets
import qs.services

// Your picture in a circle (Glass's; Material draws it in a cookie), or the
// person glyph on `color` while there's none. Named for screen readers by
// your name.
ClippingRectangle {
    id: root

    property color iconColor: Theme.color.label
    readonly property bool hasPicture: image.status === Image.Ready && image.source != ""

    implicitWidth: 40
    implicitHeight: 40
    radius: width / 2
    color: Theme.color.fill
    Accessible.role: Accessible.Graphic
    Accessible.name: Account.name

    Icon {
        anchors.centerIn: parent
        visible: !root.hasPicture
        name: "user"
        fill: true
        size: Math.round(root.width / 2)
        color: root.iconColor
    }
    Image {
        id: image
        anchors.fill: parent
        visible: root.hasPicture
        source: Account.pictureUrl
        sourceSize: Qt.size(root.width * 2, root.height * 2)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
    }
}
