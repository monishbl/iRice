import QtQuick
import Quickshell.Widgets
import qs.common
import qs.services

// Current track with art and transport controls.
Platter {
    id: root

    readonly property var player: Media.player
    implicitHeight: 72

    ClippingRectangle {
        id: art
        x: 12
        anchors.verticalCenter: parent.verticalCenter
        width: 48
        height: 48
        radius: 10
        color: Theme.color.toggleOff

        Image {
            anchors.fill: parent
            source: root.player?.trackArtUrl ?? ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(96, 96)
            asynchronous: true
        }
        Icon {
            anchors.centerIn: parent
            visible: !root.player?.trackArtUrl
            name: "music-notes"
            fill: true
            size: 22
            color: Theme.color.secondary
        }
    }

    Column {
        anchors { left: art.right; leftMargin: 12; right: controls.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
        spacing: 2
        Text {
            width: parent.width
            text: root.player?.trackTitle || root.player?.identity || "Not playing"
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: root.player?.trackArtist ?? ""
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }

    Row {
        id: controls
        anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
        spacing: 2

        Repeater {
            model: [
                // To a Chromecast or Google TV (Control Center's Cast page)
                ...(Cast.devices.length > 0 || Cast.active ? [{ icon: "screencast", fill: Cast.active, action: () => Panels.ccPage = "cast", enabled: true }] : []),
                { icon: "skip-back", action: () => root.player?.previous(), enabled: root.player?.canGoPrevious ?? false },
                { icon: root.player?.isPlaying ? "pause" : "play", action: () => root.player?.togglePlaying(), enabled: root.player?.canTogglePlaying ?? false },
                { icon: "skip-forward", action: () => root.player?.next(), enabled: root.player?.canGoNext ?? false }
            ]

            Rectangle {
                required property var modelData
                width: 32
                height: 32
                radius: 16
                color: mouse.containsMouse ? Theme.color.fillHover : "transparent"
                opacity: modelData.enabled ? 1 : 0.35

                Icon {
                    anchors.centerIn: parent
                    name: parent.modelData.icon
                    fill: parent.modelData.fill ?? true
                    size: 18
                    color: parent.modelData.icon === "screencast" && Cast.active ? Theme.accent : Theme.color.label
                }
                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    enabled: parent.modelData.enabled
                    onClicked: parent.modelData.action()
                }
            }
        }
    }
}
