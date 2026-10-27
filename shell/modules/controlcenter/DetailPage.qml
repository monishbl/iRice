import QtQuick
import qs.common

// A Control Center sub-page: back button, title, optional switch, then content.
Column {
    id: root

    property string title
    property bool showBack: true // false when shown as a menu bar menu
    property bool hasSwitch: false
    property bool switchOn: false
    property bool hasRefresh: false // a refresh button (lists that change on their own)
    signal back
    signal switchToggled
    signal refreshRequested
    // The pointer is over the page: lists hold still so rows don't move under it
    readonly property bool hovered: pageHover.hovered
    HoverHandler { id: pageHover }
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
        Rectangle {
            id: refreshButton
            visible: root.hasRefresh
            anchors { right: root.hasSwitch ? pageSwitch.left : parent.right; rightMargin: root.hasSwitch ? 10 : 0; verticalCenter: parent.verticalCenter }
            width: 28
            height: 28
            radius: 14
            color: refreshMouse.pressed ? Theme.color.fillPressed : refreshMouse.containsMouse ? Theme.color.fillHover : "transparent"
            Icon {
                id: refreshIcon
                anchors.centerIn: parent
                name: "arrow-clockwise"
                size: 14
                color: Theme.color.secondary
                RotationAnimation on rotation { id: spin; running: false; from: 0; to: 360; duration: 500; easing.type: Easing.OutCubic }
            }
            MouseArea {
                id: refreshMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    spin.restart();
                    root.refreshRequested();
                }
            }
        }
        Switch {
            id: pageSwitch
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
