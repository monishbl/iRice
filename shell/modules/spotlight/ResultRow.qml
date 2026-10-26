import QtQuick
import Quickshell.Widgets
import qs.common

// One Spotlight result. Selected row gets the accent fill.
Rectangle {
    id: root

    required property var result
    property bool selected: false
    signal activated

    height: 40
    radius: Theme.radius.control
    color: selected ? Theme.accent : mouse.containsMouse ? Theme.color.fill : "transparent"

    AppIcon {
        id: image
        visible: !!root.result.icon
        x: 10
        anchors.verticalCenter: parent.verticalCenter
        size: 26
        styled: root.result.section === "Applications" || root.result.section === "Top Hit"
        source: root.result.icon ?? ""
    }
    Text {
        visible: !!root.result.emoji
        x: 10
        width: 26
        anchors.verticalCenter: parent.verticalCenter
        horizontalAlignment: Text.AlignHCenter
        text: root.result.emoji ?? ""
        font { family: EmojiFont.family; pixelSize: 20 }
    }
    Icon {
        visible: !root.result.icon && !root.result.emoji
        x: 10
        width: 26
        anchors.verticalCenter: parent.verticalCenter
        name: root.result.glyph ?? "circle"
        size: 18
        color: root.selected ? Theme.color.onAccent : Theme.color.secondary
    }

    Row {
        anchors { left: parent.left; leftMargin: 46; right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 8
        Text {
            id: title
            width: Math.min(implicitWidth, parent.width)
            text: root.result.title
            elide: Text.ElideRight
            color: root.selected ? Theme.color.onAccent : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.Medium }
        }
        Text {
            width: parent.width - title.width - 8
            anchors.baseline: title.baseline
            text: root.result.subtitle ?? ""
            elide: Text.ElideRight
            color: root.selected ? Qt.alpha(Theme.color.onAccent, 0.75) : Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: root.activated()
    }
    // Files: Quick Look (also Shift+Space)
    IconButton {
        visible: !!root.result.path && (mouse.containsMouse || hovered || root.selected)
        readonly property bool hovered: false
        anchors { right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
        icon: "eye"
        iconColor: root.selected ? Theme.color.onAccent : Theme.color.secondary
        tooltip: "Quick Look"
        onClicked: Panels.quickLook(root.result.path)
    }
}
