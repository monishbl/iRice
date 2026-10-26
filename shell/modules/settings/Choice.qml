import QtQuick
import qs.common

// Pop-up button for one of several { value, label } options. Its list opens
// in the page's ActionMenu (`menu`), so it is never hidden under later rows.
Rectangle {
    id: root

    required property ActionMenu menu
    property var options: []
    property var value
    property string placeholder: "Choose"
    signal chosen(var value)

    function same(a, b) {
        return JSON.stringify(a) === JSON.stringify(b);
    }
    readonly property var current: options.find(o => same(o.value, value)) ?? null
    readonly property bool open: menu.shown && menu.anchor === root

    implicitWidth: Math.max(96, label.implicitWidth + 40)
    implicitHeight: 28
    radius: Theme.radius.control - 2
    opacity: enabled ? 1 : 0.45
    color: mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse || open ? Theme.color.fillHover : Theme.color.fill
    activeFocusOnTab: true
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

    function show() {
        menu.open(root, options.map(o => ({ text: o.label, checked: same(o.value, root.value), action: () => root.chosen(o.value) })));
    }
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Down) {
            show();
            event.accepted = true;
        }
    }

    Rectangle {
        anchors { fill: parent; margins: -3 }
        visible: root.activeFocus
        radius: root.radius + 3
        color: "transparent"
        border.width: 2
        border.color: Theme.accent
    }
    Text {
        id: label
        anchors { left: parent.left; leftMargin: 10; right: caret.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
        text: root.current?.label ?? root.placeholder
        elide: Text.ElideRight
        color: root.current ? Theme.color.label : Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }
    Icon {
        id: caret
        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
        name: "caret-up-down"
        bold: true
        size: 11
        color: Theme.color.secondary
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            root.forceActiveFocus();
            root.open ? root.menu.close() : root.show();
        }
    }
}
