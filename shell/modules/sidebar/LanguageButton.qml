import QtQuick
import QtQuick.Controls as QQC
import qs.common

// Pill button that opens a scrollable list of [code, name] options below it.
// The list is a Popup (above the whole window): inside the row it sat under
// the text boxes after it, which drew over it and took its clicks.
Rectangle {
    id: root

    property string label
    property var options: []
    signal chosen(string code)

    height: 30
    radius: 15
    color: mouse.containsMouse || list.visible ? Theme.color.fillHover : Theme.color.fill

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
        onClicked: list.visible ? list.close() : list.open()
    }

    QQC.Popup {
        id: list
        y: root.height + 4
        width: root.width
        height: Math.min(260, view.contentHeight + 10)
        padding: 5
        margins: 8
        closePolicy: QQC.Popup.CloseOnEscape | QQC.Popup.CloseOnPressOutsideParent
        background: GlassPopupBackground {}

        contentItem: ListView {
            id: view
            clip: true
            model: root.options
            delegate: Squircle {
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
                        list.close();
                        root.chosen(parent.modelData[0]);
                    }
                }
            }
        }
    }
}
