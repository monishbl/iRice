import QtQuick
import qs.common

// Pop-up button for one of several { value, label } options. Its list opens
// in the page's ActionMenu (`menu`), so it is never hidden under later rows.
// Material: a filled field like Dropdown's, its underline primary while open.
Squircle {
    id: root

    required property ActionMenu menu
    property var options: []
    property var value
    property string placeholder: "Choose"
    property string accessibleName
    signal chosen(var value)

    function same(a, b) {
        return JSON.stringify(a) === JSON.stringify(b);
    }
    readonly property var current: options.find(o => same(o.value, value)) ?? null
    readonly property bool open: menu.shown && menu.anchor === root

    implicitWidth: Math.max(Theme.material ? 140 : 96, label.implicitWidth + (Theme.material ? 52 : 40))
    implicitHeight: Theme.material ? 40 : 28
    radius: Theme.radius.control - 2
    opacity: enabled ? 1 : Theme.material ? 0.38 : 0.45
    color: Theme.material ? "transparent" : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse || open ? Theme.color.fillHover : Theme.color.fill
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

    Accessible.role: Accessible.ComboBox
    Accessible.name: accessibleName
    Accessible.description: current?.label ?? placeholder
    Accessible.onPressAction: show()
    FocusRing {
        radius: root.radius + 4
    }
    Loader {
        anchors.fill: parent
        active: Theme.material
        sourceComponent: Rectangle {
            topLeftRadius: 4
            topRightRadius: 4
            color: Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
            Rectangle {
                anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                height: root.open ? 2 : 1
                color: root.open ? Theme.m3.primary : Theme.m3.onSurfaceVariant
            }
        }
    }
    Text {
        id: label
        anchors { left: parent.left; leftMargin: Theme.material ? 12 : 10; right: caret.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
        text: root.current?.label ?? root.placeholder
        elide: Text.ElideRight
        color: root.current ? Theme.color.label : Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body }
    }
    Icon {
        id: caret
        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
        name: Theme.material ? "caret-down" : "caret-up-down"
        bold: !Theme.material
        size: Theme.material ? 20 : 11
        rotation: Theme.material && root.open ? 180 : 0
        Behavior on rotation { enabled: Theme.material; NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
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
