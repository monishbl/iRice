import QtQuick
import qs.common

// A stack's expander in the Material look (Android's notification expander):
// how many it holds and a chevron in a small tonal pill; open, just the
// chevron, turned up. Click, Space or Enter.
Rectangle {
    id: root

    property int count: 0
    property bool open: false
    signal clicked

    implicitWidth: row.implicitWidth + 16
    implicitHeight: 28
    radius: height / 2
    color: mouse.pressed ? Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, 0.1)) : mouse.containsMouse ? Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHighest
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    activeFocusOnTab: visible
    Keys.onReturnPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()
    Accessible.role: Accessible.Button
    Accessible.name: open ? "Show less" : "Show all " + count
    FocusRing { radius: root.radius + 5 }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.open && root.count > 1
            text: root.count
            color: Theme.m3.onSurface
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
        }
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: "caret-down"
            size: 18
            color: Theme.m3.onSurface
            rotation: root.open ? 180 : 0
            Behavior on rotation { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.5 } }
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
