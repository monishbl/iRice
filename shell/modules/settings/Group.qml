import QtQuick
import qs.common

// A grouped form section: optional caption above a rounded platter whose rows
// are separated by hairlines.
Column {
    id: root

    property string title
    property string footer
    default property alias rows: body.data

    spacing: 6

    Text {
        visible: root.title !== ""
        leftPadding: 4
        text: root.title
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
    }

    Squircle {
        width: root.width
        height: body.implicitHeight
        radius: 12
        color: Theme.dark ? Qt.rgba(1, 1, 1, 0.05) : "white"
        border.width: 1
        border.color: Theme.color.separator

        Column {
            id: body
            width: parent.width
        }

        // Hairlines between visible rows
        Repeater {
            model: body.children.length
            Rectangle {
                required property int index
                readonly property var row: body.children[index]
                visible: index > 0 && row.visible && row.height > 0
                x: 14
                y: row.y
                width: parent.width - 28
                height: 1
                color: Theme.color.separator
            }
        }
    }

    Text {
        visible: root.footer !== ""
        width: root.width
        leftPadding: 4
        rightPadding: 4
        wrapMode: Text.Wrap
        text: root.footer
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }
}
