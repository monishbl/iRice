import QtQuick
import Quickshell
import Quickshell.Services.Mpris
import Quickshell.Widgets
import qs.common
import qs.glass
import qs.services

// Media popover from the menu bar's Now Playing item: art, title, seek bar,
// transport, and a switcher when several players are open. (Material: the
// art cut into a cookie while it plays, a square when paused, and a play
// button that morphs the same way.)
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "nowPlaying"
        screen: modelData
        elsewhere: RustUi.drawing("statusmenus") // (tessera-ui draws it: StatusMenusFeed)

        Popover {
            id: pop
            readonly property var modelData: slot.modelData
            screen: modelData
            popoverName: "nowPlaying"
            lens: 0.35 // a menu right under the menu bar (see MenuPopover)
            autoClose: 5000
            contentWidth: 320
            targetX: Panels.anchorX - 120

            readonly property var player: Media.player
            property real position: player?.position ?? 0
            Timer {
                running: pop.shown && (pop.player?.isPlaying ?? false)
                interval: 1000
                repeat: true
                onTriggered: pop.position = pop.player?.position ?? 0
            }
            onShownChanged: position = player?.position ?? 0
            onPlayerChanged: position = player?.position ?? 0 // another player was picked

            function fmt(s) {
                s = Math.max(0, Math.floor(s));
                return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
            }

            Column {
                width: pop.contentWidth
                spacing: 12

                Row {
                    spacing: 12
                    ClippingRectangle {
                        visible: !Theme.material
                        width: 72
                        height: 72
                        radius: 12
                        color: Theme.color.fill
                        Image {
                            anchors.fill: parent
                            source: Theme.material ? "" : Media.artUrl(pop.player)
                            retainWhileLoading: true
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(144, 144)
                            asynchronous: true
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: Media.artUrl(pop.player) === ""
                            name: "music-notes"
                            fill: true
                            size: 28
                            color: Theme.color.tertiary
                        }
                    }
                    // Material: the art cut into a cookie while it plays, a square when paused
                    Loader {
                        active: Theme.material
                        visible: active
                        width: 72
                        height: 72
                        sourceComponent: Item {
                            ShapedArt {
                                anchors.fill: parent
                                url: Media.artUrl(pop.player)
                                sourceSize: Qt.size(144, 144)
                                shape: pop.player?.isPlaying ? "cookie9" : "square"
                                color: Theme.m3.surfaceContainerHighest
                            }
                            Icon {
                                anchors.centerIn: parent
                                visible: Media.artUrl(pop.player) === ""
                                name: "music-notes"
                                fill: true
                                size: 28
                                color: Theme.m3.onSurfaceVariant
                            }
                        }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        width: pop.contentWidth - 84
                        spacing: 2
                        Text {
                            width: parent.width
                            textFormat: Text.PlainText
                            text: pop.player?.trackTitle || "Nothing playing"
                            elide: Text.ElideRight
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                        }
                        Text {
                            width: parent.width
                            textFormat: Text.PlainText
                            text: [pop.player?.trackArtist, pop.player?.trackAlbum].filter(x => x).join(" — ")
                            elide: Text.ElideRight
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                        Text {
                            text: pop.player?.identity ?? ""
                            color: Theme.color.tertiary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                }

                // Seek bar
                Column {
                    width: parent.width
                    spacing: 4
                    visible: (pop.player?.lengthSupported ?? false) && (pop.player?.length ?? 0) > 0
                    Slider {
                        width: parent.width
                        from: 0
                        to: pop.player?.length ?? 1
                        value: pop.position
                        onMoved: v => {
                            if (pop.player?.canSeek) {
                                pop.player.position = v; // absolute, so a stale position can't skew it
                                pop.position = v;
                            }
                        }
                    }
                    Item {
                        width: parent.width
                        height: 14
                        Text { text: pop.fmt(pop.position); color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } } }
                        Text { anchors.right: parent.right; text: "-" + pop.fmt((pop.player?.length ?? 0) - pop.position); color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } } }
                    }
                }

                Row {
                    visible: !Theme.material
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 18
                    Repeater {
                        model: [
                            { icon: "skip-back", size: 22, act: () => pop.player?.previous(), on: pop.player?.canGoPrevious ?? false },
                            { icon: pop.player?.isPlaying ? "pause" : "play", size: 30, act: () => pop.player?.togglePlaying(), on: pop.player?.canTogglePlaying ?? false },
                            { icon: "skip-forward", size: 22, act: () => pop.player?.next(), on: pop.player?.canGoNext ?? false }
                        ]
                        Icon {
                            required property var modelData
                            anchors.verticalCenter: parent.verticalCenter
                            name: modelData.icon
                            fill: true
                            size: modelData.size
                            opacity: modelData.on ? 1 : 0.35
                            MouseArea { anchors { fill: parent; margins: -8 } enabled: parent.modelData.on; onClicked: parent.modelData.act() }
                        }
                    }
                }
                // Material: tonal skip buttons around the morphing play button
                Loader {
                    active: Theme.material
                    visible: active
                    anchors.horizontalCenter: parent.horizontalCenter
                    sourceComponent: Row {
                        spacing: 12
                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            width: 40
                            height: 40
                            icon: "skip-back"
                            fill: true
                            iconSize: 20
                            iconColor: Theme.m3.onSurfaceVariant
                            tooltip: "Previous"
                            enabled: pop.player?.canGoPrevious ?? false
                            opacity: enabled ? 1 : 0.38
                            onClicked: pop.player?.previous()
                        }
                        PlayButton {
                            size: 56
                            playing: pop.player?.isPlaying ?? false
                            enabled: pop.player?.canTogglePlaying ?? false
                            onClicked: pop.player?.togglePlaying()
                        }
                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            width: 40
                            height: 40
                            icon: "skip-forward"
                            fill: true
                            iconSize: 20
                            iconColor: Theme.m3.onSurfaceVariant
                            tooltip: "Next"
                            enabled: pop.player?.canGoNext ?? false
                            opacity: enabled ? 1 : 0.38
                            onClicked: pop.player?.next()
                        }
                    }
                }

                // Other players
                Column {
                    width: parent.width
                    visible: Media.players.length > 1
                    spacing: 2
                    Repeater {
                        model: Media.players.filter(p => p !== pop.player)
                        Squircle {
                            required property var modelData
                            width: parent.width
                            height: 30
                            radius: 8
                            color: other.containsMouse ? Theme.color.fill : "transparent"
                            Text {
                                x: 8
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - 16
                                elide: Text.ElideRight
                                textFormat: Text.PlainText
                                text: parent.modelData.identity + (parent.modelData.trackTitle ? " · " + parent.modelData.trackTitle : "")
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                            MouseArea { id: other; anchors.fill: parent; hoverEnabled: true; onClicked: Media.pinned = parent.modelData }
                        }
                    }
                }
            }
        }
    }
}
