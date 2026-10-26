import QtQuick
import qs.common

// A Control Center sub-page: back button, title, optional switch, then content.
Column {
    id: root

    property string title
    property bool showBack: true // false when shown as a menu bar menu
    property bool hasSwitch: false
    property bool switchOn: false
    signal back
    signal switchToggled
    default property alias content: body.data

    spacing: 10

    Item {
        width: parent.width
        height: 30
        visible: root.showBack || root.title !== "" || root.hasSwitch

        Rectangle {
            id: backButton
            visible: root.showBack
            width: root.showBack ? 30 : 0
            height: 30
            radius: 15
            color: backMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
            Icon { anchors.centerIn: parent; name: "caret-left"; bold: true; size: 14 }
            MouseArea {
                id: backMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.back()
            }
        }
        Text {
            anchors { left: backButton.right; leftMargin: root.showBack ? 10 : 2; verticalCenter: parent.verticalCenter }
            text: root.title
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
        }
        Switch {
            visible: root.hasSwitch
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            checked: root.switchOn
            onToggled: root.switchToggled()
        }
    }

    Column {
        id: body
        width: parent.width
        spacing: 2
    }
}
