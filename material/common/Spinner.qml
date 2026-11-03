import QtQuick
import "MaterialShapes.js" as Shapes

// Something is loading. Glass: a turning ring. Material: Material 3
// Expressive's loading indicator, a shape morphing through softBurst,
// cookie9, pentagon, pill, sunny, cookie4 and oval, a new one every 650 ms,
// turning a quarter more with each, on top of a slow full turn.
//   Spinner { size: 16 }
Item {
    id: root

    property int size: 16
    property color color: Theme.material ? (Theme.m3.primary ?? Theme.accent) : Theme.color.secondary
    readonly property bool spinning: visible && opacity > 0

    implicitWidth: size
    implicitHeight: size


    Loader {
        anchors.centerIn: parent
        active: Theme.material
        sourceComponent: MaterialShape {
            id: shape
            // (Google's indicator: a 38 shape in a 48 box)
            width: Math.round(root.size * 0.9)
            height: width
            color: root.color
            property int step: 0
            shape: Shapes.loading[step % Shapes.loading.length]
            property real quarter: 0
            spin: quarter + rotationBase.angle
            Behavior on quarter { SpringAnimation { spring: 3.2; damping: 0.27; epsilon: 0.05 } }
            Timer {
                interval: 650
                repeat: true
                running: root.spinning
                onTriggered: {
                    shape.step++;
                    shape.quarter += 90;
                }
            }
            QtObject {
                id: rotationBase
                property real angle: 0
            }
            NumberAnimation {
                target: rotationBase
                property: "angle"
                from: 0
                to: 360
                duration: 4666
                loops: Animation.Infinite
                running: root.spinning
            }
        }
    }
}
