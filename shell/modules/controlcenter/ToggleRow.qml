import QtQuick
import qs.common

// Circle toggle with a title and a status line, e.g. Wi-Fi / network name.
Item {
    id: root

    property alias icon: toggle.icon
    property alias checked: toggle.checked
    property string title
    property string subtitle
    signal clicked
    signal opened // label clicked: open the detail page

    implicitHeight: 36
    implicitWidth: toggle.width + 10 + Math.max(titleText.implicitWidth, subText.implicitWidth)

    CircleToggle {
        id: toggle
        anchors.verticalCenter: parent.verticalCenter
        onClicked: root.clicked()
    }

    MouseArea {
        anchors { left: toggle.right; right: parent.right; top: parent.top; bottom: parent.bottom }
        cursorShape: Qt.PointingHandCursor
        onClicked: root.opened()
    }

    Column {
        anchors { left: toggle.right; leftMargin: 10; right: parent.right; verticalCenter: parent.verticalCenter }
        spacing: 1

        Text {
            id: titleText
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            id: subText
            width: parent.width
            visible: text !== ""
            text: root.subtitle
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }
}
