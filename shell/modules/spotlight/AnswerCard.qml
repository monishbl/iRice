import QtQuick
import qs.common

// Spotlight's first result when it answers or does something (a sum, a
// date, a time zone, a conversion, "remind me…", "timer 10 min"): a tall
// card with a coloured tile, the answer large, and what Enter does.
Squircle {
    id: root

    required property var result
    property bool selected: false
    signal activated

    readonly property bool action: result.section === "Do"
    // (a colour per kind of answer; things to do take the accent)
    readonly property color tone: action ? Theme.accent
        : ({ math: Theme.accents.orange, date: Theme.accents.red, time: Theme.accents.blue, unit: Theme.accents.green, currency: Theme.accents.green })[result.kind ?? ""]
        ?? (result.section === "Calculator" ? Theme.accents.orange : result.section === "Conversion" ? Theme.accents.green : result.section === "Dictionary" ? Theme.accents.purple : Theme.accent)

    height: 84
    radius: Theme.radius.menu + 2
    color: selected ? Theme.accentTint : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
    border.width: selected ? 1 : 0
    border.color: Qt.alpha(Theme.accent, 0.5)
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

    Squircle {
        id: tile
        x: 16
        anchors.verticalCenter: parent.verticalCenter
        width: 52
        height: 52
        radius: 14
        color: root.tone
        Icon {
            anchors.centerIn: parent
            name: root.result.glyph ?? "sparkle"
            fill: true
            size: 26
            color: "white"
        }
    }
    Column {
        anchors { left: tile.right; leftMargin: 16; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
        spacing: 2
        Text {
            width: parent.width
            text: root.result.title
            elide: Text.ElideRight
            color: Theme.color.label
            // (an answer reads large; a thing to do reads like a sentence)
            font { family: Theme.font.family; pixelSize: root.action ? 18 : 24; weight: Font.DemiBold; features: { "tnum": 1 } }
        }
        Text {
            width: parent.width
            text: [root.result.subtitle ?? "", root.action ? "Enter to do it" : root.result.section === "Dictionary" ? "Enter opens it" : "Enter copies it"].filter(x => x !== "").join(" · ")
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: 13 }
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
