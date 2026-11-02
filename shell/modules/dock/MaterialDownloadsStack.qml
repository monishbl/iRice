import QtQuick
import QtQuick.Effects
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// Material Downloads stack: the newest files in ~/Downloads in a Material
// popover above the dock (surfaceContainer, corner 28, elevation 2's
// shadow), a title with "Open in Files" beside it, files in a grid with
// state layers. Same actions as the Glass stack: click to open,
// right-click to share, the eye for Quick Look, Escape or a click outside
// to close.
PanelWindow {
    id: root

    required property var dock
    readonly property string dir: Quickshell.env("HOME") + "/Downloads"
    readonly property int cols: 4
    readonly property int cell: 96
    readonly property int pad: 16

    screen: dock.screen
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "irice:stack"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    anchors { top: true; bottom: true; left: true; right: true }

    // Grows up out of the dock on the default spatial spring
    property real progress: 0
    Component.onCompleted: {
        progress = 1;
        body.forceActiveFocus();
    }
    Behavior on progress {
        SpringAnimation {
            spring: Theme.motion.springDefault.spring
            damping: Theme.motion.springDefault.damping
            epsilon: 0.005
        }
    }

    FolderListModel {
        id: files
        folder: "file://" + root.dir
        sortField: FolderListModel.Time
        showDirs: false
        showHidden: false
    }
    readonly property int shown: Math.min(12, files.count)

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onClicked: root.dock.stackOpen = false
    }

    Item {
        id: body
        width: root.cols * root.cell + 2 * root.pad
        height: 56 + (root.shown > 0 ? Math.ceil(root.shown / root.cols) * (root.cell + 4) : 160) + root.pad
        x: Math.max(8, Math.min(root.width - width - 8, root.dock.stackX - width / 2))
        y: root.height - root.dock.barHeight - root.dock.bottomGap - 8 - height
        opacity: Math.max(0, Math.min(1, root.progress * 1.5))
        scale: 0.9 + 0.1 * root.progress
        transformOrigin: Item.Bottom
        focus: true
        Keys.onEscapePressed: root.dock.stackOpen = false

        RectangularShadow {
            anchors.fill: parent
            radius: Theme.radius.popover
            offset.y: 2
            blur: 12
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.4 : 0.16)
        }
        Rectangle {
            anchors.fill: parent
            radius: Theme.radius.popover
            color: Theme.m3.surfaceContainer
        }
        MouseArea { anchors.fill: parent } // (clicks on the card don't close it)

        Text {
            x: root.pad + 8
            y: 16
            height: 24
            verticalAlignment: Text.AlignVCenter
            text: "Downloads"
            color: Theme.m3.onSurface
            font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium; letterSpacing: 0.15 }
        }
        // Open the folder: a text button
        Rectangle {
            id: openButton
            anchors { right: parent.right; rightMargin: root.pad; top: parent.top; topMargin: 12 }
            width: openLabel.implicitWidth + 24
            height: 32
            radius: height / 2
            color: Qt.alpha(Theme.m3.primary, openMouse.pressed ? 0.1 : openMouse.containsMouse ? 0.08 : 0)
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            Accessible.role: Accessible.Button
            Accessible.name: openLabel.text
            Text {
                id: openLabel
                anchors.centerIn: parent
                text: "Open in Files"
                color: Theme.m3.primary
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; letterSpacing: 0.1 }
            }
            MouseArea {
                id: openMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    root.dock.stackOpen = false;
                    Files.openFolder(root.dir);
                }
            }
        }

        Grid {
            x: root.pad
            y: 56
            columns: root.cols
            rowSpacing: 4
            Repeater {
                model: root.shown
                Item {
                    id: fileCell
                    required property int index
                    readonly property string path: files.get(index, "filePath") ?? ""
                    width: root.cell
                    height: root.cell
                    Accessible.role: Accessible.Button
                    Accessible.name: path.split("/").pop()
                    // State layer
                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 2
                        radius: 16 // (Material's list items, hovered or selected)
                        color: Qt.alpha(Theme.m3.onSurface, fileMouse.pressed ? 0.1 : fileMouse.containsMouse || quickLook.containsMouse ? 0.08 : 0)
                        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                    }
                    AppIcon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        y: 10
                        size: 48
                        styled: false
                        source: IconLookup.path(Search.iconForFile(fileCell.path), "text-x-generic")
                    }
                    Text {
                        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 10 }
                        width: parent.width - 12
                        horizontalAlignment: Text.AlignHCenter
                        text: fileCell.path.split("/").pop()
                        elide: Text.ElideMiddle
                        color: Theme.m3.onSurface
                        font { family: Theme.font.family; pixelSize: 12; letterSpacing: 0.4 }
                    }
                    MouseArea {
                        id: fileMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        cursorShape: Qt.PointingHandCursor
                        // Right-click: share it with a nearby device
                        onClicked: m => {
                            root.dock.stackOpen = false;
                            if (m.button === Qt.RightButton)
                                Panels.share([fileCell.path]);
                            else
                                Files.open(fileCell.path);
                        }
                    }
                    // Quick Look without opening the app: a small tonal icon button
                    Rectangle {
                        visible: fileMouse.containsMouse || quickLook.containsMouse
                        anchors { right: parent.right; top: parent.top; margins: 6 }
                        width: 28
                        height: 28
                        radius: quickLook.pressed ? 8 : 14
                        Behavior on radius { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
                        color: Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, quickLook.pressed ? 0.1 : quickLook.containsMouse ? 0.08 : 0))
                        Accessible.role: Accessible.Button
                        Accessible.name: "Quick Look"
                        Icon {
                            anchors.centerIn: parent
                            name: "eye"
                            size: 14
                            color: Theme.m3.onSecondaryContainer
                        }
                        MouseArea {
                            id: quickLook
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Panels.quickLook(fileCell.path)
                        }
                    }
                }
            }
        }

        // Nothing yet: the download symbol on a shape, and one line
        Column {
            visible: root.shown === 0
            anchors.horizontalCenter: parent.horizontalCenter
            y: 56 + 16
            spacing: 12
            MaterialShape {
                anchors.horizontalCenter: parent.horizontalCenter
                width: 72
                height: 72
                shape: "cookie9"
                color: Theme.m3.secondaryContainer
                Icon {
                    anchors.centerIn: parent
                    name: "download-simple"
                    size: 28
                    color: Theme.m3.onSecondaryContainer
                }
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Nothing downloaded yet"
                color: Theme.m3.onSurfaceVariant
                font { family: Theme.font.family; pixelSize: 14; letterSpacing: 0.25 }
            }
        }
    }
}
