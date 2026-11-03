import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.glass
import qs.services

// Share (Panels.share(paths)): send files, or text (Sharing.shareText), to a
// nearby device, laid out like iOS's share sheet in glass: what's shared at
// the top, nearby devices as round avatars in a row (AirDrop's), then the
// other ways as a grouped list. Devices appear as they're found, in one list
// (Sharing.devices): computers and phones running LocalSend, and Android
// phones with Quick Share open; a device seen both ways is listed once.
// Click one to send. They're asked to accept on their side; the sheet shows
// how far each send has got (the island does too, once the sheet is closed).
// Material: a dialog in surfaceContainerHigh (corner 28), devices on
// Material shapes, a wavy bar for a send, the loading indicator while it
// looks, and the other ways as a segmented list.
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
            // Nearby devices, and Chromecasts for one video, song or photo
            readonly property bool castable: win.paths.length === 1 && win.note === "" && /\.(mp4|m4v|webm|mkv|mov|mp3|m4a|aac|ogg|oga|opus|flac|wav|jpe?g|png|gif|webp)$/i.test(win.paths[0])
            readonly property var devices: Sharing.devices.concat(castable ? Cast.devices.map(d => Object.assign({}, d, { cast: true })) : [])
            function sendTo(dev) {
                if (dev.cast) {
                    Cast.castFile(dev, win.paths[0]);
                    Panels.close();
                } else {
                    Sharing.send(dev, win.paths);
                }
            }

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
                width: Theme.material ? 432 : 420
                height: column.implicitHeight + 2 * column.y
                opacity: win.progress
                scale: 0.94 + 0.06 * win.progress
                focus: win.shown
                Keys.onEscapePressed: Panels.close()

                Glass {
                    anchors.fill: parent
                    anchors.margins: -shadowMargin
                    radius: Theme.radius.panel
                }
                // (Material: a dialog's surfaceContainerHigh)
                Rectangle {
                    visible: Theme.material
                    anchors.fill: parent
                    radius: 28
                    color: Theme.material ? Theme.m3.surfaceContainerHigh : "transparent"
                }
                MouseArea { anchors.fill: parent }

                Column {
                    id: column
                    x: Theme.material ? 24 : 16
                    y: Theme.material ? 24 : 16
                    width: parent.width - 2 * x
                    spacing: Theme.material ? 16 : 14

                    // ── What's being shared (iOS: a preview, its name, ×) ──
                    Item {
                        width: parent.width
                        height: 52
                        ClippingRectangle {
                            id: thumb
                            width: 52
                            height: 52
                            radius: Theme.material ? 16 : 12
                            color: Theme.color.fill
                            readonly property bool image: win.note === "" && win.paths.length >= 1 && /\.(jpe?g|png|webp|gif|bmp|avif)$/i.test(win.paths[0] ?? "")
                            Image {
                                anchors.fill: parent
                                anchors.margins: 1
                                visible: thumb.image
                                source: thumb.image ? "file://" + win.paths[0] : ""
                                sourceSize: Qt.size(104, 104)
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                            }
                            AppIcon {
                                anchors.centerIn: parent
                                visible: !thumb.image
                                size: 36
                                styled: false
                                source: IconLookup.path(win.note !== "" ? "text-x-generic" : win.paths.length === 1 ? Search.iconForFile(win.paths[0] ?? "") : "folder-documents", "text-x-generic")
                            }
                        }
                        Column {
                            anchors { left: thumb.right; leftMargin: 12; right: close.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            spacing: 2
                            Text {
                                width: parent.width
                                text: win.note !== "" ? win.what : win.paths.length === 1 ? (win.paths[0] ?? "").split("/").pop() : win.paths.length + " items"
                                elide: Text.ElideMiddle
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                            }
                            Text {
                                width: parent.width
                                text: win.note !== "" ? "Text · sent as a text file" : win.paths.length > 1 ? "Files" : thumb.image ? "Photo" : /\.(mp4|m4v|webm|mkv|mov)$/i.test(win.paths[0] ?? "") ? "Video" : "Document"
                                elide: Text.ElideRight
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                        // ×, round like iOS's
                        Rectangle {
                            id: close
                            anchors { right: parent.right; top: parent.top }
                            width: Theme.material ? 40 : 30
                            height: width
                            transform: Theme.material ? pressFx1.effects : []
                            ElasticControl { id: pressFx1; pressed: closeMouse.pressed; enabled: Theme.material }
                            radius: Theme.material && closeMouse.pressed ? 12 : width / 2
                            color: closeMouse.pressed ? Theme.color.fillPressed : closeMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                            activeFocusOnTab: true
                            Keys.onPressed: event => {
                                if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                                    Panels.close();
                                    event.accepted = true;
                                }
                            }
                            Accessible.role: Accessible.Button
                            Accessible.name: "Close"
                            FocusRing { radius: close.radius + 4 }
                            Icon { anchors.centerIn: parent; name: "x"; size: Theme.material ? 18 : 14; color: Theme.color.secondary }
                            MouseArea { id: closeMouse; anchors.fill: parent; hoverEnabled: true; onClicked: Panels.close() }
                        }
                    }

                    Rectangle { visible: !Theme.material; width: parent.width; height: 1; color: Theme.color.separator }

                    // ── Nearby (iOS's AirDrop row: round avatars, names under) ──
                    Flickable {
                        id: people
                        Momentum {}
                        width: parent.width
                        height: Theme.material ? 128 : 118
                        visible: win.devices.length > 0
                        contentWidth: peopleRow.width
                        clip: true
                        flickableDirection: Flickable.HorizontalFlick
                        boundsBehavior: Flickable.StopAtBounds
                        Row {
                            id: peopleRow
                            spacing: 4
                            Repeater {
                                model: win.devices
                                Item {
                                    id: peer
                                    required property var modelData
                                    // The send to it in flight: { done, total, waiting }; then how it ended
                                    readonly property var sending: Sharing.progressFor(modelData)
                                    readonly property string ended: sending ? "" : Sharing.outcomeFor(modelData)
                                    readonly property real frac: sending && sending.total > 0 ? sending.done / sending.total : 0
                                    readonly property string deviceIcon: modelData.cast ? Cast.iconFor(modelData) : modelData.type === "mobile" ? "device-mobile" : modelData.type === "tablet" ? "device-tablet" : modelData.type === "laptop" ? "laptop" : "desktop"
                                    width: 92
                                    height: people.height
                                    // Keyboard: Tab or the arrows to a device, Space or Enter sends
                                    activeFocusOnTab: true
                                    Keys.onPressed: event => {
                                        const k = event.key;
                                        if ((k === Qt.Key_Space || k === Qt.Key_Return || k === Qt.Key_Enter) && !event.isAutoRepeat) {
                                            if (!peer.sending)
                                                win.sendTo(peer.modelData);
                                        } else if (k === Qt.Key_Left || k === Qt.Key_Right) {
                                            nextItemInFocusChain(k === Qt.Key_Right)?.forceActiveFocus(k === Qt.Key_Right ? Qt.TabFocusReason : Qt.BacktabFocusReason);
                                        } else {
                                            return;
                                        }
                                        event.accepted = true;
                                    }
                                    onActiveFocusChanged: if (activeFocus) people.contentX = Math.max(0, Math.min(x + width / 2 - people.width / 2, people.contentWidth - people.width))
                                    Accessible.role: Accessible.Button
                                    Accessible.name: (modelData.cast ? "Cast to " : "Send to ") + (modelData.name ?? "")
                                    Accessible.description: statusText.text
                                    Accessible.onPressAction: if (!peer.sending) win.sendTo(peer.modelData)

                                    Item {
                                        id: avatar
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        y: 4
                                        width: 68
                                        height: 68
                                        // (Glass shrinks on press; Material grows: the ElasticControl)
                                        scale: !Theme.material && peerMouse.pressed ? 0.94 : 1
                                        transform: Theme.material ? pressFx2.effects : []
                                        ElasticControl { id: pressFx2; pressed: peerMouse.pressed; enabled: Theme.material }
                                        Behavior on scale { NumberAnimation { duration: 100 } }
                                        FocusRing { radius: 38; visible: peer.activeFocus }
                                        // Material: the device on a shape, its kind
                                        // telling which (phones a cookie, tablets a
                                        // clover, computers a soft square, Chromecasts
                                        // a sunny); a hover morphs it, as Android does
                                        Loader {
                                            anchors.centerIn: parent
                                            active: Theme.material
                                            sourceComponent: MaterialShape {
                                                width: 64
                                                height: 64
                                                readonly property string kind: peer.modelData.cast ? "sunny" : peer.modelData.type === "mobile" ? "cookie9" : peer.modelData.type === "tablet" ? "clover4" : "cookie4"
                                                shape: peerMouse.containsMouse && !peer.sending ? "cookie12" : peer.sending ? "circle" : kind
                                                color: peer.modelData.cast ? Theme.m3.tertiaryContainer : Theme.m3.primaryContainer
                                                Icon {
                                                    anchors.centerIn: parent
                                                    name: peer.deviceIcon
                                                    fill: true
                                                    size: 28
                                                    color: peer.modelData.cast ? Theme.m3.onTertiaryContainer : Theme.m3.onPrimaryContainer
                                                }
                                            }
                                        }
                                        // Glass: a round avatar, and a ring that fills around
                                        // them while sending
                                        Loader {
                                            anchors.centerIn: parent
                                            active: !Theme.material
                                            sourceComponent: Rectangle {
                                                width: 60
                                                height: 60
                                                radius: 30
                                                gradient: Gradient {
                                                    GradientStop { position: 0; color: Qt.lighter(Theme.accent, 1.25) }
                                                    GradientStop { position: 1; color: Theme.accent }
                                                }
                                                opacity: peerMouse.containsMouse && !peer.sending ? 0.88 : 1
                                                Icon {
                                                    anchors.centerIn: parent
                                                    name: peer.deviceIcon
                                                    fill: true
                                                    size: 28
                                                    color: Theme.color.onAccent
                                                }
                                            }
                                        }
                                        Loader {
                                            anchors.fill: parent
                                            active: !Theme.material
                                            sourceComponent: ProgressRing {
                                                visible: peer.sending !== null
                                                value: peer.frac
                                                thickness: 3
                                                inset: 2
                                            }
                                        }
                                        // Sent: a check; declined or stopped: a mark
                                        Rectangle {
                                            visible: peer.ended !== ""
                                            anchors { right: parent.right; bottom: parent.bottom; rightMargin: 2; bottomMargin: 2 }
                                            width: 22
                                            height: 22
                                            radius: 11
                                            // (Material: sent in primary, else a neutral container)
                                            color: Theme.material ? (peer.ended === "sent" ? Theme.m3.primary : Theme.m3.surfaceContainerHighest) : peer.ended === "sent" ? Theme.color.success : Theme.color.secondary
                                            border.width: 2
                                            border.color: Theme.material ? Theme.m3.surfaceContainerHigh : Theme.glass.solid
                                            Icon {
                                                anchors.centerIn: parent
                                                name: peer.ended === "sent" ? "check" : "x"
                                                size: 12
                                                color: Theme.material ? (peer.ended === "sent" ? Theme.m3.onPrimary : Theme.m3.onSurfaceVariant) : "white"
                                            }
                                        }
                                    }
                                    // Material: how far the send has got, a wavy bar
                                    Loader {
                                        id: sendBar
                                        anchors { top: avatar.bottom; horizontalCenter: parent.horizontalCenter }
                                        active: Theme.material && peer.sending !== null
                                        visible: active
                                        sourceComponent: WavyProgress {
                                            width: 64
                                            value: peer.sending?.waiting ? -1 : peer.frac
                                        }
                                    }
                                    Text {
                                        id: nameText
                                        anchors { top: sendBar.visible ? sendBar.bottom : avatar.bottom; topMargin: 4; horizontalCenter: parent.horizontalCenter }
                                        width: 88
                                        horizontalAlignment: Text.AlignHCenter
                                        textFormat: Text.PlainText
                                        text: peer.modelData.name
                                        wrapMode: Text.Wrap
                                        maximumLineCount: 2
                                        elide: Text.ElideRight
                                        lineHeight: 0.95
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                                    }
                                    Text {
                                        id: statusText
                                        anchors { top: nameText.bottom; topMargin: 1; horizontalCenter: parent.horizontalCenter }
                                        text: peer.sending ? (peer.sending.waiting ? "Waiting…" : "Sending… " + Math.round(100 * peer.frac) + "%")
                                            : peer.ended === "sent" ? "Sent" : peer.ended === "declined" ? "Declined" : peer.ended === "failed" ? "Didn't send"
                                            : peer.modelData.cast ? "Cast" : peer.modelData.prefer === "quickshare" ? "Quick Share" : "LocalSend"
                                        color: peer.ended === "sent" ? (Theme.material ? Theme.m3.primary : Theme.color.success) : Theme.color.secondary
                                        font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                                    }
                                    MouseArea {
                                        id: peerMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        // (The sheet stays: the ring shows the send; the
                                        // island takes over once it's closed)
                                        onClicked: if (!peer.sending) win.sendTo(peer.modelData)
                                    }
                                }
                            }
                        }
                    }

                    // Looking, or nobody there (iOS: faint circles while it looks)
                    Column {
                        visible: win.devices.length === 0
                        width: parent.width
                        spacing: 8
                        Ticker { id: look; running: win.shown && win.devices.length === 0 && !Theme.material; fps: 15 }
                        Loader {
                            anchors.horizontalCenter: parent.horizontalCenter
                            active: !Theme.material
                            visible: active
                            sourceComponent: Row {
                                spacing: 24
                                Repeater {
                                    model: 3
                                    Rectangle {
                                        required property int index
                                        width: 52
                                        height: 52
                                        radius: 26
                                        color: Theme.color.fill
                                        opacity: LocalSend.available ? 0.45 + 0.55 * (0.5 + 0.5 * Math.cos(look.time * Math.PI - index * 0.9)) : 0.45
                                    }
                                }
                            }
                        }
                        // Material: the loading indicator while it looks, or
                        // a quiet shape when it can't
                        Loader {
                            anchors.horizontalCenter: parent.horizontalCenter
                            active: Theme.material
                            visible: active
                            sourceComponent: LocalSend.available ? lookingSpinner : notLooking
                            Component {
                                id: lookingSpinner
                                Spinner { size: 48 }
                            }
                            Component {
                                id: notLooking
                                MaterialShape {
                                    width: 56
                                    height: 56
                                    shape: "cookie9"
                                    color: Theme.m3.secondaryContainer
                                    Icon { anchors.centerIn: parent; name: "share-network"; size: 24; color: Theme.m3.onSecondaryContainer }
                                }
                            }
                        }
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            text: LocalSend.available ? "Looking for nearby devices…" : "Sharing needs Python 3"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                        }
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.Wrap
                            text: "Phones show up with Quick Share open and visible to everyone; computers with LocalSend open." + (SharingPorts.error !== "" ? " " + SharingPorts.error : "")
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

                    // ── Other ways (iOS: a grouped list, the icon on the right) ──
                    // (Material: a segmented list, rows 2 apart, the group's
                    // outer corners 16, inner 4)
                    Squircle {
                        width: parent.width
                        height: actions.implicitHeight
                        radius: 14
                        color: Theme.material ? "transparent" : Theme.color.platter
                        Column {
                            id: actions
                            width: parent.width
                            spacing: Theme.material ? 2 : 0
                            Repeater {
                                id: otherWays
                                model: win.note !== "" ? [
                                    { label: "Copy", icon: "copy", act: () => Clipboard.copyText(win.note) }
                                ] : [
                                    { label: "Copy", icon: "copy", act: () => Quickshell.execDetached(["sh", "-c", "printf '%s\\n' \"$@\" | sed 's|^|file://|' | wl-copy -t text/uri-list", "sh"].concat(win.paths)) },
                                    { label: "Show in Files", icon: "folder-open", act: () => Files.reveal(win.paths[0] ?? "") },
                                    // (an iPhone, or any phone's browser: a QR code on this network)
                                    { label: "Share with a QR code", icon: "qr-code", act: () => LinkShare.send(win.paths) }
                                ]
                                Item {
                                    id: other
                                    required property var modelData
                                    required property int index
                                    width: actions.width
                                    height: Theme.material ? 52 : 44
                                    activeFocusOnTab: true
                                    Keys.onPressed: event => {
                                        if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                                            other.modelData.act();
                                            Panels.close();
                                            event.accepted = true;
                                        }
                                    }
                                    Accessible.role: Accessible.Button
                                    Accessible.name: modelData.label
                                    Rectangle {
                                        visible: Theme.material
                                        anchors.fill: parent
                                        readonly property bool first: other.index === 0
                                        readonly property bool last: other.index === otherWays.count - 1
                                        topLeftRadius: first ? 16 : 4
                                        topRightRadius: first ? 16 : 4
                                        bottomLeftRadius: last ? 16 : 4
                                        bottomRightRadius: last ? 16 : 4
                                        color: Theme.material ? Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, otherMouse.pressed ? 0.1 : otherMouse.containsMouse ? 0.08 : 0)) : "transparent"
                                    }
                                    Squircle {
                                        visible: !Theme.material
                                        anchors.fill: parent
                                        anchors.margins: 2
                                        radius: 12
                                        color: otherMouse.pressed ? Theme.color.fillPressed : otherMouse.containsMouse ? Theme.color.fillHover : "transparent"
                                    }
                                    FocusRing { radius: 14 }
                                    Text {
                                        anchors { left: parent.left; leftMargin: 14; verticalCenter: parent.verticalCenter }
                                        text: other.modelData.label
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                                    }
                                    Icon {
                                        anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                                        name: other.modelData.icon
                                        size: 18
                                        color: Theme.color.label
                                    }
                                    // (Glass: a hairline between rows, inset like iOS's)
                                    Rectangle {
                                        visible: !Theme.material && other.index > 0
                                        anchors { top: parent.top; left: parent.left; leftMargin: 14; right: parent.right }
                                        height: 1
                                        color: Theme.color.separator
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
}
