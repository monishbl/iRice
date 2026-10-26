import QtQuick
import QtQuick.Shapes
import qs.common

// A circular gauge: a faint track and an arc for `value` (0..1), with an icon
// or text in the middle. Used by the Batteries and System widgets.
Item {
    id: root

    property real value: 0
    property color color: Theme.accent
    property string icon: ""
    property string text: ""
    property real thickness: Math.max(4, width * 0.09)
    implicitWidth: 56
    implicitHeight: 56

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: Theme.color.fill
            strokeWidth: root.thickness
            fillColor: "transparent"
            PathAngleArc { centerX: root.width / 2; centerY: root.height / 2; radiusX: (root.width - root.thickness) / 2; radiusY: radiusX; startAngle: -90; sweepAngle: 360 }
        }
        ShapePath {
            strokeColor: root.color
            strokeWidth: root.thickness
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            PathAngleArc { centerX: root.width / 2; centerY: root.height / 2; radiusX: (root.width - root.thickness) / 2; radiusY: radiusX; startAngle: -90; sweepAngle: 360 * Math.max(0, Math.min(1, root.value)) }
        }
    }
    Icon {
        anchors.centerIn: parent
        visible: root.icon !== ""
        name: root.icon
        fill: true
        size: Math.round(root.width * 0.36)
        color: Theme.color.label
    }
    Text {
        anchors.centerIn: parent
        visible: root.icon === "" && root.text !== ""
        text: root.text
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Math.round(root.width * 0.24); weight: Font.DemiBold; features: { "tnum": 1 } }
    }
}
