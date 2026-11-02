import QtQuick
import qs.common

// One result in the Material search view: an Expressive segmented list item.
// Items of a section sit 2 px apart as one group (outer corners 20, inner 4);
// the selected one fills with secondaryContainer and turns fully round on
// the fast spatial spring, its leading circle morphing to a cookie.
Item {
    id: root

    required property var result
    property bool selected: false
    property bool firstInGroup: true
    property bool lastInGroup: true
    signal activated

    height: 56

    readonly property bool loading: !!result.loading
    readonly property bool store: result.section === "App Store" && !loading
    readonly property bool picture: !!result.icon && !loading
    readonly property color contentColor: selected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurface

    // 0 = the group's corners, 1 = fully round (selected)
    property real round: selected ? 1 : 0
    Behavior on round { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.005 } }
    function corner(base) {
        const full = height / 2;
        return Math.max(0, Math.min(full, base + (full - base) * round));
    }

    Accessible.role: Accessible.ListItem
    Accessible.name: result.title + (result.subtitle ? ", " + result.subtitle : "")
    Accessible.selected: selected
    Accessible.onPressAction: root.activated()

    Rectangle {
        id: container
        anchors.fill: parent
        topLeftRadius: root.corner(root.firstInGroup ? 20 : 4)
        topRightRadius: topLeftRadius
        bottomLeftRadius: root.corner(root.lastInGroup ? 20 : 4)
        bottomRightRadius: bottomLeftRadius
        color: root.selected ? Theme.m3.secondaryContainer : Theme.m3.surfaceBright
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

        // State layer
        Rectangle {
            anchors.fill: parent
            topLeftRadius: parent.topLeftRadius
            topRightRadius: parent.topRightRadius
            bottomLeftRadius: parent.bottomLeftRadius
            bottomRightRadius: parent.bottomRightRadius
            color: root.contentColor
            opacity: mouse.pressed ? 0.1 : rowHover.hovered ? 0.08 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
        }
    }

    // Leading: the icon in a circle (a cookie when selected)
    MaterialShape {
        id: lead
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 40
        height: 40
        visible: !root.loading
        shape: root.selected ? "cookie9" : "circle"
        color: root.selected ? (root.picture ? Theme.m3.primaryContainer : Theme.m3.primary) : Theme.m3.surfaceContainerHighest
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }
    AppIcon {
        visible: root.picture
        anchors.centerIn: lead
        size: 26
        styled: false
        source: root.picture ? root.result.icon : ""
    }
    Text {
        visible: !!root.result.emoji
        anchors.centerIn: lead
        text: root.result.emoji ?? ""
        font { family: EmojiFont.family; pixelSize: 22 }
    }
    Icon {
        visible: !root.picture && !root.result.emoji && !root.loading
        anchors.centerIn: lead
        name: root.result.glyph ?? "circle"
        fill: root.selected
        size: 20
        color: root.selected ? Theme.m3.onPrimary : Theme.m3.onSurfaceVariant
    }
    // (still looking: Material's loading indicator in the icon's place)
    Spinner {
        visible: root.loading
        anchors.centerIn: lead
        size: 32
    }

    Column {
        anchors { left: parent.left; leftMargin: 60; right: trailing.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 0
        Text {
            width: parent.width
            text: root.result.title
            elide: Text.ElideRight
            color: root.contentColor
            // (emphasized type for the selection)
            font { family: Theme.font.family; pixelSize: 16; weight: root.selected ? Font.Medium : Font.Normal }
        }
        Text {
            readonly property string body: root.store ? (root.result.subtitle ?? "").replace(/^Get · /, "") : (root.result.subtitle ?? "")
            visible: body !== ""
            width: parent.width
            text: body
            elide: Text.ElideRight
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14 }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
    // (hover of the whole row, its buttons included)
    HoverHandler { id: rowHover }

    // Trailing: "Get" for an app from the App Store, Quick Look for a file
    Item {
        id: trailing
        anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
        // (a file keeps the button's room, so its title doesn't jump on hover)
        width: root.store ? get.width : root.result.path ? 40 : 0
        height: 40

        SearchButton {
            id: get
            visible: root.store
            anchors.verticalCenter: parent.verticalCenter
            height: 32
            text: "Get"
            filled: root.selected
            onClicked: root.activated()
        }
        IconButton {
            id: look
            visible: !!root.result.path && (rowHover.hovered || root.selected)
            anchors.verticalCenter: parent.verticalCenter
            width: 40
            height: 40
            icon: "eye"
            iconSize: 20
            iconColor: root.selected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurfaceVariant
            tooltip: "Quick Look"
            onClicked: Panels.quickLook(root.result.path)
        }
    }
}
