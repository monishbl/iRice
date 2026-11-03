import QtQuick
import qs.common

// Material's play / pause button: a filled shape that morphs on Material's
// spring, round while paused and a rounded square while playing (Material's
// selected toggle shape), squeezing a little while pressed. Material look
// only (Glass draws its own icons).
//   PlayButton { playing: player.isPlaying; onClicked: player.togglePlaying() }
Item {
    id: root

    property bool playing: false
    property int size: 48
    property color color: Theme.m3.primary
    property color iconColor: Theme.m3.onPrimary
    property string label: playing ? "Pause" : "Play"
    signal clicked

    implicitWidth: size
    implicitHeight: size
    opacity: enabled ? 1 : 0.38
    activeFocusOnTab: visible && enabled
    Keys.onPressed: event => {
        if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
            root.clicked();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: root.clicked()
    FocusRing {
        radius: root.height / 2 + 4
    }

    MaterialShape {
        anchors.centerIn: parent
        width: root.size
        height: root.size
        shape: root.playing ? "square" : "circle"
        // (a state layer of the icon's colour on hover and press)
        color: IslandInk.layer(root.color, root.iconColor, mouse)
        transform: pressFx1.effects
        ElasticControl { id: pressFx1; pressed: mouse.pressed }
        // (Material: grows on press: the ElasticControl above)
        Behavior on scale { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.005 } }
    }
    Icon {
        anchors.centerIn: parent
        name: root.playing ? "pause" : "play"
        fill: true
        size: Math.round(root.size * 0.42)
        color: root.iconColor
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
