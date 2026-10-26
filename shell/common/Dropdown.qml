import QtQuick

// Pop-up button: shows the current choice, opens a list of { value, label }.
Rectangle {
    id: root

    property var options: []
    property var value
    signal chosen(var value)

    readonly property string currentLabel: options.find(o => o.value === value)?.label ?? String(value ?? "")

    implicitWidth: Math.max(140, label.implicitWidth + 40)
    implicitHeight: 28
    radius: Theme.radius.control - 2
    color: mouse.containsMouse || menu.visible ? Theme.color.fillHover : Theme.color.fill
    z: menu.visible ? 100 : 0

    Text {
        id: label
        anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
        text: root.currentLabel
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }
    Icon {
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
        onClicked: menu.visible = !menu.visible
    }

    Rectangle {
        id: menu
        visible: false
        y: root.height + 4
        anchors.right: parent.right
        width: Math.max(root.width, 200)
        height: Math.min(280, column.implicitHeight + 10)
        radius: 12
        color: Theme.dark ? Qt.rgba(0.16, 0.16, 0.18, 0.99) : Qt.rgba(0.99, 0.99, 1, 0.99)
        border.width: 1
        border.color: Theme.color.separator
        clip: true

        Flickable {
            anchors { fill: parent; margins: 5 }
            contentHeight: column.implicitHeight
            Column {
                id: column
                width: parent.width
                Repeater {
                    model: root.options
                    Rectangle {
                        required property var modelData
                        width: column.width
                        height: 28
                        radius: 7
                        color: itemMouse.containsMouse ? Theme.accent : "transparent"
                        Icon {
                            x: 6
                            anchors.verticalCenter: parent.verticalCenter
                            visible: parent.modelData.value === root.value
                            name: "check"
                            bold: true
                            size: 11
                            color: itemMouse.containsMouse ? Theme.color.onAccent : Theme.color.label
                        }
                        Text {
                            x: 24
                            anchors.verticalCenter: parent.verticalCenter
                            text: parent.modelData.label
                            color: itemMouse.containsMouse ? Theme.color.onAccent : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                        MouseArea {
                            id: itemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                menu.visible = false;
                                root.chosen(parent.modelData.value);
                            }
                        }
                    }
                }
            }
        }
    }
}
