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
    property bool paused: false // (Material: the wave lies flat and still, as Android's does while media is paused)
    property color color: Theme.material ? (Theme.m3.primary ?? Theme.accent) : Theme.accent
    property color trackColor: Theme.material ? (Theme.m3.secondaryContainer ?? Theme.color.toggleOff) : Theme.color.toggleOff
    readonly property bool indeterminate: value < 0
    readonly property real shown: indeterminate ? 0.3 : Math.max(0, Math.min(1, value))

    implicitWidth: 200
    implicitHeight: Theme.material ? 10 : 4

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
            property real amplitude: root.paused || (!root.indeterminate && (root.shown <= 0.1 || root.shown >= 0.95)) ? 0 : 3
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
            // The wave: drawn once, a wavelength longer than the bar, and slid
            // sideways inside the visible stretch (rebuilding the path every
            // frame cost a re-triangulation 60 times a second wherever music
            // played); only a new width or height redraws it
            readonly property real wl: 40
            Item {
                id: strip
                x: m.start
                width: Math.max(0, m.end - m.start)
                height: m.height
                visible: width > 1
                clip: true
                Shape {
                    // (the same wave as before at every point on screen: y = sin(2π(x/wl − phase)))
                    x: -((((m.start - m.phase * m.wl) % m.wl) + m.wl) % m.wl)
                    width: strip.width + m.wl
                    height: m.height
                    preferredRendererType: Shape.CurveRenderer
                    ShapePath {
                        strokeColor: root.color
                        strokeWidth: m.stroke
                        fillColor: "transparent"
                        capStyle: ShapePath.FlatCap
                        PathPolyline {
                            path: {
                                const pts = [], h = m.height / 2, a = m.amplitude, w = strip.width + m.wl;
                                for (let x = 0; x <= w + 0.01; x += 2)
                                    pts.push(Qt.point(x, h + a * Math.sin(2 * Math.PI * x / m.wl)));
                                return pts;
                            }
                        }
                    }
                }
            }
            // (round ends, as the clip cuts the stroke's own caps)
            Repeater {
                model: 2
                Rectangle {
                    required property int index
                    visible: strip.visible
                    readonly property real at: index === 0 ? m.start + m.stroke / 2 : m.end - m.stroke / 2
                    width: m.stroke
                    height: m.stroke
                    radius: m.stroke / 2
                    color: root.color
                    x: at - m.stroke / 2
                    y: m.height / 2 + m.amplitude * Math.sin(2 * Math.PI * (at / m.wl - m.phase)) - m.stroke / 2
                }
            }
        }
    }
}
