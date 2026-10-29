import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Downloads stack: the newest files in ~/Downloads as a glass grid above the
// dock, like macOS's grid stack. Click a file to open it.
GlassWindow {
    id: root

    required property var dock
    readonly property string dir: Quickshell.env("HOME") + "/Downloads"
    readonly property int cols: 4
    readonly property int cell: 96
    readonly property int m: glass.shadowMargin

    screen: dock.screen
    name: "stack"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    anchors { top: true; bottom: true; left: true; right: true }

    property real progress: 0
    Component.onCompleted: {
        progress = 1;
        body.forceActiveFocus();
    }
    Behavior on progress { SpringAnimation { spring: 4.5; damping: 0.34; epsilon: 0.005 } }

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
        width: root.cols * root.cell + 24 + 2 * root.m
        height: Math.max(1, Math.ceil(root.shown / root.cols)) * (root.cell + 8) + 24 + 44 + 2 * root.m
        x: Math.max(0, Math.min(root.width - width, root.dock.stackX - width / 2))
        y: root.height - root.dock.barHeight - root.dock.bottomGap - 8 - height + root.m
        opacity: Math.min(1, root.progress)
        scale: 0.92 + 0.08 * root.progress
        transformOrigin: Item.Bottom
        onScaleChanged: glass.updatePosition()
        focus: true
        Keys.onEscapePressed: root.dock.stackOpen = false

        Glass {
            id: glass
            anchors.fill: parent
            radius: Theme.radius.popover
        }
        MouseArea { anchors.fill: parent; anchors.margins: root.m }

        Grid {
            x: root.m + 12
            y: root.m + 12
            columns: root.cols
            rowSpacing: 8
            Repeater {
                model: root.shown
                Item {
                    id: fileCell
                    required property int index
                    readonly property string path: files.get(index, "filePath") ?? ""
                    width: root.cell
                    height: root.cell
                    Rectangle {
                        anchors.fill: parent
                        radius: Theme.radius.control
                        color: fileMouse.containsMouse ? Theme.color.fill : "transparent"
                    }
                    AppIcon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        y: 8
                        size: 48
                        styled: false
                        source: IconLookup.path(Search.iconForFile(fileCell.path), "text-x-generic")
                    }
                    Text {
                        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 8 }
                        width: parent.width - 8
                        horizontalAlignment: Text.AlignHCenter
                        text: fileCell.path.split("/").pop()
                        elide: Text.ElideMiddle
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    MouseArea {
                        id: fileMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        // Right-click: share it with a nearby device
                        onClicked: m => {
                            root.dock.stackOpen = false;
                            if (m.button === Qt.RightButton)
                                Panels.share([fileCell.path]);
                            else
                                Files.open(fileCell.path);
                        }
                    }
                    // Quick Look without opening the app
                    IconButton {
                        visible: fileMouse.containsMouse || hovered
                        readonly property bool hovered: false
                        anchors { right: parent.right; top: parent.top; margins: 2 }
                        width: 24
                        height: 24
                        icon: "eye"
                        iconSize: 14
                        onClicked: Panels.quickLook(fileCell.path)
                    }
                }
            }
        }

        Text {
            visible: root.shown === 0
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -20
            text: "Nothing downloaded yet"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }

        // Open the folder
        Rectangle {
            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: root.m + 12 }
            width: openLabel.implicitWidth + 28
            height: 28
            radius: 14
            color: openMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
            Text {
                id: openLabel
                anchors.centerIn: parent
                text: "Open in Files"
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
            MouseArea {
                id: openMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: {
                    root.dock.stackOpen = false;
                    Files.openFolder(root.dir);
                }
            }
        }
    }
}
