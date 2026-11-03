import QtQuick

// Material's filled field: a container with 4 px top corners and an underline
// that turns primary and thicker while the field is active. TextField's and
// Dropdown's look in the Material look (Glass draws its own), loaded only
// there:
//   Loader { anchors.fill: parent; active: Theme.material; sourceComponent: FilledField { active: input.activeFocus } }
Rectangle {
    id: root

    property bool active: false // typing, or the list open
    property bool hovered: hover.hovered

    topLeftRadius: 4
    topRightRadius: 4
    color: hovered ? Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHighest
    HoverHandler {
        id: hover
    }
    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: root.active ? 2 : 1
        color: root.active ? Theme.m3.primary : Theme.m3.onSurfaceVariant
    }
}
