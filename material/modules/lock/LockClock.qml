import QtQuick
import qs.common
import qs.services

// The Material look's lock screen clock, in one of Android 17's styles
// (lock.clockFace), large or small: the dynamic clock (`small` runs 0 → 1
// as notifications or Now Playing arrive) shrinks the face into one line of
// the same digits, Android's small clock (analog: a small dial). All of it
// is Google Sans Flex on its axes: weight, width and roundness come from
// Settings → Lock Screen → Clock.
//   stacked  the hours over the minutes (MaterialClock: the StandBy morph's)
//   wide     one line, as wide as the font goes
//   overlap  heavy two-tone pairs, the minutes over the hours' corner
//   analog   hands on a 12-scallop dial
//   weather  the time over the weather's icon and temperature, as large
//   LockClock { face: "stacked"; time: clock.date; size: 240; small: 0 }
Item {
    id: root

    property string face: "stacked"
    property date time: new Date()
    property int size: 200 // the large face's digit size, px
    property int smallSize: 64 // the small clock's
    property real weight: 640 // wght 100 .. 1000
    property real widthAxis: 112 // wdth 25 .. 151
    property bool round: true // ROND 100, else 0
    property color color: "white" // the hours, the hands
    property color color2: color // the minutes
    property color color3: color2 // overlap's minutes, the dial
    property int align: Text.AlignHCenter // stacked's lines against each other
    property bool ticks: false // the minutes dip as they change (stacked)
    property real small: 0 // 0 large … 1 small

    readonly property string family: "Google Sans Flex"
    readonly property int h12: (time.getHours() + 11) % 12 + 1
    readonly property string mm: String(time.getMinutes()).padStart(2, "0")
    // (weight in steps: an animated weight makes a handful of font
    // instances, not one a frame)
    function axes(w, px, wd) {
        const q = Math.max(1, Math.min(1000, Math.round(w / 40) * 40));
        return { "wght": q, "wdth": Math.round(wd ?? widthAxis), "ROND": round ? 100 : 0, "opsz": Math.min(144, px) };
    }

    // The two forms cross over: the large one shrinks toward the small
    // one's size as it fades, the small one grows into place
    readonly property real k: Math.max(0, Math.min(1, small))
    readonly property real ratio: large.h > 0 ? Math.max(0.05, smallForm.h / large.h) : 1
    implicitWidth: large.w + (smallForm.w - large.w) * k
    implicitHeight: large.h + (smallForm.h - large.h) * k

    Loader {
        id: large
        active: root.k < 1
        visible: active && opacity > 0
        opacity: Math.max(0, 1 - root.k * 1.7)
        scale: 1 - (1 - root.ratio) * root.k
        transformOrigin: Item.TopLeft
        sourceComponent: root.face === "wide" ? wide : root.face === "overlap" ? overlap : root.face === "analog" ? analog : root.face === "weather" ? weatherFace : stacked
        readonly property real w: item?.implicitWidth ?? 0
        readonly property real h: item?.implicitHeight ?? 0
    }
    Loader {
        id: smallForm
        active: root.k > 0
        visible: active && opacity > 0
        opacity: Math.max(0, Math.min(1, (root.k - 0.35) / 0.65))
        scale: 1 / root.ratio + (1 - 1 / root.ratio) * root.k
        transformOrigin: Item.TopLeft
        sourceComponent: root.face === "analog" ? smallDial : smallLine
        // (its size is known while it's off, so the cross-over can aim at it)
        readonly property real w: root.face === "analog" ? Math.round(root.smallSize * 1.1) : probe.implicitWidth
        readonly property real h: root.face === "analog" ? Math.round(root.smallSize * 1.1) : Math.round(root.smallSize * 0.716)
    }
    Ink {
        id: probe
        visible: false
        px: root.smallSize
        text: root.h12 + ":" + root.mm
        wght: root.face === "overlap" ? Math.max(root.weight, 760) : root.weight
    }

    // Digits placed by their ink, not their line box: Google Sans Flex's
    // digits are 0.716 em tall, 0.25 em under the line's top
    component Ink: Item {
        id: ink
        property alias text: label.text
        property int px: root.size
        property real wght: root.weight
        property real wd: root.widthAxis
        property color tone: root.color
        implicitWidth: label.implicitWidth
        implicitHeight: Math.round(px * 0.716)
        Text {
            id: label
            y: -ink.px * 0.25
            color: ink.tone
            font.family: root.family
            font.pixelSize: ink.px
            font.features: { "tnum": 1 }
            font.variableAxes: root.axes(ink.wght, ink.px, ink.wd)
        }
    }

    Component {
        id: stacked
        MaterialClock {
            time: root.time
            size: root.size
            weight: root.weight
            widthAxis: root.widthAxis
            round: root.round ? 100 : 0
            color: root.color
            color2: root.color2
            align: root.align
            ticks: root.ticks
            width: implicitWidth
            height: implicitHeight
        }
    }

    Component {
        id: wide
        Ink {
            px: Math.round(root.size * 0.8)
            wd: Math.max(root.widthAxis, 140)
            text: root.h12 + ":" + root.mm
        }
    }

    Component {
        id: overlap
        Item {
            readonly property real w: Math.max(root.weight, 760)
            implicitWidth: Math.max(hours.implicitWidth, minutes.x + minutes.implicitWidth)
            implicitHeight: minutes.y + minutes.implicitHeight
            Ink {
                id: hours
                px: Math.round(root.size * 0.95)
                wght: parent.w
                wd: Math.min(root.widthAxis, 100)
                text: String(root.h12).padStart(2, "0")
            }
            Ink {
                id: minutes
                x: Math.round(hours.implicitWidth * 0.4)
                y: Math.round(hours.implicitHeight * 0.58)
                px: hours.px
                wght: parent.w
                wd: hours.wd
                tone: root.color3
                text: root.mm
            }
        }
    }

    Component {
        id: analog
        Dial {
            d: Math.round(root.size * 1.5)
        }
    }
    Component {
        id: smallDial
        Dial {
            d: Math.round(root.smallSize * 1.1)
        }
    }

    // A 12-scallop dial (Material's cookie), the hours' hand thick and the
    // minutes' long, both rounded
    component Dial: Item {
        id: dial
        property int d: 200
        implicitWidth: d
        implicitHeight: d
        readonly property real hourAngle: ((root.time.getHours() % 12) + root.time.getMinutes() / 60) * 30
        readonly property real minuteAngle: root.time.getMinutes() * 6
        MaterialShape {
            anchors.fill: parent
            shape: "cookie12"
            color: Qt.alpha(root.color3, 0.2)
        }
        Hand {
            length: dial.d * 0.27
            thick: dial.d * 0.085
            angle: dial.hourAngle
            tone: root.color
        }
        Hand {
            length: dial.d * 0.38
            thick: dial.d * 0.05
            angle: dial.minuteAngle
            tone: root.color2
        }
        Rectangle {
            anchors.centerIn: parent
            width: dial.d * 0.06
            height: width
            radius: width / 2
            color: Qt.darker(root.color, 1.6)
        }
    }
    component Hand: Rectangle {
        id: hand
        property real length
        property real thick
        property real angle
        property color tone
        width: thick
        height: length + thick / 2
        radius: thick / 2
        color: tone
        x: (parent?.d ?? 0) / 2 - width / 2
        y: (parent?.d ?? 0) / 2 - length
        transform: Rotation {
            origin.x: hand.width / 2
            origin.y: hand.length
            angle: hand.angle
            Behavior on angle {
                RotationAnimation { direction: RotationAnimation.Clockwise; duration: 450; easing.type: Easing.OutBack }
            }
        }
    }

    Component {
        id: weatherFace
        Item {
            implicitWidth: Math.max(clockLine.implicitWidth, sky.implicitWidth)
            implicitHeight: clockLine.implicitHeight + (sky.visible ? sky.implicitHeight + Math.round(root.size * 0.12) : 0)
            Ink {
                id: clockLine
                x: (parent.width - implicitWidth) / 2
                px: Math.round(root.size * 0.78)
                text: root.h12 + ":" + root.mm
            }
            Row {
                id: sky
                visible: Weather.ready
                x: (parent.width - implicitWidth) / 2
                y: clockLine.implicitHeight + Math.round(root.size * 0.12)
                spacing: Math.round(root.size * 0.06)
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: Weather.icon
                    fill: true
                    size: Math.round(root.size * 0.42)
                    color: root.color3
                }
                Ink {
                    anchors.verticalCenter: parent.verticalCenter
                    px: Math.round(root.size * 0.5)
                    tone: root.color2
                    text: Weather.temp + "°"
                }
            }
        }
    }

    Component {
        id: smallLine
        Ink {
            px: root.smallSize
            wght: root.face === "overlap" ? Math.max(root.weight, 760) : root.weight
            text: root.h12 + ":" + root.mm
        }
    }
}
