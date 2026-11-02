import QtQuick
import "../../common/MaterialShapes.js" as Shapes

// common/MaterialShape for the login screen: the same shader and shapes, but
// nothing read from Theme (the greeter runs without the user's settings, so
// it passes its own colours). Changing `shape` morphs on Material's spring;
// `source` fills the shape with a picture; `spin` turns it, in degrees.
ShaderEffect {
    id: root

    property string shape: "circle"
    property color color: "transparent"
    property real borderWidth: 0
    property color borderColor: "transparent"
    property real spin: 0
    property Item source: null

    property string from: shape
    property string shownTo: shape
    property real progress: 1
    onShapeChanged: {
        from = progress < 0.5 ? from : shownTo;
        shownTo = shape;
        morphAnim.stop();
        progress = 0;
        morphAnim.start();
    }
    SpringAnimation {
        id: morphAnim
        target: root
        property: "progress"
        to: 1
        // (Material's expressive spring for shapes, ζ 0.6, k 200)
        spring: 3.2
        damping: 0.27
        epsilon: 0.002
    }

    readonly property var a: Shapes.get(from)
    readonly property var b: Shapes.get(shownTo)
    readonly property size size: Qt.size(width, height)
    readonly property vector4d shapeA: Qt.vector4d(a[0], a[1], a[2], a[3])
    readonly property vector4d shapeB: Qt.vector4d(b[0], b[1], b[2], b[3])
    readonly property point turnA: Qt.point(a[4] * Math.PI / 180, a[5])
    readonly property point turnB: Qt.point(b[4] * Math.PI / 180, b[5])
    readonly property real morph: Math.max(-0.2, Math.min(1.2, progress))
    readonly property real turn: spin * Math.PI / 180
    readonly property real picture: source ? 1 : 0
    readonly property color fillColor: color
    readonly property var pictureTex: pictureSource
    ShaderEffectSource {
        id: pictureSource
        sourceItem: root.source
        hideSource: true
        live: true
        visible: false
    }
    fragmentShader: Qt.resolvedUrl("../../common/materialshape.frag.qsb")
}
