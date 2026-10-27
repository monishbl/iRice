import QtQuick

// A thin overlay scrollbar for a Flickable, like macOS: shown while the
// content scrolls or the pointer is near, hidden when everything fits.
// Drag the thumb or click the track to move.
//   ScrollIndicator { target: flick; anchors { right: flick.right; top: flick.top; bottom: flick.bottom } }
Item {
    id: root

    required property Flickable target
    readonly property bool needed: target.contentHeight > target.height + 1
    readonly property real span: Math.max(1, target.contentHeight - target.height)
    readonly property bool active: hover.containsMouse || drag.pressed || idle.running

    width: 12
    visible: needed
    opacity: active ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: root.active ? 150 : 400; easing.type: Easing.OutCubic } }

    Timer { id: idle; interval: 900 }
    Connections {
        target: root.target
        function onContentYChanged() { idle.restart(); }
    }

    Rectangle {
        id: thumb
        x: (root.width - width) / 2
        width: hover.containsMouse || drag.pressed ? 8 : 6
        height: Math.max(28, root.height * root.target.height / Math.max(1, root.target.contentHeight))
        y: (root.height - height) * Math.max(0, Math.min(1, root.target.contentY / root.span))
        radius: width / 2
        color: Theme.dark ? Qt.rgba(1, 1, 1, drag.pressed ? 0.5 : 0.32) : Qt.rgba(0, 0, 0, drag.pressed ? 0.5 : 0.32)
        Behavior on width { NumberAnimation { duration: 150 } }
    }

    MouseArea {
        id: hover
        anchors.fill: parent
        hoverEnabled: true
        onPressed: m => {
            if (m.y < thumb.y || m.y > thumb.y + thumb.height)
                root.target.contentY = Math.max(0, Math.min(root.span, (m.y - thumb.height / 2) / Math.max(1, root.height - thumb.height) * root.span));
        }
    }
    MouseArea {
        id: drag
        x: 0
        y: thumb.y
        width: root.width
        height: thumb.height
        property real grab: 0 // where in the thumb it was taken
        onPressed: m => grab = m.y
        onPositionChanged: m => {
            if (!pressed)
                return;
            const top = mapToItem(root, 0, m.y).y - grab; // (this area moves with the thumb)
            root.target.contentY = Math.max(0, Math.min(root.span, top / Math.max(1, root.height - thumb.height) * root.span));
        }
    }
}
