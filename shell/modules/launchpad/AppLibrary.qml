import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// App Library (Super+Shift+Space, or the Apps button in the dock): every app,
// grouped into category folders or listed A–Z. Typing searches; arrow keys
// move; Enter opens; Escape steps back out of a folder, then closes.
Variants {
    model: Quickshell.screens

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

            // [label, freedesktop categories]; apps go in the first match
            readonly property var groups: [
                ["Productivity", ["Office", "Calendar", "ContactManagement", "Finance", "WordProcessor", "Spreadsheet"]],
                ["Development", ["Development", "IDE", "TextEditor"]],
                ["Internet", ["Network", "WebBrowser", "Email", "Chat", "InstantMessaging"]],
                ["Creativity", ["Graphics", "Photography", "2DGraphics", "3DGraphics", "VectorGraphics", "RasterGraphics"]],
                ["Entertainment", ["AudioVideo", "Audio", "Video", "Player", "Music"]],
                ["Games", ["Game"]],
                ["Education", ["Education", "Science", "Math"]],
                ["Utilities", ["Utility", "Accessories", "FileManager", "TerminalEmulator", "Archiving"]],
                ["System", ["System", "Settings", "Monitor", "PackageManager", "HardwareSettings"]]
            ]

            readonly property var apps: DesktopEntries.applications.values.filter(a => !a.noDisplay).slice().sort((a, b) => a.name.localeCompare(b.name))
            readonly property var folders: {
                const map = {};
                for (const a of apps) {
                    const cats = a.categories ?? [];
                    const g = groups.find(g => g[1].some(c => cats.includes(c)));
                    const key = g ? g[0] : "Other";
                    (map[key] = map[key] ?? []).push(a);
                }
                return groups.map(g => g[0]).concat(["Other"]).filter(k => map[k]).map(k => ({ name: k, apps: map[k] }));
            }

            property int view: 0 // 0 folders, 1 all apps
            property var folder: null // open folder
            property int selected: 0
            readonly property string query: field.text.trim().toLowerCase()
            readonly property bool showingApps: query !== "" || view === 1 || folder !== null
            readonly property var visibleApps: query !== "" ? apps.map(a => ({ a: a, s: Math.max(Search.score(a.name, query), Search.score(a.genericName, query) * 0.8, Search.score((a.keywords ?? []).join(" "), query) * 0.6) })).filter(x => x.s >= 30).sort((x, y) => y.s - x.s).map(x => x.a) : folder ? folder.apps : apps
            readonly property int count: showingApps ? visibleApps.length : folders.length
            onCountChanged: selected = Math.min(selected, Math.max(0, count - 1))

            function reset() {
                field.text = "";
                folder = null;
                selected = 0;
                field.forceActiveFocus();
            }
            onShownChanged: if (shown) reset()
            Component.onCompleted: reset()
            onTabPressed: {
                view = 1 - view;
                folder = null;
                selected = 0;
            }

            function activate(i) {
                if (showingApps) {
                    const a = visibleApps[i];
                    if (!a)
                        return;
                    Panels.close();
                    Apps.launch(a);
                } else if (folders[i]) {
                    folder = folders[i];
                    selected = 0;
                }
            }
            function back() {
                if (field.text !== "")
                    field.text = "";
                else if (folder) {
                    selected = Math.max(0, folders.indexOf(folder));
                    folder = null;
                } else
                    Panels.close();
            }
            function move(d) {
                selected = Math.max(0, Math.min(count - 1, selected + d));
            }

            // Header: back button (in a folder), search, view switch
            Item {
                id: header
                width: parent.width
                height: 36

                IconButton {
                    id: backButton
                    visible: dialog.folder !== null && dialog.query === ""
                    anchors.verticalCenter: parent.verticalCenter
                    width: 36
                    height: 36
                    icon: "caret-left"
                    iconSize: 18
                    iconColor: Theme.color.label
                    onClicked: dialog.back()
                }
                Text {
                    visible: backButton.visible
                    anchors { left: backButton.right; leftMargin: 8; verticalCenter: parent.verticalCenter }
                    text: dialog.folder?.name ?? ""
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.large; weight: Font.DemiBold }
                }

                Rectangle {
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
                        selectionColor: Qt.alpha(Theme.accent, 0.4)
                        clip: true
                        font { family: Theme.font.family; pixelSize: Theme.font.body + 1 }
                        onTextChanged: dialog.selected = 0
                        Keys.onEscapePressed: dialog.back()
                        Keys.onReturnPressed: dialog.activate(dialog.selected)
                        Keys.onEnterPressed: dialog.activate(dialog.selected)
                        Keys.onLeftPressed: e => { if (text === "") dialog.move(-1); else e.accepted = false; }
                        Keys.onRightPressed: e => { if (text === "") dialog.move(1); else e.accepted = false; }
                        Keys.onUpPressed: dialog.move(-grid.columns)
                        Keys.onDownPressed: dialog.move(grid.columns)
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: field.text === ""
                            text: "Search " + dialog.apps.length + " apps"
                            color: Theme.color.tertiary
                            font: field.font
                        }
                    }
                }

                Segmented {
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    width: 200
                    options: ["Categories", "All Apps"]
                    current: dialog.view
                    onSelected: i => {
                        dialog.view = i;
                        dialog.folder = null;
                        dialog.selected = 0;
                    }
                }
            }

            GridView {
                id: grid
                anchors { top: header.bottom; topMargin: 24; left: parent.left; right: parent.right; bottom: parent.bottom }
                readonly property int columns: Math.max(1, Math.floor(width / (dialog.showingApps ? 120 : 210)))
                cellWidth: Math.floor(width / columns)
                cellHeight: dialog.showingApps ? 116 : 236
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: dialog.showingApps ? dialog.visibleApps : dialog.folders
                currentIndex: dialog.selected
                highlightMoveDuration: 0
                highlightFollowsCurrentItem: false

                delegate: Item {
                    id: cell
                    required property var modelData
                    required property int index
                    readonly property bool isSelected: index === dialog.selected
                    width: grid.cellWidth
                    height: grid.cellHeight

                    // App
                    Rectangle {
                        visible: dialog.showingApps
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 104
                        height: 104
                        radius: Theme.radius.module
                        color: cell.isSelected ? Qt.alpha(Theme.accent, Theme.dark ? 0.3 : 0.16) : appMouse.containsMouse ? Theme.color.fill : "transparent"
                        AppIcon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            y: 12
                            size: 56
                            source: dialog.showingApps ? Apps.iconFor(cell.modelData) : ""
                            scale: appMouse.pressed ? 0.94 : 1
                            Behavior on scale { NumberAnimation { duration: Theme.motion.press } }
                        }
                        Text {
                            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 10 }
                            width: parent.width - 12
                            horizontalAlignment: Text.AlignHCenter
                            text: dialog.showingApps ? cell.modelData.name : ""
                            elide: Text.ElideRight
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                        MouseArea {
                            id: appMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onEntered: dialog.selected = cell.index
                            onClicked: dialog.activate(cell.index)
                        }
                    }

                    // Folder: three large icons and a cluster of small ones, like App Library
                    Column {
                        visible: !dialog.showingApps
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 8
                        Rectangle {
                            id: folderCard
                            width: 188
                            height: 188
                            radius: Theme.radius.panel
                            color: folderMouse.containsMouse || cell.isSelected ? Theme.color.fillHover : Theme.color.platter
                            border.width: cell.isSelected ? 2 : 1
                            border.color: cell.isSelected ? Theme.accent : Theme.color.separator
                            scale: folderMouse.pressed ? 0.97 : 1
                            Behavior on scale { NumberAnimation { duration: Theme.motion.press } }
                            readonly property var list: dialog.showingApps ? [] : cell.modelData.apps

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
                                onEntered: dialog.selected = cell.index
                                onClicked: dialog.activate(cell.index)
                            }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: dialog.showingApps ? "" : cell.modelData.name + "  " + cell.modelData.apps.length
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                        }
                    }
                }
            }

            // No results
            Column {
                anchors.centerIn: grid
                visible: dialog.query !== "" && dialog.visibleApps.length === 0
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
                    text: "Check the spelling, or search the web from Spotlight."
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
            }
        }
    }
}
