import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Share (Panels.share(paths)): send files to a nearby device, like AirDrop.
// Devices running LocalSend on the same network appear as they're found;
// click one to send. They're asked to accept on their side; progress shows
// in the island.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "share"
        screen: modelData

        GlassWindow {
            id: win
            screen: slot.modelData
            name: "share"
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            readonly property bool shown: Panels.isOpen("share", screen)
            readonly property var paths: Panels.sharePaths
            readonly property string what: paths.length === 1 ? "“" + paths[0].split("/").pop() + "”" : paths.length + " items"
            onShownChanged: LocalSend.sharing = shown
            Component.onDestruction: LocalSend.sharing = false

            property real progress: 0
            Component.onCompleted: {
                LocalSend.sharing = shown;
                progress = Qt.binding(() => shown ? 1 : 0);
            }
            Behavior on progress { NumberAnimation { duration: win.shown ? 220 : 150; easing.type: win.shown ? Easing.OutCubic : Easing.InCubic } }
            visible: progress > 0.01 || shown

            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, 0.25 * win.progress)
                MouseArea { anchors.fill: parent; onClicked: Panels.close() }
            }

            Item {
                id: card
                anchors.centerIn: parent
                width: 460
                height: column.implicitHeight + 40
                opacity: win.progress
                scale: 0.95 + 0.05 * win.progress
                focus: win.shown
                Keys.onEscapePressed: Panels.close()

                Glass {
                    anchors.fill: parent
                    anchors.margins: -shadowMargin
                    radius: 20
                }
                MouseArea { anchors.fill: parent }

                Column {
                    id: column
                    x: 20
                    y: 20
                    width: parent.width - 40
                    spacing: 16

                    // What's being shared
                    Row {
                        spacing: 12
                        AppIcon {
                            size: 44
                            styled: false
                            source: IconLookup.path(win.paths.length === 1 ? Search.iconForFile(win.paths[0] ?? "") : "folder-documents", "text-x-generic")
                        }
                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            Text {
                                width: column.width - 56
                                text: "Share " + win.what
                                elide: Text.ElideMiddle
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                            }
                            Text {
                                text: "With a nearby device"
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                    }

                    // Nearby devices
                    Flow {
                        width: parent.width
                        spacing: 8
                        visible: LocalSend.peers.length > 0
                        Repeater {
                            model: LocalSend.peers
                            Rectangle {
                                id: peer
                                required property var modelData
                                width: 100
                                height: 100
                                radius: 14
                                color: peerMouse.pressed ? Theme.color.fillPressed : peerMouse.containsMouse ? Theme.color.fill : "transparent"
                                Column {
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Rectangle {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        width: 52
                                        height: 52
                                        radius: 26
                                        color: Theme.accent
                                        Icon {
                                            anchors.centerIn: parent
                                            name: peer.modelData.type === "mobile" ? "device-mobile" : peer.modelData.type === "tablet" ? "device-tablet" : peer.modelData.type === "laptop" ? "laptop" : "desktop"
                                            fill: true
                                            size: 26
                                            color: "white"
                                        }
                                    }
                                    Text {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        width: 92
                                        horizontalAlignment: Text.AlignHCenter
                                        text: peer.modelData.alias
                                        elide: Text.ElideRight
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                                    }
                                }
                                MouseArea {
                                    id: peerMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        LocalSend.send(peer.modelData.id, win.paths);
                                        Panels.close();
                                    }
                                }
                            }
                        }
                    }

                    // Looking, or nobody there
                    Column {
                        visible: LocalSend.peers.length === 0
                        width: parent.width
                        spacing: 6
                        Row {
                            spacing: 8
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: 8
                                height: 8
                                radius: 4
                                color: Theme.accent
                                visible: LocalSend.available
                                opacity: 0.4 + 0.6 * (0.5 + 0.5 * Math.cos(look.time * Math.PI))
                                Ticker { id: look; running: win.shown && LocalSend.peers.length === 0; fps: 15 }
                            }
                            Text {
                                text: LocalSend.available ? "Looking for devices…" : "Sharing needs Python 3"
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                            }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "Open LocalSend on your phone or other computer, on the same Wi-Fi. If nothing shows up, your firewall may be blocking port 53317."
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }

                    // Other ways
                    Row {
                        spacing: 8
                        Repeater {
                            model: [
                                { label: "Copy", icon: "copy", act: () => Quickshell.execDetached(["sh", "-c", "printf '%s\\n' \"$@\" | sed 's|^|file://|' | wl-copy -t text/uri-list", "sh"].concat(win.paths)) },
                                { label: "Show in Files", icon: "folder-open", act: () => Files.reveal(win.paths[0] ?? "") }
                            ]
                            Rectangle {
                                id: other
                                required property var modelData
                                width: otherRow.implicitWidth + 24
                                height: 32
                                radius: Theme.radius.control
                                color: otherMouse.pressed ? Theme.color.fillPressed : otherMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                                Row {
                                    id: otherRow
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Icon { anchors.verticalCenter: parent.verticalCenter; name: other.modelData.icon; size: 14; color: Theme.color.secondary }
                                    Text { anchors.verticalCenter: parent.verticalCenter; text: other.modelData.label; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                                }
                                MouseArea {
                                    id: otherMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: {
                                        other.modelData.act();
                                        Panels.close();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
