import QtQuick
import qs.common

// A Material 3 Expressive button for the Material search view: a pill that
// squares off a little while pressed (round → 12, on the fast spatial
// spring). Filled (primary) for the key action, tonal (secondaryContainer)
// otherwise: "Get" on App Store rows, "Copy answer" under an AI answer.
Rectangle {
    id: root

    property string text
    property bool filled: false
    property color containerColor: filled ? Theme.m3.primary : Theme.m3.secondaryContainer
    property color contentColor: filled ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer
    signal clicked

    implicitHeight: 40
    implicitWidth: label.implicitWidth + 2 * Math.round(height * 0.6)
    property real corner: mouse.pressed ? Math.min(12, height / 2) : height / 2
    Behavior on corner { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
    radius: Math.max(0, Math.min(height / 2, corner))
    color: containerColor
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

    activeFocusOnTab: false
    Accessible.role: Accessible.Button
    Accessible.name: text
    Accessible.onPressAction: root.clicked()

    // State layer: hover 8%, pressed 10% of the content colour
    Rectangle {
        anchors.fill: parent
        radius: parent.radius
        color: root.contentColor
        opacity: mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
    }
    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        color: root.contentColor
        font { family: Theme.font.family; pixelSize: root.height >= 40 ? 14 : 13; weight: Font.Medium }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
