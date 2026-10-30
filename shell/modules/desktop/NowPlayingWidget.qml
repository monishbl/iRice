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
        color: Theme.color.fill
        clip: true
        Image {
            anchors.fill: parent
            source: root.player?.trackArtUrl ?? ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(art.width * 2, art.width * 2)
            asynchronous: true
        }
        Icon {
            anchors.centerIn: parent
            visible: !(root.player?.trackArtUrl)
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
            text: root.player?.trackTitle || "Not Playing"
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            text: root.player?.trackArtist ?? ""
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }

    Row {
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
                width: 40
                height: 40
                icon: modelData.icon
                fill: true
                iconSize: 20
                iconColor: Theme.color.label
                onClicked: modelData.act()
            }
        }
    }
}
