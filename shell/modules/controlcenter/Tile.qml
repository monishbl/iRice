import QtQuick
import qs.common

// A compact platter with a toggle and a label; the whole tile is clickable.
Platter {
    id: root

    property alias icon: toggle.icon
    property alias checked: toggle.checked
    property string title
    property string subtitle
    property bool hasPage: false // clicking the label opens a page instead of toggling
    signal clicked
    signal opened

    implicitHeight: 52

    CircleToggle {
        id: toggle
        x: 10
        anchors.verticalCenter: parent.verticalCenter
        onClicked: root.clicked()
    }
    Column {
        anchors { left: toggle.right; leftMargin: 8; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
        Text {
            width: parent.width
            text: root.title
            wrapMode: Text.Wrap
            maximumLineCount: root.subtitle ? 1 : 2
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            visible: root.subtitle !== ""
            text: root.subtitle
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }
    MouseArea {
        anchors.fill: parent
        z: -1
        onClicked: root.hasPage ? root.opened() : root.clicked()
    }
}
