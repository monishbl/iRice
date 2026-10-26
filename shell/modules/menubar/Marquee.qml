import QtQuick
import qs.common

// A one-line label that, when it doesn't fit and `scroll` is on, slides
// slowly to show the rest, pauses, and comes back. Otherwise it elides.
Item {
    id: root

    property string text
    property color color: "white"
    property font font
    property bool scroll: true
    property real maxWidth: 140

    readonly property bool overflow: probe.implicitWidth > maxWidth
    implicitWidth: Math.min(probe.implicitWidth, maxWidth)
    implicitHeight: probe.implicitHeight
    clip: overflow && scroll

    Text {
        id: probe
        visible: false
        text: root.text
        font: root.font
    }
    Text {
        id: label
        width: root.overflow && root.scroll ? implicitWidth : root.width
        text: root.text
        color: root.color
        font: root.font
        elide: root.overflow && root.scroll ? Text.ElideNone : Text.ElideRight
        x: 0
    }
    // Slide to the end at ~30 px/s, wait, slide back
    SequentialAnimation {
        running: root.overflow && root.scroll && root.visible
        loops: Animation.Infinite
        onRunningChanged: if (!running) label.x = 0
        PauseAnimation { duration: 1800 }
        NumberAnimation { target: label; property: "x"; to: root.width - probe.implicitWidth; duration: Math.max(600, (probe.implicitWidth - root.width) * 33); easing.type: Easing.InOutSine }
        PauseAnimation { duration: 1400 }
        NumberAnimation { target: label; property: "x"; to: 0; duration: Math.max(400, (probe.implicitWidth - root.width) * 12); easing.type: Easing.InOutSine }
    }
    onTextChanged: label.x = 0
}
