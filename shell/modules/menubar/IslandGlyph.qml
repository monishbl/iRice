import QtQuick
import QtQuick.Shapes
import qs.common

// The animated leading icon of an island alert. Every alert springs in; then
// each kind plays its own short motion:
//   device, usb, display, custom   ripples spread out from the icon
//   charging                       a ring fills to the battery level, the bolt pulses
//   battery (low)                  the icon shakes, the ring glows red
//   audio                          sound waves pulse beside the icon
//   music                          the album art flips in
//   timer, focus                   the icon turns in, a ring sweeps
//   temperature                    a ring fills and warms from orange to red,
//                                  then the thermometer pulses with a red glow
// Everything stops a moment after the alert opens (nothing loops forever).
Item {
    id: root

    property var event: null
    readonly property string kind: event?.kind ?? ""
    readonly property color tint: event?.color ?? Theme.accent
    readonly property bool hasArt: !!event?.art
    readonly property bool ringed: (kind === "charging" || kind === "battery" || kind === "timer" || kind === "temperature") && event?.value !== undefined
    property int size: 44

    implicitWidth: size
    implicitHeight: size

    // Restart the motion whenever a new alert arrives
    // (deferred: bindings that depend on `event`, like kind, update after this handler)
    // The owner calls play() when it appears (stacked alerts start later)
    readonly property bool rippled: ["device", "usb", "display", "custom", "notification"].includes(kind)
    signal played
    function play() {
        enter.restart();
        ringT = 0;
        if (ringed)
            ringAnim.restart();
        if (kind === "battery")
            shake.restart();
        if (kind === "charging")
            bolt.restart();
        if (kind === "temperature")
            heat.restart();
        played();
    }

    // ── Disc and icon ───────────────────────────────────────────────────
    Item {
        id: body
        anchors.centerIn: parent
        width: root.size
        height: root.size
        property real pop: 1
        property real turn: 0
        scale: pop

        // Ripples behind the disc
        Repeater {
            model: 2
            Rectangle {
                id: rip
                required property int index
                anchors.centerIn: parent
                width: body.width
                height: width
                radius: width / 2
                color: "transparent"
                border.width: 1.5
                border.color: root.tint
                property real p: 1
                scale: 1 + p * 0.9
                opacity: (1 - p) * 0.7
                Connections {
                    target: root
                    function onPlayed() { if (root.rippled) ripAnim.restart(); }
                }
                SequentialAnimation {
                    id: ripAnim
                    PropertyAction { target: rip; property: "p"; value: 1 }
                    PauseAnimation { duration: 140 + rip.index * 260 }
                    NumberAnimation { target: rip; property: "p"; from: 0; to: 1; duration: 900; easing.type: Easing.OutCubic }
                }
            }
        }

        Rectangle {
            id: disc
            anchors.fill: parent
            radius: root.hasArt ? 12 : width / 2
            color: root.hasArt ? Qt.rgba(1, 1, 1, 0.1) : Qt.alpha(root.tint, 0.24)
            clip: true
            Image {
                anchors.fill: parent
                visible: root.hasArt
                source: root.event?.art ?? ""
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(root.size * 2, root.size * 2)
                asynchronous: true
            }
            // Low battery and heat: the disc glows
            Rectangle {
                id: glow
                anchors.fill: parent
                radius: parent.radius
                color: root.tint
                opacity: 0
                SequentialAnimation on opacity {
                    running: shake.running
                    loops: 2
                    NumberAnimation { to: 0.35; duration: 260 }
                    NumberAnimation { to: 0; duration: 380 }
                }
            }
        }
        Icon {
            id: glyph
            anchors.centerIn: parent
            anchors.verticalCenterOffset: glyphDrop.y
            visible: !root.hasArt
            name: root.event?.icon ?? ""
            fill: true
            size: Math.round(root.size * (root.ringed ? 0.42 : 0.48))
            color: root.tint
            rotation: body.turn
        }
        transform: [
            Translate { id: shakeX },
            Rotation { id: flip; origin.x: body.width / 2; origin.y: body.height / 2; axis { x: 0; y: 1; z: 0 } angle: 0 }
        ]
    }

    // Battery / timer ring
    Shape {
        id: ring
        anchors.fill: parent
        visible: root.ringed
        readonly property real target: Math.max(0, Math.min(1, root.event?.value ?? 0))
        // A timer ring runs down from full; a battery ring fills up
        readonly property real sweep: root.kind === "timer" ? 1 - (1 - target) * root.ringT : target * root.ringT
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: Qt.rgba(1, 1, 1, 0.14)
            strokeWidth: 3
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            PathAngleArc { centerX: root.size / 2; centerY: root.size / 2; radiusX: root.size / 2 - 2; radiusY: radiusX; startAngle: -90; sweepAngle: 360 }
        }
        ShapePath {
            // Temperature warms from orange to red as it fills
            strokeColor: root.kind === "temperature" ? Qt.rgba(1, 0.62 - 0.35 * root.ringT, 0.1, 1) : root.tint
            strokeWidth: 3
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            PathAngleArc { centerX: root.size / 2; centerY: root.size / 2; radiusX: root.size / 2 - 2; radiusY: radiusX; startAngle: -90; sweepAngle: 360 * ring.sweep }
        }
    }

    // Sound waves to the right of a speaker or headphones
    Row {
        id: waveRow
        visible: root.kind === "audio"
        anchors.left: parent.right
        anchors.leftMargin: -6
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1
        Repeater {
            model: 3
            Shape {
                id: wave
                required property int index
                width: 6
                height: 10 + index * 6
                anchors.verticalCenter: parent.verticalCenter
                opacity: 0
                ShapePath {
                    strokeColor: root.tint
                    strokeWidth: 2
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    PathAngleArc { centerX: -wave.height / 2 + 4; centerY: wave.height / 2; radiusX: wave.height / 2; radiusY: wave.height / 2; startAngle: -40; sweepAngle: 80 }
                }
                Connections {
                    target: root
                    function onPlayed() { if (root.kind === "audio") waveAnim.restart(); }
                }
                SequentialAnimation {
                    id: waveAnim
                    PauseAnimation { duration: 200 + wave.index * 120 }
                    SequentialAnimation {
                        loops: 3
                        NumberAnimation { target: wave; property: "opacity"; to: 1; duration: 180 }
                        NumberAnimation { target: wave; property: "opacity"; to: 0.25; duration: 360 }
                    }
                    NumberAnimation { target: wave; property: "opacity"; to: 1; duration: 200 }
                }
            }
        }
    }

    // ── Motions ─────────────────────────────────────────────────────────
    // Spring in: the disc pops from small, the icon turns or flips in
    SequentialAnimation {
        id: enter
        PropertyAction { target: body; property: "pop"; value: 0.35 }
        PropertyAction { target: body; property: "turn"; value: root.kind === "timer" || root.kind === "focus" ? -90 : root.kind === "usb" ? 0 : -18 }
        PropertyAction { target: flip; property: "angle"; value: root.hasArt ? 90 : 0 }
        PropertyAction { target: glyphDrop; property: "y"; value: root.kind === "usb" ? -14 : 0 }
        ParallelAnimation {
            SpringAnimation { target: body; property: "pop"; to: 1; spring: 4; damping: 0.22; epsilon: 0.002 }
            NumberAnimation { target: body; property: "turn"; to: 0; duration: 520; easing.type: Easing.OutBack; easing.overshoot: 2 }
            NumberAnimation { target: flip; property: "angle"; to: 0; duration: 520; easing.type: Easing.OutBack }
            NumberAnimation { target: glyphDrop; property: "y"; to: 0; duration: 620; easing.type: Easing.OutBounce }
        }
    }
    // USB: the icon drops in and bounces
    QtObject {
        id: glyphDrop
        property real y: 0
    }
    property real ringT: 0
    SequentialAnimation {
        id: ringAnim
        PauseAnimation { duration: 180 }
        NumberAnimation { target: root; property: "ringT"; from: 0; to: 1; duration: 900; easing.type: Easing.OutCubic }
    }
    SequentialAnimation {
        id: shake
        PauseAnimation { duration: 250 }
        SequentialAnimation {
            loops: 3
            NumberAnimation { target: shakeX; property: "x"; to: -3; duration: 50 }
            NumberAnimation { target: shakeX; property: "x"; to: 3; duration: 90 }
            NumberAnimation { target: shakeX; property: "x"; to: 0; duration: 50 }
        }
    }
    // Heat: after the ring fills, the thermometer swells with a red glow, twice
    SequentialAnimation {
        id: heat
        PauseAnimation { duration: 900 }
        SequentialAnimation {
            loops: 2
            ParallelAnimation {
                NumberAnimation { target: glow; property: "opacity"; to: 0.4; duration: 380; easing.type: Easing.OutSine }
                NumberAnimation { target: glyph; property: "scale"; to: 1.18; duration: 380; easing.type: Easing.OutSine }
            }
            ParallelAnimation {
                NumberAnimation { target: glow; property: "opacity"; to: 0; duration: 520; easing.type: Easing.InOutSine }
                NumberAnimation { target: glyph; property: "scale"; to: 1; duration: 520; easing.type: Easing.InOutSine }
            }
        }
    }

    // Charging: the bolt pulses twice
    SequentialAnimation {
        id: bolt
        PauseAnimation { duration: 400 }
        SequentialAnimation {
            loops: 2
            NumberAnimation { target: glyph; property: "scale"; to: 1.25; duration: 160; easing.type: Easing.OutCubic }
            NumberAnimation { target: glyph; property: "scale"; to: 1; duration: 260; easing.type: Easing.InOutCubic }
        }
    }
}
