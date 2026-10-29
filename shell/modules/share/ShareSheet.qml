import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Share (Panels.share(paths)): send files, or text (Sharing.shareText), to a
// nearby device, like AirDrop. Devices appear as they're found, in one list
// (Sharing.devices): computers and phones running LocalSend, and Android
// phones with Quick Share open; a device seen both ways is listed once.
// Click one to send. They're asked to accept on their side; the sheet shows
// how far each send has got (the island does too, once the sheet is closed).
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
            readonly property string note: Panels.shareNote // text being shared, if that's what it is
            readonly property string what: note !== "" ? "“" + note.replace(/\s+/g, " ") + "”" : paths.length === 1 ? "“" + paths[0].split("/").pop() + "”" : paths.length + " items"
            // Finding devices while the sheet is up (Sharing counts who's looking)
            property bool lookingHere: false
            function look(on) {
                if (on !== lookingHere) {
                    lookingHere = on;
                    Sharing.look(on);
                }
            }
            // (called as win.look: unqualified, a handler on this window type
            // didn't find the function and the call failed)
            onShownChanged: win.look(shown)
            Component.onDestruction: win.look(false)
            readonly property var devices: Sharing.devices

            property real progress: 0
            Component.onCompleted: {
                win.look(shown);
                progress = Qt.binding(() => shown ? 1 : 0);
            }
            Behavior on progress { NumberAnimation { duration: win.shown ? 220 : 150; easing.type: win.shown ? Easing.OutCubic : Easing.InCubic } }
            visible: progress > 0.01 || shown
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }

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
                            source: IconLookup.path(win.note !== "" ? "text-x-generic" : win.paths.length === 1 ? Search.iconForFile(win.paths[0] ?? "") : "folder-documents", "text-x-generic")
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
                                text: win.note !== "" ? "Sent as a text file" : "Nearby devices on the same Wi-Fi"
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                    }

                    // Nearby devices
                    Flow {
                        width: parent.width
                        spacing: 8
                        visible: win.devices.length > 0
                        Repeater {
                            model: win.devices
                            Rectangle {
                                id: peer
                                required property var modelData
                                // The send to it in flight: { done, total, waiting }
                                readonly property var sending: Sharing.progressFor(modelData)
                                width: 100
                                height: 112
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
                                            color: Theme.color.onAccent
                                        }
                                        // Sending: a ring fills around it
                                        Canvas {
                                            id: ring
                                            anchors.fill: parent
                                            anchors.margins: -4
                                            visible: peer.sending !== null
                                            readonly property real frac: peer.sending && peer.sending.total > 0 ? peer.sending.done / peer.sending.total : 0
                                            onFracChanged: requestPaint()
                                            onVisibleChanged: requestPaint()
                                            onPaint: {
                                                const ctx = getContext("2d");
                                                ctx.reset();
                                                ctx.lineWidth = 3;
                                                ctx.lineCap = "round";
                                                ctx.strokeStyle = Theme.color.fill;
                                                ctx.beginPath();
                                                ctx.arc(width / 2, height / 2, width / 2 - 2, 0, 2 * Math.PI);
                                                ctx.stroke();
                                                ctx.strokeStyle = Theme.accent;
                                                ctx.beginPath();
                                                ctx.arc(width / 2, height / 2, width / 2 - 2, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * frac);
                                                ctx.stroke();
                                            }
                                        }
                                    }
                                    Text {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        width: 92
                                        horizontalAlignment: Text.AlignHCenter
                                        text: peer.modelData.name
                                        elide: Text.ElideRight
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                                    }
                                    Text {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        text: peer.sending ? (peer.sending.waiting ? "Waiting…" : Math.round(100 * (peer.sending.total > 0 ? peer.sending.done / peer.sending.total : 0)) + "%")
                                            : peer.modelData.prefer === "quickshare" ? "Quick Share" : "LocalSend"
                                        color: Theme.color.secondary
                                        font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                                    }
                                }
                                MouseArea {
                                    id: peerMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    // (The sheet stays: the ring shows the send; the
                                    // island takes over once it's closed)
                                    onClicked: if (!peer.sending) Sharing.send(peer.modelData, win.paths)
                                }
                            }
                        }
                    }

                    // Looking, or nobody there
                    Column {
                        visible: win.devices.length === 0
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
                                Ticker { id: look; running: win.shown && win.devices.length === 0; fps: 15 }
                            }
                            Text {
                                text: LocalSend.available ? "No devices nearby" : "Sharing needs Python 3"
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                            }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "Make sure they're on the same Wi-Fi with LocalSend open, or Quick Share visible to everyone. Still nothing? Your firewall may be blocking them: allow ports 53317 and 53318 and mDNS (port 5353)."
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }

                    Text {
                        readonly property string errors: [LocalSend.error, QuickShare.error].filter(e => !!e).join(" ")
                        visible: errors !== ""
                        width: parent.width
                        wrapMode: Text.Wrap
                        text: errors
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }

                    // Other ways
                    Row {
                        spacing: 8
                        Repeater {
                            model: win.note !== "" ? [
                                { label: "Copy", icon: "copy", act: () => Quickshell.execDetached(["wl-copy", "--", win.note]) }
                            ] : [
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
