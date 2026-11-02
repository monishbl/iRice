import QtQuick
import QtQuick.Shapes

// A progress bar. Glass: a thin rounded bar. Material: Material 3
// Expressive's wavy bar: the done part a moving wave (amplitude 3,
// wavelength 40, one wavelength a second), flat near the ends, a gap, then
// the track and a stop dot. `value` 0..1, or < 0 for "don't know yet".
//   WavyProgress { width: 200; value: 0.4 }
Item {
    id: root

    property real value: 0
    property color color: Theme.material ? (Theme.m3.primary ?? Theme.accent) : Theme.accent
    property color trackColor: Theme.material ? (Theme.m3.secondaryContainer ?? Theme.color.toggleOff) : Theme.color.toggleOff
    readonly property bool indeterminate: value < 0
    readonly property real shown: indeterminate ? 0.3 : Math.max(0, Math.min(1, value))

    implicitWidth: 200
    implicitHeight: Theme.material ? 10 : 4

    // ── Glass ──
    Rectangle {
        visible: !Theme.material
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        height: 4
        radius: 2
        color: root.trackColor
        Rectangle {
            x: root.indeterminate ? slide.pos * (parent.width - width) : 0
            width: parent.width * root.shown
            height: parent.height
            radius: 2
            color: root.color
            Behavior on width { enabled: !root.indeterminate; NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }
        }
    }
    QtObject {
        id: slide
        property real pos: 0
    }
    SequentialAnimation {
        running: root.indeterminate && root.visible
        loops: Animation.Infinite
        NumberAnimation { target: slide; property: "pos"; from: 0; to: 1; duration: 900; easing.type: Easing.InOutCubic }
        NumberAnimation { target: slide; property: "pos"; from: 1; to: 0; duration: 900; easing.type: Easing.InOutCubic }
    }

    // ── Material ──
    Loader {
        anchors.fill: parent
        active: Theme.material
        sourceComponent: Item {
            id: m
            readonly property real stroke: 4
            readonly property real gap: 4
            readonly property real start: root.indeterminate ? slide.pos * (width - width * 0.3) : 0
            readonly property real end: start + width * root.shown
            // (flat near 0 and 100%, as Google's)
            property real amplitude: !root.indeterminate && (root.shown <= 0.1 || root.shown >= 0.95) ? 0 : 3
            Behavior on amplitude { NumberAnimation { duration: 300 } }
            property real phase: 0
            NumberAnimation on phase {
                from: 0
                to: 1
                duration: 1000
                loops: Animation.Infinite
                running: root.visible && m.amplitude > 0
            }
            // the track, after a gap, with its stop dot
            Rectangle {
                x: Math.min(m.width, m.end + m.gap + (m.end > 0 ? m.stroke / 2 : 0))
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(0, m.width - x)
                height: m.stroke
                radius: 2
                color: root.trackColor
                Rectangle {
                    visible: !root.indeterminate
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: 4
                    height: 4
                    radius: 2
                    color: root.color
                }
            }
            Shape {
                anchors.fill: parent
                preferredRendererType: Shape.CurveRenderer
                visible: m.end - m.start > 1
                ShapePath {
                    strokeColor: root.color
                    strokeWidth: m.stroke
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    PathPolyline {
                        path: {
                            const pts = [], h = m.height / 2, wl = 40, a = m.amplitude;
                            const x0 = m.start + m.stroke / 2, x1 = Math.max(x0 + 0.5, m.end - m.stroke / 2);
                            for (let x = x0; x <= x1 + 0.01; x += 2)
                                pts.push(Qt.point(x, h + a * Math.sin(2 * Math.PI * (x / wl - m.phase))));
                            return pts;
                        }
                    }
                }
            }
        }
    }
}
