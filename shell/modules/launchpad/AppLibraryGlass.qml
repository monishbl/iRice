import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// App Library (Super+Ctrl+Space, or the Apps button in the dock): every app,
// grouped into category folders or listed A–Z (with a letter index down the
// side when the list is long). Typing searches; arrow keys move; Enter opens;
// Escape steps back out of a folder, then closes. Right-click, press and
// hold, or the Menu key: the app's menu (its shortcuts, Open, Keep in Dock).
Variants {
    model: Theme.material ? [] : Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "launchpad"
        screen: modelData
        elsewhere: RustUi.drawing("launchpad") // (tessera-ui draws it)

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
            onShownChanged: {
                if (shown)
                    lib.reset();
                else
                    menu = null;
            }
            Component.onCompleted: lib.reset()
            onTabPressed: lib.toggleView()

            // An app's menu (AppLibraryModel's items), a glass menu at the
            // pointer, or at the app for the Menu key
            property var menu: null
            function openMenu(entry, m, from) {
                const p = from.mapToItem(null, m.x, m.y);
                menu = { items: lib.menuItems(entry), x: p.x, y: p.y };
            }
            function openSelectedMenu() {
                const entry = lib.showingApps ? lib.visibleApps[lib.selected] : null;
                const cell = entry ? grid.itemAtIndex(lib.selected) : null;
                if (cell)
                    openMenu(entry, Qt.point(cell.width / 2, 92), cell);
            }
            Loader {
                active: dialog.menu !== null
                sourceComponent: ContextMenu {
                    screen: dialog.screen
                    items: dialog.menu?.items ?? []
                    pointX: (dialog.menu?.x ?? 0) + menuWidth / 2
                    pointY: dialog.menu?.y ?? 0
                    upward: false
                    onDismissed: dialog.menu = null
                }
            }

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
                        Keys.onPressed: e => {
                            if (e.key === Qt.Key_Menu || (e.key === Qt.Key_F10 && (e.modifiers & Qt.ShiftModifier))) {
                                dialog.openSelectedMenu();
                                e.accepted = true;
                            }
                        }
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
                anchors { top: header.bottom; topMargin: 24; left: parent.left; right: parent.right; rightMargin: scrubber.visible ? scrubber.width + 8 : 0; bottom: parent.bottom }
                readonly property int columns: Math.max(1, Math.floor(width / (lib.showingApps ? 120 : 210)))
                cellWidth: Math.floor(width / columns)
                cellHeight: lib.showingApps ? 116 : 236
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: lib.showingApps ? lib.visibleApps : lib.folders
                currentIndex: lib.selected
                highlightMoveDuration: 0
                highlightFollowsCurrentItem: false

                // The row of app i at the top (the index's jumps)
                function lead(i) {
                    const max = originY + Math.max(0, contentHeight - height);
                    scrollAnim.stop();
                    scrollAnim.to = Math.max(originY, Math.min(max, originY + Math.floor(i / columns) * cellHeight));
                    scrollAnim.start();
                }
                NumberAnimation {
                    id: scrollAnim
                    target: grid
                    property: "contentY"
                    duration: Theme.motion.panel
                    easing.type: Easing.OutCubic
                }

                delegate: Item {
                    id: cell
                    required property var modelData
                    required property int index
                    readonly property bool isSelected: index === lib.selected
                    // (a letter picked on the scrubber: its apps lit, the rest dimmed)
                    readonly property bool lit: lib.inPicked(index)
                    width: grid.cellWidth
                    height: grid.cellHeight

                    // App
                    Squircle {
                        visible: lib.showingApps
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 104
                        height: 104
                        radius: Theme.radius.module
                        color: cell.isSelected || cell.lit ? Qt.alpha(Theme.accent, Theme.dark ? 0.3 : 0.16) : appMouse.containsMouse ? Theme.color.fill : "transparent"
                        opacity: lib.picked !== "" && !cell.lit ? 0.4 : 1
                        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
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
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onEntered: lib.selected = cell.index
                            // Right-click or press and hold: its menu
                            onClicked: m => m.button === Qt.RightButton ? dialog.openMenu(cell.modelData, m, appMouse) : lib.activate(cell.index)
                            onPressAndHold: m => dialog.openMenu(cell.modelData, m, appMouse)
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

            // A–Z: a letter index down the side, as in Contacts. Click or drag
            // along it to jump; the letter in view is marked (bold, in the
            // accent), the one under the pointer gets a fill.
            Item {
                id: scrubber
                visible: lib.alphabetical && lib.letters.length > 1 && grid.contentHeight > grid.height
                anchors { right: parent.right; top: grid.top; bottom: grid.bottom }
                width: 24

                readonly property real step: lib.letters.length > 0 ? Math.min(22, height / lib.letters.length) : 22
                // (the letter of the first row in view)
                readonly property string current: visible ? lib.letterAt(Math.max(0, Math.floor((grid.contentY - grid.originY) / grid.cellHeight + 0.5)) * grid.columns) : ""
                readonly property int hovered: scrubMouse.containsMouse ? Math.floor((scrubMouse.mouseY - letterColumn.y) / step) : -1
                function jump(y) {
                    const x = lib.letters[Math.max(0, Math.min(lib.letters.length - 1, Math.floor((y - letterColumn.y) / step)))];
                    if (!x)
                        return;
                    lib.pick(x);
                    grid.lead(x.index);
                }

                Column {
                    id: letterColumn
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    Repeater {
                        model: scrubber.visible ? lib.letters : []
                        Item {
                            required property var modelData
                            required property int index
                            readonly property bool marked: modelData.letter === (lib.picked || scrubber.current)
                            width: scrubber.width
                            height: scrubber.step
                            Accessible.role: Accessible.Button
                            Accessible.name: "Jump to " + modelData.letter
                            Accessible.onPressAction: {
                                lib.pick(modelData);
                                grid.lead(modelData.index);
                            }
                            Rectangle {
                                anchors.centerIn: parent
                                width: Math.min(20, parent.height)
                                height: width
                                radius: width / 2
                                color: Theme.color.fill
                                opacity: scrubber.hovered === parent.index ? 1 : 0
                                Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                            }
                            Text {
                                anchors.centerIn: parent
                                text: parent.modelData.letter
                                color: parent.marked ? Theme.accentInk : Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: parent.marked ? Font.Bold : Font.Medium }
                            }
                        }
                    }
                }
                MouseArea {
                    id: scrubMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onPressed: e => scrubber.jump(e.y)
                    onPositionChanged: e => {
                        if (pressed)
                            scrubber.jump(e.y);
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
