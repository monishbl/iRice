import QtQuick
import qs.common

// The Material look's big clock, as Android 16's large lock screen clock: the
// hours over the minutes in Google Sans Flex, chunky and rounded through the
// font's variable axes (wght, wdth, ROND). Both lines are two tabular digits,
// so they're the same width and line up whatever the time. Nothing here reads
// Theme: the login screen uses it too, without the user's settings.
//   MaterialClock { time: clock.date; size: 280; color: Theme.m3.primaryFixed }
Item {
    id: root

    property date time: new Date()
    property int size: 200 // the digits' font size, px
    property string family: "Google Sans Flex"
    property real weight: 640 // wght 1..1000
    property real widthAxis: 112 // wdth 25..151
    property real round: 100 // ROND 0..100
    // Weight is rounded to steps, so animating it (the lock's entrance, the
    // move to StandBy) makes a handful of font instances, not one per frame
    property int step: 40
    property color color: "white" // the hours
    property color color2: color // the minutes
    property int align: Text.AlignHCenter // the lines against each other: AlignLeft | AlignHCenter | AlignRight
    // The minutes thin out and swell back as they change, as Pixel's clock
    // does (once a minute; off for StandBy, where nothing should move)
    property bool ticks: false

    readonly property string hours: TimeFormat.hour(time).padStart(2, "0")
    readonly property string minutes: String(time.getMinutes()).padStart(2, "0")

    // Google Sans Flex: ascent 0.966 em, digits 0.716 em tall. The lines are
    // placed by their digits, not their line boxes, so the stack is tight and
    // the item's bounds are the ink.
    readonly property real capTop: size * 0.25
    readonly property real digits: size * 0.716
    readonly property real gap: size * 0.085

    implicitWidth: Math.max(hoursText.implicitWidth, minutesText.implicitWidth)
    implicitHeight: digits * 2 + gap

    function axes(w) {
        const q = Math.max(1, Math.min(1000, Math.round(w / step) * step));
        return { "wght": q, "wdth": Math.round(widthAxis), "ROND": Math.round(round), "opsz": Math.min(144, size) };
    }

    property real dip: 0 // 0..1, the minute tick
    onMinutesChanged: if (ticks && visible) tick.restart()
    SequentialAnimation {
        id: tick
        NumberAnimation { target: root; property: "dip"; to: 1; duration: 180; easing.type: Easing.OutCubic }
        NumberAnimation { target: root; property: "dip"; to: 0; duration: 420; easing.type: Easing.OutBack }
    }

    Text {
        id: hoursText
        width: root.width
        y: -root.capTop
        horizontalAlignment: root.align
        text: root.hours
        color: root.color
        font.family: root.family
        font.pixelSize: root.size
        font.features: { "tnum": 1 }
        font.variableAxes: root.axes(root.weight)
    }
    Text {
        id: minutesText
        width: root.width
        y: root.digits + root.gap - root.capTop
        horizontalAlignment: root.align
        text: root.minutes
        color: root.color2
        font.family: root.family
        font.pixelSize: root.size
        font.features: { "tnum": 1 }
        font.variableAxes: root.axes(root.weight - root.dip * 240)
    }
}
