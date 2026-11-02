import QtQuick
import "MaterialShapes.js" as Shapes

// A Material 3 Expressive shape (MaterialShapes.js: "cookie9", "sunny",
// "clover4", "pill", …), filled with a colour, or with a picture through
// `source` (an Image or any item; it's drawn into the shape), with an
// optional border. Changing `shape` morphs to the new one on Material's
// spring, the way Android morphs selected and pressed shapes.
//   MaterialShape { width: 48; height: 48; shape: selected ? "cookie9" : "circle"; color: Theme.m3.primaryContainer }
ShaderEffect {
    id: root

    property string shape: "circle"
    property color color: Theme.m3.primary ?? Theme.accent
    property real borderWidth: 0
    property color borderColor: "transparent"
    property real spin: 0 // degrees, the whole shape
    property Item source: null // a picture to fill the shape with (hidden itself)
    property bool animated: true

    // The morph: from the last shape toward the current one
    property string from: shape
    property real progress: 1
    onShapeChanged: {
        if (!animated) {
            from = shape;
            progress = 1;
            return;
        }
        // Mid-morph: start from the shape it's mostly showing
        from = progress < 0.5 ? from : shownTo;
        shownTo = shape;
        morphAnim.stop();
        progress = 0;
        morphAnim.start();
    }
    property string shownTo: shape
    SpringAnimation {
        id: morphAnim
        target: root
        property: "progress"
        to: 1
        // Material's expressive spring for shapes: damping ratio 0.6,
        // stiffness 200 (Qt's spring steps 16 ms: k / 62.5, 2ζ√k / 62.5)
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
    readonly property real morph: Math.max(-0.2, Math.min(1.2, progress)) // (a little overshoot, bounded)
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
    fragmentShader: Qt.resolvedUrl("materialshape.frag.qsb")
}
