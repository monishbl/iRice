import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// The drop shelf (services/Shelf): a glass card at the edge of a screen.
// Drop files or text on it; drag a row out to take that one, or "All" to
// take everything. They stay until removed or cleared.
LazyLoader {
    id: loader
    property bool leaving: false
    active: Shelf.opened || leaving
    Connections {
        target: Shelf
        function onOpenedChanged() {
            if (!Shelf.opened) {
                loader.leaving = true;
                leaveTimer.restart();
            }
        }
    }
    Timer { id: leaveTimer; interval: 140; onTriggered: loader.leaving = false }

    GlassWindow {
        id: win
        readonly property int m: 24 // (room for the shadow)
        readonly property int cardWidth: 240
        readonly property int rowH: 44

        name: "shelf"
        screen: Shelf.screen ?? Quickshell.screens[0]
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        exclusionMode: ExclusionMode.Ignore
        anchors { top: true; left: Shelf.side === "left"; right: Shelf.side !== "left" }
        margins.left: 8 - m
        margins.right: 8 - m
        margins.top: Shelf.atY < 0 ? 40 - m : Math.max(40 - m, Math.min(Shelf.atY - implicitHeight / 2, (screen?.height ?? 1080) - implicitHeight - 8))
        implicitWidth: cardWidth + 2 * m
        implicitHeight: card.height + 2 * m
        mask: Region { item: card }

        Glass {
            anchors.fill: parent
            radius: 20
            bevel: 16
            shadowMargin: win.m
            opacity: card.opacity
        }

        Item {
            id: card
            x: win.m
            y: win.m
            width: win.cardWidth
            height: body.implicitHeight + 24
            opacity: Shelf.opened ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Shelf.opened ? 200 : 130; easing.type: Shelf.opened ? Easing.OutCubic : Easing.InCubic } }
            transform: Translate { x: (Shelf.opened ? 0 : 6) * (Shelf.side === "left" ? -1 : 1); Behavior on x { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } } }

            HoverHandler { id: hover }
            Binding { target: Shelf; property: "hovered"; value: hover.hovered || drop.containsDrag }

            Column {
                id: body
                x: 12
                y: 12
                width: parent.width - 24
                spacing: 8

                // Title, and what can be done with everything
                Item {
                    width: parent.width
                    height: 28
                    Text {
                        id: title
                        anchors { left: parent.left; leftMargin: 2; verticalCenter: parent.verticalCenter }
                        text: "Shelf"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                    }
                    Text {
                        anchors { left: title.right; leftMargin: 6; verticalCenter: parent.verticalCenter }
                        visible: Shelf.count > 0
                        text: Shelf.count
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                    }
                    Row {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        spacing: 2
                        IconButton {
                            visible: Shelf.paths.length > 0
                            icon: "share-network"
                            tooltip: "Share to a nearby device"
                            onClicked: Panels.share(Shelf.paths, win.screen)
                        }
                        IconButton {
                            visible: Shelf.count > 0
                            icon: "trash"
                            tooltip: "Clear the shelf"
                            onClicked: Shelf.clear()
                        }
                        IconButton {
                            icon: "x"
                            tooltip: "Close"
                            onClicked: Shelf.hide()
                        }
                    }
                }

                // Empty: where to drop
                Rectangle {
                    visible: Shelf.count === 0
                    width: parent.width
                    height: 132
                    radius: 14
                    color: drop.containsDrag ? Qt.alpha(Theme.accent, 0.12) : "transparent"
                    border.width: 1
                    border.color: drop.containsDrag ? Theme.accent : Theme.color.separator
                    Column {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        spacing: 6
                        Icon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: "tray"
                            size: 24
                            color: drop.containsDrag ? Theme.accent : Theme.color.tertiary
                        }
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            text: "Drop files here"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                        }
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.Wrap
                            text: "Then drag them out wherever they're going."
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                }

                // Everything at once
                ShelfRow {
                    visible: Shelf.count > 1
                    width: parent.width
                    height: win.rowH
                    entries: Shelf.items
                    label: "All " + Shelf.count + " items"
                    detail: "Drag to take them together"
                    glyph: "stack"
                }

                ListView {
                    id: list
                    visible: Shelf.count > 0
                    width: parent.width
                    height: Math.min(Shelf.count, 6) * win.rowH
                    clip: true
                    interactive: contentHeight > height
                    boundsBehavior: Flickable.StopAtBounds
                    model: Shelf.items
                    delegate: ShelfRow {
                        required property var modelData
                        required property int index
                        width: list.width
                        height: win.rowH
                        entries: [modelData]
                        elide: modelData.kind === "text" ? Text.ElideRight : Text.ElideMiddle
                        label: modelData.name ?? ""
                        detail: modelData.kind === "text" ? "Text" : (modelData.path ?? "").replace(/\/[^/]*$/, "").replace(Quickshell.env("HOME"), "~")
                        glyph: modelData.kind === "text" ? "text-t" : modelData.kind === "folder" ? "folder" : /\.pdf$/i.test(modelData.path ?? "") ? "file-pdf" : /\.(zip|tar|gz|xz|zst|7z|rar)$/i.test(modelData.path ?? "") ? "file-zip" : /\.(mp3|flac|ogg|wav|m4a|opus)$/i.test(modelData.path ?? "") ? "file-audio" : /\.(mp4|mkv|webm|mov|avi)$/i.test(modelData.path ?? "") ? "file-video" : /\.(txt|md|csv|log|json)$/i.test(modelData.path ?? "") ? "file-text" : "file"
                        image: modelData.kind === "image" ? modelData.uri : ""
                        removable: true
                        onRemove: Shelf.removeAt(index)
                    }
                }
            }

            // Anything dropped anywhere on the card (but not its own rows
            // coming back)
            DropArea {
                id: drop
                anchors.fill: parent
                onEntered: d => {
                    if (d.source) {
                        d.accepted = false;
                        return;
                    }
                    d.accept(Qt.CopyAction);
                }
                onDropped: d => {
                    if (Shelf.drop(d.urls, d.text))
                        d.accept(Qt.CopyAction);
                }
            }
            // Its outline while something is over it with items already there
            Rectangle {
                anchors.fill: parent
                visible: drop.containsDrag && Shelf.count > 0
                radius: 20
                color: Qt.alpha(Theme.accent, 0.08)
                border.width: 2
                border.color: Theme.accent
            }
        }
    }
}
