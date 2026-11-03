import QtQuick
import qs.common

// The Material cards' top right (MaterialNotificationCard, MaterialAlertCard):
// close, while the card is hovered or the button has the focus, and a stack's
// count and chevron to expand it.
Row {
    id: root

    property bool hovered: false
    property string closeName
    property int groupCount: 1
    property bool groupOpen: false
    signal closeClicked
    signal groupToggled

    spacing: 4
    IconButton {
        width: 28
        height: 28
        visible: root.hovered || activeFocus
        icon: "x"
        iconSize: 16
        iconColor: Theme.m3.onSurfaceVariant
        accessibleName: root.closeName
        onClicked: root.closeClicked()
    }
    MaterialExpander {
        visible: root.groupCount > 1
        count: root.groupCount
        open: root.groupOpen
        onClicked: root.groupToggled()
    }
}
