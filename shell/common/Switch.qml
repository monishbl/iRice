import QtQuick

// macOS-style toggle switch.
Item {
    id: root

    property bool checked: false
    property string accessibleName // what it switches (a settings Row names it after its label)
    signal toggled

    implicitWidth: 38
    implicitHeight: 22
    activeFocusOnTab: true
    Keys.onPressed: event => {
        if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
            root.toggled();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.CheckBox
    Accessible.name: accessibleName
    Accessible.checkable: true
    Accessible.checked: checked
    Accessible.onToggleAction: root.toggled()
    Accessible.onPressAction: root.toggled()
    FocusRing {
        radius: height / 2
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
