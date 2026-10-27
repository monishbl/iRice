import QtQuick

// A rounded rectangle with continuous (squircle) corners, like the glass:
// `color`, `radius`, and an optional `border` inside the edge.
//   Squircle { width: 200; height: 80; radius: 18; color: "white"; borderWidth: 1; borderColor: "#22000000" }
ShaderEffect {
    property real radius: 12
    property color color: "white"
    property real borderWidth: 0
    property color borderColor: "transparent"

    readonly property size size: Qt.size(width, height)
    readonly property color fillColor: color
    fragmentShader: Qt.resolvedUrl("squircle.frag.qsb")
}
