import QtQuick

// macOS-style toggle switch.
Item {
    id: root

    property bool checked: false
    signal toggled

    implicitWidth: 38
    implicitHeight: 22
    activeFocusOnTab: true
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Space && !event.isAutoRepeat) {
            root.toggled();
            event.accepted = true;
        }
    }
    // Keyboard focus (Tab)
    Rectangle {
        anchors { fill: parent; margins: -3 }
        visible: root.activeFocus
        radius: height / 2
        color: "transparent"
        border.width: 2
        border.color: Theme.accent
    }

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.checked ? Theme.accent : Theme.color.toggleOff
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

        Rectangle {
            width: parent.height - 4
            height: width
            radius: width / 2
            x: root.checked ? parent.width - width - 2 : 2
            y: 2
            color: "white"
            Behavior on x { XAnimator { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
        }
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
