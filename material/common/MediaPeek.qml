import QtQuick
import Quickshell.Widgets
import qs.common
import qs.services

// Android 17's media carousel, the other players: beside the current card,
// each one waiting is a narrow card of its own (its album art, else its
// app's icon), with a dot while it plays. A click, or a swipe on the
// current card (MediaCarousel), brings it to the front (Media.pinned, so
// the island and every player view follow).
//   MediaPeek { player: p; height: 136 }
Item {
    id: root

    required property var player
    property real radius: 20
    readonly property bool playing: player?.isPlaying ?? false
    width: 48

    ClippingRectangle {
        id: frame
        anchors.fill: parent
        radius: Math.min(root.radius, width / 2)
        color: Theme.m3.surfaceContainerHighest
        Image {
            id: art
            anchors.fill: parent
            source: Media.artUrl(root.player)
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(96, 272)
            asynchronous: true
            visible: status === Image.Ready
        }
        Rectangle {
            anchors.fill: parent
            visible: art.visible
            color: Qt.alpha(Theme.m3.scrim, mouse.containsMouse ? 0.12 : 0.28)
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        }
        AppIcon {
            anchors.centerIn: parent
            visible: !art.visible
            size: 28
            styled: false
            source: Apps.iconFor(root.player?.desktopEntry || root.player?.identity || "")
        }
        // Playing: a dot at the foot, in primary
        Rectangle {
            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 12 }
            width: 8
            height: 8
            radius: 4
            visible: root.playing
            color: Theme.m3.primary
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: Media.pinned = root.player
    }
    activeFocusOnTab: true
    Keys.onPressed: e => {
        if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
            Media.pinned = root.player;
            e.accepted = true;
        }
    }
    FocusRing {
        radius: root.radius + 4
    }
    Accessible.role: Accessible.Button
    Accessible.name: "Switch to " + (root.player?.identity || "player") + (root.player?.trackTitle ? ": " + root.player.trackTitle : "")
}
