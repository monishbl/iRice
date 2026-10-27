import QtQuick
import QtQuick.Controls as QQC

// Pop-up button: shows the current choice, opens a list of { value, label }.
// The list is a Popup, drawn above the whole window: as a child of the row
// it sat under every row after it, which drew over it and took its clicks.
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
        onClicked: {
            if (menu.visible)
                return menu.close();
            menu.x = root.width - menu.width; // right edges line up (set now: a window popup is placed once, as it opens)
            menu.open();
        }
    }

    QQC.Popup {
        id: menu
        y: root.height + 4
        width: Math.max(root.width, 200)
        height: Math.min(280, column.implicitHeight + 10)
        padding: 5
        margins: 12 // stays inside the window (moves up when near the bottom)
        modal: false
        closePolicy: QQC.Popup.CloseOnEscape | QQC.Popup.CloseOnPressOutsideParent
        background: GlassPopupBackground {}

        contentItem: Flickable {
            id: flick
            clip: true
            contentHeight: column.implicitHeight
            Column {
                id: column
                width: menu.availableWidth
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
                                menu.close();
                                root.chosen(parent.modelData.value);
                            }
                        }
                    }
                }
            }
            ScrollIndicator {
                target: flick
                x: flick.width - width
                height: flick.height
            }
        }
    }
}
