import QtQuick
import qs.common

// Text as a terminal shows it, in a dark scrolling box: a bug report's
// attachments, an update's output. `follow` keeps the end in view as text
// comes, unless you scrolled up to read.
Squircle {
    id: root

    property string text
    property bool follow: false

    radius: Theme.material ? 16 : 8
    color: Theme.material ? Theme.m3.inverseSurface : Theme.dark ? "#0E0E10" : "#18181B"
    Flickable {
        id: flick
        Momentum {}
        anchors { fill: parent; margins: 10 }
        clip: true
        contentHeight: body.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        property bool atEnd: true
        onMovementEnded: atEnd = contentY >= contentHeight - height - 4
        onContentHeightChanged: if (root.follow && atEnd) contentY = Math.max(0, contentHeight - height)
        Text {
            id: body
            width: flick.width
            text: root.text
            wrapMode: Text.WrapAnywhere
            textFormat: Text.PlainText
            color: Theme.material ? Theme.m3.inverseOnSurface : "#D4D4D8"
            font { family: Theme.font.mono; pixelSize: 11 }
        }
    }
    ScrollIndicator {
        target: flick
        anchors { right: parent.right; top: parent.top; bottom: parent.bottom; rightMargin: 2 }
    }
}
