import QtQuick
import qs.common

// A form row: label (and optional detail) on the left, a control on the right.
// Material: Android's preference row, 16 px title over a 14 px summary, at
// least 56 high (its tile is drawn by the Group).
Item {
    id: root

    property string label
    property string detail
    property color labelColor: Theme.color.label // (a status line: secondary, or danger for an error)
    property string icon: "" // (optional, before the label: a fingerprint per finger, say)
    default property alias control: slot.data

    width: parent?.width ?? 0
    readonly property int pad: Theme.material ? 20 : 14
    implicitHeight: Theme.material ? Math.max(56, labels.implicitHeight + 24, slot.childrenRect.height + 16) : Math.max(44, labels.implicitHeight + 20, slot.childrenRect.height + 16)

    Icon {
        id: leading
        visible: root.icon !== ""
        anchors { left: parent.left; leftMargin: root.pad; verticalCenter: parent.verticalCenter }
        name: root.icon
        size: Theme.material ? 24 : 20
        color: Theme.material ? Theme.m3.onSurfaceVariant : Theme.color.tertiary
    }
    Column {
        id: labels
        anchors { left: parent.left; leftMargin: root.icon !== "" ? root.pad + leading.size + (Theme.material ? 16 : 12) : root.pad; right: slot.left; rightMargin: Theme.material ? 16 : 12; verticalCenter: parent.verticalCenter }
        spacing: 2
        Text {
            width: parent.width
            text: root.label
            wrapMode: Text.Wrap
            color: root.labelColor
            font { family: Theme.font.family; pixelSize: Theme.material ? 16 : Theme.font.body }
        }
        Text {
            width: parent.width
            visible: root.detail !== ""
            text: root.detail
            wrapMode: Text.Wrap
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.caption }
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
        anchors { right: parent.right; rightMargin: root.pad; verticalCenter: parent.verticalCenter }
        width: childrenRect.width
        height: childrenRect.height
    }
}
