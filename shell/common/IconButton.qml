import QtQuick

// Ghost icon button: transparent, fill on hover, darker on press.
Rectangle {
    id: root

    property string icon
    property bool fill: false
    property color iconColor: Theme.color.secondary
    property int iconSize: 16
    property string tooltip
    signal clicked

    implicitWidth: 28
    implicitHeight: 28
    radius: Theme.radius.control - 2
    color: mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fill : "transparent"
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    transform: elastic.effects

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
