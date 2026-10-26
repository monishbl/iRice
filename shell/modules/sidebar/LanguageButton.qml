import QtQuick
import qs.common

// Pill button that opens a scrollable list of [code, name] options below it.
Rectangle {
    id: root

    property string label
    property var options: []
    signal chosen(string code)

    height: 30
    radius: 15
    color: mouse.containsMouse || list.visible ? Theme.color.fillHover : Theme.color.fill
    z: list.visible ? 10 : 0

    Row {
        anchors.centerIn: parent
        spacing: 6
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
        }
        Icon { anchors.verticalCenter: parent.verticalCenter; name: "caret-down"; bold: true; size: 10; color: Theme.color.secondary }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: list.visible = !list.visible
    }

    Rectangle {
        id: list
        visible: false
        y: root.height + 4
        width: root.width
        height: Math.min(260, view.contentHeight + 10)
        radius: 12
        color: Theme.dark ? Qt.rgba(0.14, 0.14, 0.16, 0.98) : Qt.rgba(0.98, 0.98, 0.99, 0.98)
        border.width: 1
        border.color: Theme.color.separator

        ListView {
            id: view
            anchors { fill: parent; margins: 5 }
            clip: true
            model: root.options
            delegate: Rectangle {
                required property var modelData
                width: view.width
                height: 28
                radius: 7
                color: itemMouse.containsMouse ? Theme.accent : "transparent"
                Text {
                    x: 10
                    anchors.verticalCenter: parent.verticalCenter
                    text: parent.modelData[1]
                    color: itemMouse.containsMouse ? Theme.color.onAccent : Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
                MouseArea {
                    id: itemMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                        list.visible = false;
                        root.chosen(parent.modelData[0]);
                    }
                }
            }
        }
    }
}
