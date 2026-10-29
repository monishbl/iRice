import QtQuick
import qs.common

// A form row: label (and optional detail) on the left, a control on the right.
Item {
    id: root

    property string label
    property string detail
    property color labelColor: Theme.color.label // (a status line: secondary, or danger for an error)
    default property alias control: slot.data

    width: parent?.width ?? 0
    implicitHeight: Math.max(44, labels.implicitHeight + 20, slot.childrenRect.height + 16)

    Column {
        id: labels
        anchors { left: parent.left; leftMargin: 14; right: slot.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 2
        Text {
            width: parent.width
            text: root.label
            wrapMode: Text.Wrap
            color: root.labelColor
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        Text {
            width: parent.width
            visible: root.detail !== ""
            text: root.detail
            wrapMode: Text.Wrap
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }

    // Controls in the row are named after its label for screen readers
    // (unless they have a name of their own)
    function nameControls() {
        for (const c of slot.children)
            if (c.accessibleName !== undefined && c.accessibleName === "")
                c.accessibleName = Qt.binding(() => root.label);
    }
    Component.onCompleted: nameControls()

    Item {
        id: slot
        anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
        width: childrenRect.width
        height: childrenRect.height
    }
}
