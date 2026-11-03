import QtQuick
import Quickshell.Widgets
import qs.common

// A result's own picture in place of its icon (a screenshot found by the
// words in it): cropped to a rounded square, read small
ClippingRectangle {
    id: root

    property string path

    radius: Theme.material ? width / 2 : 6
    color: Theme.color.fill
    Image {
        anchors.fill: parent
        source: Wallpaper.fileUrl(root.path)
        sourceSize: Qt.size(96, 96)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        smooth: true
    }
}
