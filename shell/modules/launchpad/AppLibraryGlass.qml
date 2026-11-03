import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// App Library (Super+Shift+Space, or the Apps button in the dock): every app,
// grouped into category folders or listed A–Z. Typing searches; arrow keys
// move; Enter opens; Escape steps back out of a folder, then closes.
Variants {
    model: Theme.material ? [] : Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "launchpad"
        screen: modelData

        Dialog {
            id: dialog
            readonly property var modelData: slot.modelData
            screen: modelData
            dialogName: "launchpad"
            contentWidth: Math.min(1120, (screen?.width ?? 1920) - 160)
            contentHeight: Math.min(720, (screen?.height ?? 1080) - 200)
            padding: 24

            // The apps, the search and the selection: AppLibraryModel
            AppLibraryModel {
                id: lib
                searchField: field
            }
            onShownChanged: if (shown) lib.reset()
            Component.onCompleted: lib.reset()
            onTabPressed: lib.toggleView()

            // Header: back button (in a folder), search, view switch
            Item {
                id: header
                width: parent.width
                height: 36

                IconButton {
                    id: backButton
                    visible: lib.folder !== null && lib.query === ""
                    anchors.verticalCenter: parent.verticalCenter
                    width: 36
                    height: 36
                    icon: "caret-left"
                    iconSize: 18
                    iconColor: Theme.color.label
                    onClicked: lib.back()
                }
                Text {
                    visible: backButton.visible
                    anchors { left: backButton.right; leftMargin: 8; verticalCenter: parent.verticalCenter }
                    text: lib.folder?.name ?? ""
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.large; weight: Font.DemiBold }
                }

                Squircle {
                    id: search
                    anchors.centerIn: parent
                    width: 320
                    height: 36
                    radius: Theme.radius.control
                    color: Theme.color.fill
                    border.width: 1
                    border.color: field.activeFocus ? Theme.accent : "transparent"
                    Icon {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        name: "magnifying-glass"
                        size: 16
                        color: Theme.color.tertiary
                    }
                    TextInput {
                        id: field
                        anchors { fill: parent; leftMargin: 34; rightMargin: 10 }
                        verticalAlignment: TextInput.AlignVCenter
                        color: Theme.color.label
                        selectionColor: Theme.accentAlpha(0.4)
                        clip: true
                        font { family: Theme.font.family; pixelSize: Theme.font.body + 1 }
                        onTextChanged: lib.selected = 0
                        Keys.onEscapePressed: lib.back()
                        Keys.onReturnPressed: lib.activate(lib.selected)
                        Keys.onEnterPressed: lib.activate(lib.selected)
                        Keys.onLeftPressed: e => { if (text === "") lib.move(-1); else e.accepted = false; }
                        Keys.onRightPressed: e => { if (text === "") lib.move(1); else e.accepted = false; }
                        Keys.onUpPressed: lib.move(-grid.columns)
                        Keys.onDownPressed: lib.move(grid.columns)
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: field.text === ""
                            text: "Search " + lib.apps.length + " apps"
                            color: Theme.color.tertiary
                            font: field.font
                        }
                    }
                }

                Segmented {
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    width: 200
                    options: ["Categories", "All Apps"]
                    current: lib.view
                    onSelected: i => {
                        lib.view = i;
                        lib.folder = null;
                        lib.selected = 0;
                    }
                }
            }

            GridView {
                id: grid

                Momentum {}
                anchors { top: header.bottom; topMargin: 24; left: parent.left; right: parent.right; bottom: parent.bottom }
                readonly property int columns: Math.max(1, Math.floor(width / (lib.showingApps ? 120 : 210)))
                cellWidth: Math.floor(width / columns)
                cellHeight: lib.showingApps ? 116 : 236
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: lib.showingApps ? lib.visibleApps : lib.folders
                currentIndex: lib.selected
                highlightMoveDuration: 0
                highlightFollowsCurrentItem: false

                delegate: Item {
                    id: cell
                    required property var modelData
                    required property int index
                    readonly property bool isSelected: index === lib.selected
                    width: grid.cellWidth
                    height: grid.cellHeight

                    // App
                    Squircle {
                        visible: lib.showingApps
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 104
                        height: 104
                        radius: Theme.radius.module
                        color: cell.isSelected ? Qt.alpha(Theme.accent, Theme.dark ? 0.3 : 0.16) : appMouse.containsMouse ? Theme.color.fill : "transparent"
                        AppIcon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            y: 12
                            size: 56
                            source: lib.showingApps ? Apps.iconFor(cell.modelData) : ""
                            scale: appMouse.pressed ? 0.94 : 1
                            Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
                        }
                        Text {
                            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 10 }
                            width: parent.width - 12
                            horizontalAlignment: Text.AlignHCenter
                            text: lib.showingApps ? cell.modelData.name : ""
                            elide: Text.ElideRight
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                        MouseArea {
                            id: appMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onEntered: lib.selected = cell.index
                            onClicked: lib.activate(cell.index)
                        }
                    }

                    // Folder: three large icons and a cluster of small ones, like App Library
                    Column {
                        visible: !lib.showingApps
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 8
                        Squircle {
                            id: folderCard
                            width: 188
                            height: 188
                            radius: Theme.radius.panel
                            color: folderMouse.containsMouse || cell.isSelected ? Theme.color.fillHover : Theme.color.platter
                            border.width: cell.isSelected ? 2 : 1
                            border.color: cell.isSelected ? Theme.accent : Theme.color.separator
                            scale: folderMouse.pressed ? 0.97 : 1
                            Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
                            readonly property var list: lib.showingApps ? [] : cell.modelData.apps

                            Grid {
                                anchors.centerIn: parent
                                columns: 2
                                spacing: 16
                                Repeater {
                                    model: folderCard.list.slice(0, folderCard.list.length > 4 ? 3 : 4)
                                    AppIcon {
                                        required property var modelData
                                        size: 64
                                        source: Apps.iconFor(modelData)
                                    }
                                }
                                // Fourth slot: up to four more, small
                                Grid {
                                    visible: folderCard.list.length > 4
                                    width: 64
                                    height: 64
                                    columns: 2
                                    spacing: 4
                                    padding: 2
                                    Repeater {
                                        model: folderCard.list.slice(3, 7)
                                        AppIcon {
                                            required property var modelData
                                            size: 27
                                            source: Apps.iconFor(modelData)
                                        }
                                    }
                                }
                            }
                            MouseArea {
                                id: folderMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onEntered: lib.selected = cell.index
                                onClicked: lib.activate(cell.index)
                            }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: lib.showingApps ? "" : cell.modelData.name + "  " + cell.modelData.apps.length
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                        }
                    }
                }
            }

            // No results
            Column {
                anchors.centerIn: grid
                visible: lib.query !== "" && lib.visibleApps.length === 0
                spacing: 8
                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: "magnifying-glass"
                    size: 32
                    color: Theme.color.tertiary
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "No apps match “" + field.text + "”"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Check the spelling, or search the web in Search."
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
            }
        }
    }
}
