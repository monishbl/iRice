import QtQuick
import qs.common
import qs.services

// Now Playing. Small: art and the track. Medium: art, track and controls.
Item {
    id: root

    property string size: "m"
    property bool preview: false
    readonly property var player: Media.player

    Squircle {
        id: art
        width: root.size === "s" ? 64 : parent.height
        height: width
        radius: 12
        color: Theme.material ? "transparent" : Theme.color.fill
        clip: true
        Image {
            id: artImage
            anchors.fill: parent
            source: Media.artUrl(root.player)
            retainWhileLoading: true
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(art.width * 2, art.width * 2)
            asynchronous: true
        }
        // Material: the art cut to a shape that morphs as it plays and
        // pauses, as Android's media widget does
        Loader {
            anchors.fill: parent
            active: Theme.material
            sourceComponent: MaterialShape {
                shape: root.player?.isPlaying ? "cookie9" : "square"
                color: Theme.m3.surfaceContainerHighest
                source: artImage.status === Image.Ready ? artImage : null
            }
        }
        Icon {
            anchors.centerIn: parent
            visible: Media.artUrl(root.player) === ""
            name: "music-notes"
            size: 26
            color: Theme.color.tertiary
        }
    }

    Column {
        anchors { left: root.size === "s" ? parent.left : art.right; leftMargin: root.size === "s" ? 0 : 14; right: parent.right }
        y: root.size === "s" ? parent.height - height : 4
        spacing: 2
        Text {
            width: parent.width
            textFormat: Text.PlainText
            text: root.player?.trackTitle || "Not Playing"
            elide: Text.ElideRight
            color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            textFormat: Text.PlainText
            text: root.player?.trackArtist ?? ""
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }

    // Material: play/pause is the filled primary button, round while
    // paused, squaring off while it plays (behind the middle control; the
    // controls are rebuilt when it toggles, so the shape lives out here)
    Rectangle {
        visible: Theme.material && controls.visible
        x: controls.x + 40
        y: controls.y
        width: 40
        height: 40
        radius: Media.player?.isPlaying ? 12 : 20
        color: !Theme.material ? "transparent" : controls.enabled ? Theme.m3.primary : Qt.alpha(Theme.m3.onSurface, 0.12)
        Behavior on radius { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
    }
    Row {
        id: controls
        visible: root.size !== "s"
        anchors { left: art.right; leftMargin: 6; bottom: parent.bottom }
        enabled: !root.preview && !!root.player
        Repeater {
            model: [
                { icon: "skip-back", act: () => Media.player?.previous() },
                { icon: Media.player?.isPlaying ? "pause" : "play", act: () => Media.player?.togglePlaying() },
                { icon: "skip-forward", act: () => Media.player?.next() }
            ]
            IconButton {
                required property var modelData
                required property int index
                width: 40
                height: 40
                icon: modelData.icon
                fill: true
                iconSize: 20
                iconColor: !Theme.material ? Theme.color.label : !enabled ? Qt.alpha(Theme.m3.onSurface, 0.38) : index === 1 ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer
                onClicked: modelData.act()
            }
        }
    }
}
