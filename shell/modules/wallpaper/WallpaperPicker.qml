import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.services
import qs.glass

// Wallpaper picker: a glass strip above the dock. Arrow keys move, Enter
// applies, Esc closes. Lists the wallpaper folder plus the bundled wallpapers.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "wallpaper"
        screen: modelData

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "wallpaper"

            readonly property bool shown: Panels.isOpen("wallpaper", screen)
            readonly property int m: 24
            readonly property int thumbW: 220
            readonly property int thumbH: Math.round(thumbW * 9 / 16)
            readonly property var files: {
                const out = [];
                for (let i = 0; i < userFolder.count; i++)
                    out.push(userFolder.get(i, "filePath"));
                for (let i = 0; i < bundled.count; i++)
                    out.push(bundled.get(i, "filePath"));
                return out;
            }

            // Starts at 0 so the window animates in when it is created
            property real progress: 0
            Component.onCompleted: {
                progress = Qt.binding(() => shown ? 1 : 0);
                Qt.callLater(() => list.currentIndex = Math.max(0, files.indexOf(Wallpaper.path)));
            }
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.panel; easing.type: Easing.BezierSpline; easing.bezierCurve: win.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            anchors.bottom: true
            margins.bottom: Config.options.dock.iconSize + 14 + 6 + 12 - m
            implicitWidth: Math.min((screen?.width ?? 1920) - 80, 1200) + 2 * m
            implicitHeight: thumbH + 78 + 2 * m
            onShownChanged: if (shown) list.currentIndex = Math.max(0, files.indexOf(Wallpaper.path))

            FolderListModel {
                id: userFolder
                folder: Wallpaper.fileUrl(Wallpaper.directory)
                nameFilters: ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.JPG", "*.PNG"]
                showDirs: false
                sortField: FolderListModel.Time
            }
            FolderListModel {
                id: bundled
                folder: Wallpaper.fileUrl(Wallpaper.bundledDir)
                nameFilters: ["*.jpg"]
                showDirs: false
            }

            Item {
                id: body
                anchors.fill: parent
                opacity: win.progress
                y: (1 - win.progress) * 16
                onYChanged: glass.updatePosition()

                Glass {
                    id: glass
                    anchors.fill: parent
                    radius: 26
                    bevel: 20
                    shadowMargin: win.m
                }

                Item {
                    anchors { fill: parent; margins: win.m + 14 }
                    focus: win.shown
                    Keys.onEscapePressed: Panels.close()
                    Keys.onLeftPressed: list.decrementCurrentIndex()
                    Keys.onRightPressed: list.incrementCurrentIndex()
                    Keys.onReturnPressed: Wallpaper.set(win.files[list.currentIndex])

                    Text {
                        id: title
                        text: "Wallpaper"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                    }
                    Text {
                        anchors { left: title.right; leftMargin: 10; baseline: title.baseline }
                        text: Wallpaper.directory.replace(Quickshell.env("HOME"), "~")
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    Rectangle {
                        anchors { right: parent.right; verticalCenter: title.verticalCenter }
                        width: openLabel.implicitWidth + 24
                        height: 26
                        radius: 13
                        color: openMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                        Text {
                            id: openLabel
                            anchors.centerIn: parent
                            text: "Open folder"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                        }
                        MouseArea {
                            id: openMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                Panels.close();
                                Files.openFolder(Wallpaper.directory);
                            }
                        }
                    }

                    ListView {
                        id: list
                        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                        height: win.thumbH + 8
                        orientation: ListView.Horizontal
                        spacing: 12
                        clip: true
                        model: win.files
                        highlightMoveDuration: Theme.motion.popover
                        preferredHighlightBegin: width / 2 - win.thumbW / 2
                        preferredHighlightEnd: width / 2 + win.thumbW / 2
                        highlightRangeMode: ListView.ApplyRange

                        delegate: Item {
                            id: cell
                            required property string modelData
                            required property int index
                            readonly property bool current: modelData === Wallpaper.path
                            width: win.thumbW
                            height: win.thumbH + 8

                            ClippingRectangle {
                                id: thumb
                                y: 4
                                width: parent.width
                                height: win.thumbH
                                radius: 12
                                color: Theme.color.fill
                                border.width: cell.current ? 3 : list.currentIndex === cell.index ? 2 : 0
                                border.color: cell.current ? Theme.accent : Theme.color.label
                                scale: thumbMouse.containsMouse ? 1.03 : 1
                                Behavior on scale { ScaleAnimator { duration: Theme.motion.hover } }

                                Image {
                                    anchors.fill: parent
                                    anchors.margins: thumb.border.width
                                    source: Wallpaper.fileUrl(cell.modelData)
                                    fillMode: Image.PreserveAspectCrop
                                    sourceSize: Qt.size(win.thumbW * 2, win.thumbH * 2)
                                    asynchronous: true
                                    opacity: status === Image.Ready ? 1 : 0
                                    Behavior on opacity { NumberAnimation { duration: Theme.motion.popover } }
                                }
                                MouseArea {
                                    id: thumbMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        list.currentIndex = cell.index;
                                        Wallpaper.set(cell.modelData);
                                    }
                                }
                            }
                        }

                        Text {
                            anchors.centerIn: parent
                            visible: win.files.length === 0
                            text: "No images in " + Wallpaper.directory.replace(Quickshell.env("HOME"), "~")
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                    }
                }
            }
        }
    }
}
