import QtQuick
import qs.common

// A labelled button in the chat (Search the web, Switch to …): tonal, or
// filled in the accent for the one to press (`primary`). Busy: a spinner in
// place of its icon, and it can't be pressed again. Material: a pill that
// squares a little while pressed.
Squircle {
    id: root

    property string icon
    property string label
    property bool primary: false
    property bool busy: false
    signal clicked

    readonly property color ink: primary ? Theme.color.onAccent : Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label
    implicitWidth: row.implicitWidth + (Theme.material ? 32 : 24)
    implicitHeight: Theme.material ? 40 : 32
    radius: Theme.material ? (mouse.pressed ? 12 : height / 2) : Theme.radius.control
    Behavior on radius { enabled: Theme.material; MaterialSpring { speed: "fast"; epsilon: 0.1 } }
    color: primary ? (Theme.material ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
            : mouse.pressed ? Qt.darker(Theme.accent, 1.15) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.07) : Theme.accent)
        : Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
        : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    // (Glass squishes on press; Material grows: the ElasticControl)
    scale: !Theme.material && mouse.pressed ? 0.97 : 1
    Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
    transform: Theme.material ? pressFx.effects : []
    ElasticControl { id: pressFx; pressed: mouse.pressed; enabled: Theme.material }

    activeFocusOnTab: visible
    Keys.onPressed: event => {
        if (!root.busy && !event.isAutoRepeat && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            root.clicked();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.Button
    Accessible.name: label
    FocusRing { radius: root.radius + 4 }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.material ? 8 : 6
        Item {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.material ? 18 : 14
            height: width
            Icon { anchors.centerIn: parent; visible: !root.busy; name: root.icon; size: parent.width; color: root.ink }
            Spinner { anchors.centerIn: parent; visible: root.busy; size: parent.width; color: root.ink }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: root.ink
            font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.body : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
        }
    }
    MouseArea {
        id: mouse
        anchors { fill: parent; topMargin: -6; bottomMargin: -6 } // (44 px to hit)
        hoverEnabled: true
        cursorShape: root.busy ? Qt.ArrowCursor : Qt.PointingHandCursor
        enabled: !root.busy
        onClicked: root.clicked()
    }
}
