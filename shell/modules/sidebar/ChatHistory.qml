import QtQuick
import qs.common
import qs.glass
import qs.services
import "../../services/AiChats.js" as Chats

// Past chats (the clock in the AI chat's header): a search field, then the
// chats grouped Pinned, Today, Yesterday, Previous 7 days and by month, each
// with its time and model, the open one marked. Click opens; "…", a
// right-click or press and hold gives Rename, Pin and Delete (at once, with
// Undo). Keys: Up/Down move, Enter opens, Delete deletes, F2 renames, the
// Menu key opens the menu, Escape goes back to the chat; typing searches.
// Glass: macOS-like rows with hover fills, a compact search field. Material:
// list items, the open one in secondary container, a search pill.
Item {
    id: root

    property var panelScreen: null // (for the menu's window)
    signal back // to the chat
    signal opened // a chat was opened: the chat shows it

    property string query: ""
    property string selectedId: ""
    property string renamingId: ""
    property string renameText: ""
    property bool byKeys: false // (the selection is ringed only while moving by keyboard)
    property var menuFor: null // { chat, x, y, end } (window coordinates; end: the menu ends at x)

    function enter() {
        field.text = "";
        renamingId = "";
        selectedId = Ai.currentId;
        now = Date.now();
        rebuild();
        Qt.callLater(() => field.forceActiveFocus());
    }

    // The rows shown: { header } or { chat } (AiChats.group). Held still
    // while renaming: rebuilt delegates would drop the field.
    property var rows: []
    property real now: Date.now()
    readonly property var chats: rows.filter(r => r.chat).map(r => r.chat)
    function rebuild() {
        if (renamingId !== "")
            return;
        const q = query.trim();
        rows = Chats.group(Chats.filter(Ai.sessions, q, Ai.searchedFor === q ? Ai.searchFound : []), now);
        if (selectedId !== "" && !chats.some(c => c.id === selectedId))
            selectedId = "";
    }
    Connections {
        target: Ai
        function onSessionsChanged() { root.rebuild(); }
        function onSearchFoundChanged() { root.rebuild(); }
        function onSearchedForChanged() { root.rebuild(); }
    }
    onQueryChanged: {
        rebuild();
        searchDelay.restart();
    }
    Timer {
        id: searchDelay
        interval: 250
        onTriggered: Ai.searchChats(root.query)
    }
    // (times stay right while it's open)
    Timer {
        interval: 60000
        repeat: true
        running: root.visible
        onTriggered: {
            root.now = Date.now();
            root.rebuild();
        }
    }

    // (Enter in the search field: the selected chat, or the first)
    function openSelected() {
        const c = chats.find(x => x.id === selectedId) ?? chats[0];
        if (c)
            open(c.id);
    }
    function open(id) {
        if (Ai.openChat(id))
            root.opened();
    }
    function remove(id) {
        // (the selection moves on to the next chat)
        const i = chats.findIndex(c => c.id === id);
        const next = chats[i + 1] ?? chats[i - 1] ?? null;
        Ai.deleteChat(id);
        selectedId = next ? next.id : "";
        if (list.activeFocus || field.activeFocus)
            return;
        list.forceActiveFocus();
    }
    function rename(id) {
        const c = chats.find(x => x.id === id);
        if (!c)
            return;
        selectedId = id;
        renameText = c.title;
        renamingId = id;
    }
    function finishRename(save) {
        const id = renamingId;
        if (id === "")
            return;
        renamingId = "";
        if (save)
            Ai.renameChat(id, renameText);
        rebuild();
        list.forceActiveFocus();
    }
    function menuItems(c) {
        return [
            { text: "Rename", action: () => root.rename(c.id) },
            { text: c.pinned ? "Unpin" : "Pin", action: () => Ai.setPinned(c.id, !c.pinned) },
            { separator: true },
            { text: "Delete", danger: true, action: () => root.remove(c.id) }
        ];
    }
    function openMenu(c, from, x, y, end) {
        const p = from.mapToItem(null, x, y);
        selectedId = c.id;
        menuFor = { chat: c, x: p.x, y: p.y, end: end === true };
    }
    function move(d) {
        const i = chats.findIndex(c => c.id === selectedId);
        byKeys = true;
        if (d < 0 && i <= 0)
            return field.forceActiveFocus();
        // (from the search field, Down lands on the selected chat first)
        const c = chats[!list.activeFocus && i >= 0 ? i : Math.max(0, Math.min(chats.length - 1, i + d))];
        if (!c)
            return;
        selectedId = c.id;
        list.forceActiveFocus();
        list.positionViewAtIndex(rows.findIndex(r => r.chat && r.chat.id === c.id), ListView.Contain);
    }

    Shortcut {
        sequence: "Ctrl+Z"
        enabled: root.visible && Ai.pendingDelete !== null && root.renamingId === ""
        onActivated: Ai.undoDelete()
    }

    // ── Search ──
    // (Glass: a compact field with a hairline; Material: a search pill)
    Squircle {
        id: searchBox
        width: parent.width
        height: Theme.material ? 44 : 30
        radius: Theme.material ? height / 2 : Theme.radius.control - 2
        color: Theme.material ? Qt.tint(Theme.m3.surfaceContainerHigh, Qt.alpha(Theme.m3.onSurface, searchHover.hovered && !field.activeFocus ? 0.08 : 0)) : Theme.color.fill
        border.width: Theme.material ? (field.activeFocus ? 2 : 0) : 1
        border.color: field.activeFocus ? Theme.accent : Theme.color.separator
        HoverHandler { id: searchHover; cursorShape: Qt.IBeamCursor }
        Icon {
            id: searchIcon
            x: Theme.material ? 16 : 9
            anchors.verticalCenter: parent.verticalCenter
            name: "magnifying-glass"
            size: Theme.material ? 18 : 14
            color: Theme.color.secondary
        }
        TextInput {
            id: field
            anchors { left: searchIcon.right; leftMargin: Theme.material ? 12 : 6; right: searchEnd.left; rightMargin: 4; verticalCenter: parent.verticalCenter }
            color: Theme.color.label
            selectionColor: Theme.material ? Qt.alpha(Theme.m3.primary, 0.3) : Theme.accentAlpha(0.4)
            selectByMouse: true
            clip: true
            font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.body + 1 : Theme.font.body }
            onTextChanged: root.query = text
            Accessible.role: Accessible.EditableText
            Accessible.name: "Search chats"
            Keys.onDownPressed: root.move(1)
            Keys.onReturnPressed: root.openSelected()
            Keys.onEnterPressed: root.openSelected()
            Keys.onEscapePressed: text !== "" ? text = "" : root.back()
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: field.text === ""
                text: "Search chats"
                color: Theme.color.tertiary
                font: field.font
            }
        }
        Item {
            id: searchEnd
            anchors { right: parent.right; rightMargin: Theme.material ? 6 : 2; verticalCenter: parent.verticalCenter }
            width: Theme.material ? 36 : 26
            height: width
            Spinner {
                anchors.centerIn: parent
                visible: Ai.searching && field.text !== ""
                size: 14
            }
            IconButton {
                anchors.fill: parent
                visible: field.text !== "" && !Ai.searching
                icon: "x"
                iconSize: Theme.material ? 18 : 12
                tooltip: "Clear search"
                onClicked: {
                    field.text = "";
                    field.forceActiveFocus();
                }
            }
        }
    }

    // ── The list ──
    ListView {
        id: list

        Momentum {}
        anchors { top: searchBox.bottom; topMargin: Theme.material ? 12 : 8; left: parent.left; right: parent.right; bottom: undoBar.shown ? undoBar.top : parent.bottom; bottomMargin: undoBar.shown ? 8 : 0 }
        clip: true
        model: root.rows
        boundsBehavior: Flickable.StopAtBounds
        activeFocusOnTab: root.chats.length > 0
        onActiveFocusChanged: if (activeFocus && root.selectedId === "" && root.chats.length) root.selectedId = root.chats[0].id
        Accessible.role: Accessible.List
        Accessible.name: "Past chats"

        Keys.onPressed: e => {
            const id = root.selectedId;
            root.byKeys = true;
            if (e.key === Qt.Key_Down || e.key === Qt.Key_Up) {
                root.move(e.key === Qt.Key_Down ? 1 : -1);
            } else if ((e.key === Qt.Key_Return || e.key === Qt.Key_Enter) && id !== "") {
                root.open(id);
            } else if (e.key === Qt.Key_Delete && id !== "") {
                root.remove(id);
            } else if (e.key === Qt.Key_F2 && id !== "") {
                root.rename(id);
            } else if ((e.key === Qt.Key_Menu || (e.key === Qt.Key_F10 && (e.modifiers & Qt.ShiftModifier))) && id !== "") {
                const r = list.itemAtIndex(root.rows.findIndex(x => x.chat && x.chat.id === id));
                const c = root.chats.find(x => x.id === id);
                if (r && c)
                    root.openMenu(c, r, 16, r.height);
            } else if (e.key === Qt.Key_Escape) {
                root.back();
            } else if (e.text !== "" && e.text.charCodeAt(0) >= 32 && !(e.modifiers & (Qt.ControlModifier | Qt.AltModifier))) {
                // Typing searches
                field.text += e.text;
                field.forceActiveFocus();
            } else {
                return;
            }
            e.accepted = true;
        }

        delegate: Item {
            id: row
            required property var modelData
            required property int index
            readonly property var chat: modelData.chat ?? null
            readonly property bool isOpen: chat !== null && chat.id === Ai.currentId
            readonly property bool selected: chat !== null && chat.id === root.selectedId
            readonly property bool renaming: chat !== null && chat.id === root.renamingId
            readonly property bool hot: rowMouse.containsMouse || moreButton.hovered || (root.menuFor !== null && root.menuFor.chat.id === chat?.id)
            // (Material: two-line list items; Glass: compact rows)
            width: list.width
            height: chat === null ? (index === 0 ? 0 : 8) + (Theme.material ? 36 : 26) : (Theme.material ? 60 : 46)

            // A group's name
            Text {
                visible: row.chat === null
                x: Theme.material ? 16 : 10
                anchors { bottom: parent.bottom; bottomMargin: Theme.material ? 8 : 6 }
                text: row.modelData.header ?? ""
                color: Theme.material ? Theme.m3.primary : Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }

            // A chat
            Squircle {
                id: tile
                visible: row.chat !== null
                anchors { fill: parent; topMargin: 1; bottomMargin: 1 }
                radius: Theme.material ? 16 : 8
                readonly property real stateLayer: rowMouse.pressed ? 0.1 : row.hot ? 0.08 : 0
                color: Theme.material ? (row.isOpen ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, stateLayer)) : Qt.alpha(Theme.m3.onSurface, stateLayer))
                    : row.isOpen ? Theme.accentTint : rowMouse.pressed ? Theme.color.fillPressed : row.hot ? Theme.color.fill : "transparent"
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                Accessible.role: Accessible.ListItem
                Accessible.name: row.chat ? row.chat.title + ", " + Chats.relativeTime(row.chat.updated, root.now) + (row.isOpen ? ", open" : "") + (row.chat.pinned ? ", pinned" : "") : ""
                Accessible.selected: row.selected
                Accessible.onPressAction: if (row.chat) root.open(row.chat.id)

                // The keyboard's place in the list
                Squircle {
                    anchors { fill: parent; margins: Theme.material ? -3 : -2 }
                    visible: row.selected && list.activeFocus && root.byKeys && !row.renaming
                    radius: tile.radius + (Theme.material ? 3 : 2)
                    color: "transparent"
                    border.width: Theme.material ? 3 : 2
                    border.color: Theme.material ? Theme.m3.secondary : Theme.accent
                }

                MouseArea {
                    id: rowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    enabled: !row.renaming
                    onPressed: root.byKeys = false
                    onClicked: m => m.button === Qt.RightButton ? root.openMenu(row.chat, rowMouse, m.x, m.y) : root.open(row.chat.id)
                    onPressAndHold: m => root.openMenu(row.chat, rowMouse, m.x, m.y)
                }

                Column {
                    anchors { left: parent.left; leftMargin: Theme.material ? 16 : 10; right: moreButton.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                    spacing: Theme.material ? 2 : 1
                    Item {
                        width: parent.width
                        height: Theme.material ? 22 : 18
                        Text {
                            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                            visible: !row.renaming
                            text: row.chat?.title ?? ""
                            elide: Text.ElideRight
                            color: Theme.material && row.isOpen ? Theme.m3.onSecondaryContainer : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.body + 1 : Theme.font.body; weight: Font.Medium }
                        }
                        // Rename in place: Enter saves, Escape cancels
                        Loader {
                            anchors { left: parent.left; leftMargin: -4; right: parent.right; verticalCenter: parent.verticalCenter }
                            active: row.renaming
                            sourceComponent: Squircle {
                                height: Theme.material ? 30 : 24
                                radius: Theme.material ? 8 : 6
                                color: Theme.material ? Theme.m3.surfaceContainerHighest : Theme.color.fill
                                border.width: Theme.material ? 2 : 1
                                border.color: Theme.accent
                                TextInput {
                                    id: renameField
                                    anchors { fill: parent; leftMargin: 4; rightMargin: 4 }
                                    verticalAlignment: TextInput.AlignVCenter
                                    text: root.renameText
                                    color: Theme.color.label
                                    selectionColor: Theme.material ? Qt.alpha(Theme.m3.primary, 0.3) : Theme.accentAlpha(0.4)
                                    selectByMouse: true
                                    clip: true
                                    maximumLength: 120
                                    font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.body + 1 : Theme.font.body; weight: Font.Medium }
                                    Accessible.role: Accessible.EditableText
                                    Accessible.name: "Chat name"
                                    onTextChanged: root.renameText = text
                                    Component.onCompleted: {
                                        selectAll();
                                        forceActiveFocus();
                                    }
                                    Keys.onReturnPressed: root.finishRename(true)
                                    Keys.onEnterPressed: root.finishRename(true)
                                    Keys.onEscapePressed: root.finishRename(false)
                                    // (clicking away keeps what was typed, as in Files)
                                    onActiveFocusChanged: if (!activeFocus && root.renamingId === row.chat?.id) root.finishRename(true)
                                }
                            }
                        }
                    }
                    Text {
                        width: parent.width
                        readonly property string modelName: Ai.models.find(m => m.key === row.chat?.model)?.name ?? row.chat?.modelName ?? ""
                        text: row.chat ? Chats.relativeTime(row.chat.updated, root.now) + (modelName !== "" ? " · " + modelName : "") : ""
                        elide: Text.ElideRight
                        color: Theme.material && row.isOpen ? Theme.m3.onSecondaryContainer : Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                }

                // (Glass: shown on hover, as in Finder's sidebar; Material: always)
                IconButton {
                    id: moreButton
                    anchors { right: parent.right; rightMargin: Theme.material ? 6 : 6; verticalCenter: parent.verticalCenter }
                    width: Theme.material ? 40 : 28
                    height: width
                    opacity: Theme.material || row.hot || row.isOpen || row.selected ? 1 : 0
                    icon: "dots-three"
                    iconSize: Theme.material ? 20 : 16
                    iconColor: Theme.material && row.isOpen ? Theme.m3.onSecondaryContainer : Theme.color.secondary
                    tooltip: "Chat options"
                    activeFocusOnTab: false // (the list's keys reach it: Menu, F2, Delete)
                    accessibleName: "Options for “" + (row.chat?.title ?? "") + "”"
                    onClicked: {
                        root.byKeys = false;
                        root.openMenu(row.chat, moreButton, moreButton.width, moreButton.height + 4, true);
                    }
                }
            }
        }
    }

    // ── Nothing to show ──
    Column {
        anchors.centerIn: list
        width: parent.width - 40
        visible: root.rows.length === 0
        spacing: 8
        readonly property bool none: Ai.sessions.length === 0

        Icon { anchors.horizontalCenter: parent.horizontalCenter; visible: !Theme.material; name: parent.none ? "clock-counter-clockwise" : "magnifying-glass"; size: 34; color: Theme.color.tertiary }
        // Material: on a soft shape, as the chat's own empty state
        Loader {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: Theme.material
            active: Theme.material
            sourceComponent: Item {
                implicitWidth: 72
                implicitHeight: 80
                MaterialShape {
                    width: 72
                    height: 72
                    shape: "cookie9"
                    color: Theme.m3.secondaryContainer
                    Icon { anchors.centerIn: parent; name: Ai.sessions.length === 0 ? "clock-counter-clockwise" : "magnifying-glass"; size: 30; color: Theme.m3.onSecondaryContainer }
                }
            }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: parent.none ? "No past chats yet" : "No chats match “" + root.query.trim() + "”"
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.large : Theme.font.title; weight: Theme.material ? Font.Normal : Font.DemiBold }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: parent.none ? "Chats are kept here once you've sent a message, so you can come back to them." : "Try other words, or fewer of them."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Item { width: 1; height: 8; visible: !parent.none }
        ChatButton {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: !parent.none
            icon: "x"
            label: "Clear search"
            onClicked: {
                field.text = "";
                field.forceActiveFocus();
            }
        }
    }

    UndoBar {
        id: undoBar
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
    }

    // A chat's menu: the shell's menu at the pointer (or under "…")
    Loader {
        active: root.menuFor !== null
        sourceComponent: ContextMenu {
            screen: root.panelScreen
            items: root.menuFor ? root.menuItems(root.menuFor.chat) : []
            pointX: (root.menuFor?.x ?? 0) + (root.menuFor?.end ? -menuWidth / 2 : menuWidth / 2)
            pointY: root.menuFor?.y ?? 0
            upward: false
            onDismissed: {
                root.menuFor = null;
                if (root.renamingId === "")
                    list.forceActiveFocus();
            }
        }
    }
}
