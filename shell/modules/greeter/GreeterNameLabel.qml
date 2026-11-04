import QtQuick
import "../../common/phosphor.js" as Phosphor

// The chosen person's name, as text: hovered, a soft pill and a pencil; a
// click types another name in its place (GreeterWindow.editName). Each
// style sets its colours and font.
Rectangle {
    id: root
    required property var win
    property alias text: label.text
    property alias textColor: label.color
    property alias font: label.font
    property color hoverFill
    width: label.implicitWidth + 2 * (12 + 20)
    height: label.implicitHeight + 12
    radius: height / 2
    color: mouse.containsMouse ? hoverFill : "transparent"
    Behavior on color { ColorAnimation { duration: 150 } }
    Text {
        id: label
        anchors.centerIn: parent
    }
    Text {
        anchors { left: label.right; leftMargin: 6; verticalCenter: parent.verticalCenter }
        text: Phosphor.glyphs["pencil-simple"] ?? ""
        color: label.color
        opacity: mouse.containsMouse ? 0.8 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }
        font { family: "Phosphor"; pixelSize: 14 }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.IBeamCursor
        onClicked: root.win.editName()
    }
    Accessible.role: Accessible.Button
    Accessible.name: label.text + ", type another username"
    Accessible.onPressAction: win.editName()
}
