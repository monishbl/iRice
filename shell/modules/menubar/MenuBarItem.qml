import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Widgets
import qs.common

// One menu bar entry: an icon and/or label with a rounded highlight on hover and
// while its menu is open.
Item {
    id: root

    property string text
    property string icon
    property string image // icon path, e.g. a tray icon
    property bool tintImage: false // draw the image in the bar's text colour (a distro logo)
    property bool fill: false
    property bool bold: false
    property bool active: false
    property color color: "white"
    property int iconSize: 15
    property Component glyph: null // a drawn icon (battery, Wi-Fi) in place of `icon`
    property int badge: 0 // small count bubble, e.g. unread notifications
    property string menu // popover this item opens; sliding onto it while a menu is open switches to it
    // Unread notifications, drawn inline before the text: bellDot | dot | count | bellCount | off
    property string indicator: "off"
    property int unread: 0
    signal clicked(var mouse)

    Component.onCompleted: Panels.registerBarItem(root)
    Component.onDestruction: Panels.unregisterBarItem(root)

    implicitWidth: content.implicitWidth + 18
    implicitHeight: 24

    Squircle {
        anchors.fill: parent
        radius: Theme.radius.item
        color: Qt.alpha(root.color, root.active ? 0.22 : mouse.pressed ? 0.2 : root.hovered ? 0.12 : 0)
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }

    Row {
        id: content
        anchors.centerIn: parent
        spacing: 6

        Item {
            id: unreadMark
            readonly property bool bell: root.indicator === "bellDot" || root.indicator === "bellCount"
            readonly property bool number: root.indicator === "count" || root.indicator === "bellCount"
            visible: root.unread > 0 && root.indicator !== "off"
            anchors.verticalCenter: parent.verticalCenter
            width: markRow.implicitWidth
            height: 16
            Row {
                id: markRow
                anchors.verticalCenter: parent.verticalCenter
                spacing: 3
                Item {
                    visible: unreadMark.bell
                    width: 15
                    height: 16
                    Icon {
                        anchors.centerIn: parent
                        name: "bell"
                        fill: true
                        size: 14
                        color: root.color
                    }
                    Rectangle {
                        visible: root.indicator === "bellDot"
                        x: 9
                        y: 0
                        width: 7
                        height: 7
                        radius: 3.5
                        color: Theme.accents.blue // unread is status: always the vivid system blue
                        border.width: 1
                        border.color: Qt.alpha("black", 0.25)
                    }
                }
                Rectangle {
                    visible: root.indicator === "dot"
                    anchors.verticalCenter: parent.verticalCenter
                    width: 7
                    height: 7
                    radius: 3.5
                    color: Theme.accents.blue
                }
                // Count as a pill (inline, not an overlay)
                Rectangle {
                    visible: unreadMark.number
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.max(16, countText.implicitWidth + 8)
                    height: 16
                    radius: 8
                    color: root.indicator === "count" ? Theme.accents.blue : "transparent"
                    Text {
                        id: countText
                        anchors.centerIn: parent
                        text: root.unread > 99 ? "99+" : root.unread
                        color: root.indicator === "count" ? "white" : root.color
                        font { family: Theme.font.family; pixelSize: 11; weight: Font.DemiBold; features: { "tnum": 1 } }
                    }
                }
            }
        }

        IconImage {
            visible: root.image !== ""
            anchors.verticalCenter: parent.verticalCenter
            implicitSize: 16
            source: root.image
            layer.enabled: root.tintImage
            layer.effect: MultiEffect {
                colorization: 1
                colorizationColor: root.color
                brightness: 1
            }
        }
        Loader {
            active: root.glyph !== null
            visible: active
            anchors.verticalCenter: parent.verticalCenter
            sourceComponent: root.glyph
        }
        Icon {
            visible: root.icon !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            fill: root.fill
            size: root.iconSize
            color: root.color
        }
        Text {
            visible: root.text !== ""
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            color: root.color
            font.family: Theme.font.family
            font.pixelSize: Theme.font.body
            font.weight: root.bold ? Font.Bold : Font.Medium
            font.features: { "tnum": 1 }
        }
    }

    Rectangle {
        visible: root.badge > 0
        anchors { right: parent.right; top: parent.top; rightMargin: 1; topMargin: 0 }
        width: Math.max(14, badgeText.implicitWidth + 6)
        height: 14
        radius: 7
        color: Theme.color.danger
        Text {
            id: badgeText
            anchors.centerIn: parent
            text: root.badge > 99 ? "99+" : root.badge
            color: "white"
            font { family: Theme.font.family; pixelSize: 9; weight: Font.Bold }
        }
    }

    // While a menu is open its full-screen layer covers the bar, so the
    // hover state can go stale (a faint chip left behind). Trust it again
    // only once the pointer really moves over the item.
    property bool hoverFresh: true
    readonly property bool hovered: mouse.containsMouse && hoverFresh
    Connections {
        target: Panels
        function onOpenChanged() { root.hoverFresh = false; }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: m => root.clicked(m)
        onPositionChanged: root.hoverFresh = true
        onExited: root.hoverFresh = true
    }
}
