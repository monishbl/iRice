import QtQuick
import qs.common

// The first result in the Material search view when it's the top hit, an
// answer (a sum, a date, a conversion, a definition) or a thing to do: a
// hero card, bigger than the rows, in a bold container (primaryContainer
// for the top hit and things to do, tertiaryContainer for answers), with
// the key action as a large round button. Selected, it turns fully round.
Item {
    id: root

    required property var result
    property bool selected: false
    signal activated

    readonly property bool topHit: result.section === "Top Hit"
    readonly property bool action: result.section === "Do"
    readonly property bool answer: !topHit && !action
    // (an app from the App Store as the top hit: its button says "Get")
    readonly property bool store: topHit && /^Get · /.test(result.subtitle ?? "")
    readonly property bool picture: !!result.icon
    readonly property color containerColor: answer ? Theme.m3.tertiaryContainer : Theme.m3.primaryContainer
    readonly property color contentColor: answer ? Theme.m3.onTertiaryContainer : Theme.m3.onPrimaryContainer
    readonly property color strongColor: answer ? Theme.m3.tertiary : Theme.m3.primary
    readonly property color onStrongColor: answer ? Theme.m3.onTertiary : Theme.m3.onPrimary

    height: 96

    property real round: selected ? 1 : 0
    Behavior on round { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.005 } }

    Accessible.role: Accessible.ListItem
    Accessible.name: result.title + (result.subtitle ? ", " + result.subtitle : "")
    Accessible.selected: selected
    Accessible.onPressAction: root.activated()

    Rectangle {
        id: card
        anchors.fill: parent
        radius: Math.max(0, Math.min(height / 2, 28 + (height / 2 - 28) * root.round))
        color: root.containerColor
        // State layer
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: root.contentColor
            opacity: mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
        }
    }

    // Leading: a big shape; it changes with the selection, as Android's do
    MaterialShape {
        id: lead
        x: 16
        anchors.verticalCenter: parent.verticalCenter
        width: 64
        height: 64
        shape: root.selected ? "sunny" : root.picture ? "cookie12" : "cookie9"
        color: root.picture ? Theme.m3.surfaceBright : root.strongColor
    }
    AppIcon {
        visible: root.picture
        anchors.centerIn: lead
        size: 40
        styled: false
        source: root.picture ? root.result.icon : ""
    }
    Icon {
        visible: !root.picture
        anchors.centerIn: lead
        name: root.result.glyph ?? "sparkle"
        fill: true
        size: 28
        color: root.onStrongColor
    }

    Column {
        anchors { left: lead.right; leftMargin: 16; right: key.left; rightMargin: 16; verticalCenter: parent.verticalCenter }
        spacing: 2
        Text {
            visible: !root.action
            width: parent.width
            text: root.topHit ? "Top hit" : root.result.section
            elide: Text.ElideRight
            color: root.contentColor
            opacity: 0.8
            font { family: Theme.font.family; pixelSize: 12; weight: Font.Medium; letterSpacing: 0.5 }
        }
        Text {
            width: parent.width
            text: root.result.title
            elide: Text.ElideRight
            color: root.contentColor
            // (an answer reads large; a thing to do reads like a sentence)
            font { family: Theme.font.family; pixelSize: root.answer ? 24 : root.action ? 18 : 22; weight: root.answer ? Font.DemiBold : Font.Medium; features: { "tnum": 1 } }
        }
        Text {
            readonly property string hint: root.topHit ? "" : root.action ? "Enter to do it" : root.result.section === "Dictionary" ? "Enter opens it" : "Enter copies it"
            readonly property string body: root.store ? (root.result.subtitle ?? "").replace(/^Get · /, "") : (root.result.subtitle ?? "")
            visible: text !== ""
            width: parent.width
            text: [body, hint].filter(x => x !== "").join(" · ")
            elide: Text.ElideRight
            color: root.contentColor
            opacity: 0.8
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

    // The key action, much bigger than the rest: open, do, copy, or get
    Item {
        id: key
        anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
        width: root.store ? getButton.width : 56
        height: 56

        SearchButton {
            id: getButton
            visible: root.store
            anchors.verticalCenter: parent.verticalCenter
            height: 40
            text: "Get"
            filled: true
            onClicked: root.activated()
        }
        Rectangle {
            id: go
            visible: !root.store
            anchors.fill: parent
            // (a round button that squares off a little while pressed)
            transform: pressFx1.effects
            ElasticControl { id: pressFx1; pressed: goMouse.pressed }
            property real corner: goMouse.pressed ? 16 : height / 2
            Behavior on corner { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
            radius: Math.max(0, Math.min(height / 2, corner))
            color: root.strongColor
            Accessible.role: Accessible.Button
            Accessible.name: root.answer ? (root.result.section === "Dictionary" ? "Open" : "Copy") : "Open"
            Rectangle {
                anchors.fill: parent
                radius: parent.radius
                color: root.onStrongColor
                opacity: goMouse.pressed ? 0.1 : goMouse.containsMouse ? 0.08 : 0
            }
            Icon {
                anchors.centerIn: parent
                name: root.answer ? (root.result.section === "Dictionary" ? "arrow-square-out" : "copy") : "arrow-up-right"
                size: 24
                color: root.onStrongColor
            }
            MouseArea {
                id: goMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.activated()
            }
        }
    }
}
