import QtQuick
import qs.common

// The close button over a card's top-left corner, like macOS: Glass
// notification cards and island alerts show it while hovered.
Rectangle {
    id: root

    property real reach: 0 // (room round it the pointer still hits)
    signal clicked

    x: -6
    y: -6
    width: 22
    height: 22
    radius: 11
    color: Theme.dark ? Qt.rgba(0.22, 0.22, 0.25, 1) : "white"
    border.width: 1
    border.color: Theme.color.separator
    Icon {
        anchors.centerIn: parent
        name: "x"
        bold: true
        size: 11
        color: Theme.color.secondary
    }
    MouseArea {
        anchors { fill: parent; margins: -root.reach }
        onClicked: root.clicked()
    }
}
