import QtQuick
import qs.common

// Material's checkbox (the Material look only): an 18 px rounded square,
// outlined while open, filled primary with an onPrimary tick once checked,
// a round state layer around it on hover and press. Click, Space or Enter.
//   MaterialCheck { checked: todo.done; onToggled: Todos.update(todo, { done: !todo.done }) }
Item {
    id: root

    property bool checked: false
    property string accessibleName: "Done"
    signal toggled

    implicitWidth: 18
    implicitHeight: 18
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

    // State layer: 40 px round, the colour of what it would become
    Rectangle {
        anchors.centerIn: parent
        width: 36
        height: 36
        radius: 18
        color: Qt.alpha(root.checked ? Theme.m3.primary : Theme.m3.onSurface, mouse.pressed ? 0.1 : 0.08)
        visible: mouse.containsMouse || mouse.pressed
    }
    Rectangle {
        anchors.fill: parent
        radius: 4
        color: root.checked ? Theme.m3.primary : "transparent"
        border.width: root.checked ? 0 : 2
        border.color: Theme.m3.onSurfaceVariant
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        Icon {
            anchors.centerIn: parent
            visible: root.checked
            name: "check"
            bold: true
            size: 14
            color: Theme.m3.onPrimary
            scale: root.checked ? 1 : 0.4
            Behavior on scale { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.01 } }
        }
    }
    FocusRing {
        radius: 8
    }
    MouseArea {
        id: mouse
        anchors { fill: parent; margins: -9 }
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
