import QtQuick

// A rounded rectangle with continuous (squircle) corners, the same curve as
// the glass. A drop-in for `Rectangle { radius; color; border }`:
//   Squircle { width: 200; height: 80; radius: 14; color: "white"; border.width: 1; border.color: "#22000000" }
// (borderWidth / borderColor still work too.) The border is drawn inside the
// edge, like Rectangle's. One small shared shader, no layer and no per-frame
// work; nothing is drawn while it's fully transparent (a hover highlight at
// rest costs only an Item). At a full radius (a pill or a circle) the curve is
// the ordinary round one, so a pill can stay a Rectangle.
// No gradient: put a Rectangle with the gradient under a Squircle mask.
Item {
    id: root

    property real radius: 0
    property color color: "white"
    component Stroke: QtObject {
        property real width: 0
        property color color: "black"
    }
    readonly property Stroke border: Stroke { id: stroke }
    property alias borderWidth: stroke.width
    property alias borderColor: stroke.color

    ShaderEffect {
        anchors.fill: parent
        visible: width > 0 && height > 0 && (root.color.a > 0 || (stroke.width > 0 && stroke.color.a > 0))
        readonly property size size: Qt.size(width, height)
        readonly property real radius: root.radius
        readonly property real borderWidth: stroke.width
        readonly property color fillColor: root.color
        readonly property color borderColor: stroke.color
        fragmentShader: Qt.resolvedUrl("squircle.frag.qsb")
    }
}
