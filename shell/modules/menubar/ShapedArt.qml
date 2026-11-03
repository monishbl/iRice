import QtQuick
import qs.common

// Material: a picture (album art, an avatar) cut into one of Material's
// shapes; the shape's own colour shows while there's none.
//   ShapedArt { width: 72; height: 72; url: Media.artUrl(player); sourceSize: Qt.size(144, 144); shape: "cookie9"; color: Theme.m3.surfaceContainerHighest }
Item {
    id: root

    property url url
    property size sourceSize
    property bool retainWhileLoading: true
    property alias shape: cut.shape
    property alias animated: cut.animated
    property alias color: cut.color

    Image {
        id: picture
        anchors.fill: parent
        source: root.url
        retainWhileLoading: root.retainWhileLoading
        fillMode: Image.PreserveAspectCrop
        sourceSize: root.sourceSize
        asynchronous: true
    }
    MaterialShape {
        id: cut
        anchors.fill: parent
        source: root.url.toString() !== "" ? picture : null
    }
}
