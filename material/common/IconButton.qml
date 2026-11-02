import QtQuick

// Ghost icon button: transparent, fill on hover, darker on press. Material:
// round, its state layer the icon's colour, squaring off while pressed.
Squircle {
    id: root

    property string icon
    property bool fill: false
    property color iconColor: Theme.color.secondary
    property int iconSize: 16
    property string tooltip
    property string accessibleName: tooltip || icon // icon-only: its tooltip names it
    signal clicked

    implicitWidth: 28
    implicitHeight: 28
    radius: Theme.material ? (mouse.pressed ? Math.min(8, height / 2) : height / 2) : Theme.radius.control - 2
    Behavior on radius { enabled: Theme.material; NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
    color: Theme.material ? (mouse.pressed ? Qt.alpha(Theme.m3.onSurfaceVariant, 0.1) : mouse.containsMouse ? Qt.alpha(Theme.m3.onSurfaceVariant, 0.08) : "transparent")
        : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fill : "transparent"
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    transform: elastic.effects
    activeFocusOnTab: visible && enabled
    Keys.onPressed: event => {
        if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
            root.clicked();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.Button
    Accessible.name: accessibleName
    Accessible.onPressAction: root.clicked()
    FocusRing {
        radius: root.radius + 4
    }

    Icon {
        anchors.centerIn: parent
        name: root.icon
        fill: root.fill
        size: root.iconSize
        color: root.iconColor
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    ElasticControl {
        id: elastic
        pressed: mouse.pressed
    }
}
