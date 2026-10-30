import QtQuick
import QtQuick.Effects
import Quickshell
import qs.common
import qs.glass
import qs.services

// Clipboard manager (Super+V): search, pins, image previews. Enter pastes
// into the previous window, Shift+Enter only copies, Alt+Enter pastes as
// plain text, Ctrl+P pins,
// Delete removes. Tab switches All / Text / Images. Items are grouped by
// when they were copied (Just now, 2 hours ago, Yesterday, Monday, 12 Oct);
// Images is a two-column grid.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "clipboard"
        screen: modelData

        Dialog {
            id: dialog
            readonly property var modelData: slot.modelData
            screen: modelData
            dialogName: "clipboard"
            contentWidth: 560
            contentHeight: 480
            padding: 16

            readonly property var kinds: ["all", "text", "images"]
            property int kind: 0
            property int selected: 0
            property bool confirmClear: false
            readonly property var rows: {
                Clipboard.items;
                Clipboard.pins;
                Clipboard.stamps;
                return Clipboard.filtered(field.text, kinds[kind]);
            }
            onRowsChanged: selected = Math.min(selected, Math.max(0, rows.length - 1))

            function reset() {
                field.text = "";
                kind = 0;
                selected = 0;
                confirmClear = false;
                Clipboard.refresh();
                field.forceActiveFocus();
            }
            onShownChanged: if (shown) reset()
            Component.onCompleted: reset()
            onTabPressed: kind = (kind + 1) % kinds.length
            readonly property bool grid: kinds[kind] === "images"

            // The rows in their sections: [{ title, start, count }]
            readonly property var groups: {
                const out = [];
                rows.forEach((r, i) => {
                    if (out.length === 0 || out[out.length - 1].title !== r.section)
                        out.push({ title: r.section, start: i, count: 0 });
                    out[out.length - 1].count++;
                });
                return out;
            }
            // Arrow keys: up/down in the list; in the grid also left/right,
            // and up/down by a row (into the section above or below)
            function move(dx, dy) {
                if (rows.length === 0)
                    return;
                if (!grid) {
                    selected = Math.max(0, Math.min(rows.length - 1, selected + dy));
                    return;
                }
                if (dx !== 0) {
                    selected = Math.max(0, Math.min(rows.length - 1, selected + dx));
                    return;
                }
                const gi = groups.findIndex(g => selected >= g.start && selected < g.start + g.count);
                const g = groups[gi];
                if (!g)
                    return;
                const col = (selected - g.start) % 2;
                if (dy > 0) {
                    if (selected + 2 < g.start + g.count)
                        selected += 2;
                    else if (Math.floor((selected - g.start) / 2) < Math.floor((g.count - 1) / 2))
                        selected = g.start + g.count - 1; // the shorter last row
                    else if (groups[gi + 1])
                        selected = groups[gi + 1].start + Math.min(col, groups[gi + 1].count - 1);
                } else {
                    if (selected - 2 >= g.start)
                        selected -= 2;
                    else if (groups[gi - 1]) {
                        const p = groups[gi - 1];
                        const lastRow = p.start + Math.floor((p.count - 1) / 2) * 2;
                        selected = Math.min(lastRow + col, p.start + p.count - 1);
                    }
                }
            }

            // Enter pastes into the app; Shift+Enter (or Shift+click) only
            // copies; Alt+Enter pastes as plain text
            function activate(i, plain, copyOnly) {
                const r = rows[i];
                if (!r)
                    return;
                Panels.close();
                if (copyOnly)
                    Clipboard.copy(r.pinned ? r.pin : r.item, plain);
                else
                    Clipboard.paste(r.pinned ? r.pin : r.item, plain);
            }
            function togglePin(i) {
                const r = rows[i];
                if (r)
                    r.pinned ? Clipboard.unpin(r.pin) : Clipboard.pin(r.item);
            }
            function remove(i) {
                const r = rows[i];
                if (r)
                    r.pinned ? Clipboard.unpin(r.pin) : Clipboard.remove(r.item);
            }

            // Search
            Squircle {
                id: search
                width: parent.width - kindPicker.width - 12
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
                    onTextChanged: dialog.selected = 0
                    Keys.onDownPressed: dialog.move(0, 1)
                    Keys.onUpPressed: dialog.move(0, -1)
                    // (in the grid only: in the list they move the cursor)
                    Keys.onLeftPressed: e => dialog.grid ? dialog.move(-1, 0) : (e.accepted = false)
                    Keys.onRightPressed: e => dialog.grid ? dialog.move(1, 0) : (e.accepted = false)
                    Keys.onReturnPressed: e => dialog.activate(dialog.selected, e.modifiers & Qt.AltModifier, e.modifiers & Qt.ShiftModifier)
                    Keys.onEnterPressed: e => dialog.activate(dialog.selected, e.modifiers & Qt.AltModifier, e.modifiers & Qt.ShiftModifier)
                    Keys.onPressed: e => {
                        if (e.key === Qt.Key_P && (e.modifiers & Qt.ControlModifier)) {
                            dialog.togglePin(dialog.selected);
                            e.accepted = true;
                        } else if (e.key === Qt.Key_Delete) {
                            dialog.remove(dialog.selected);
                            e.accepted = true;
                        }
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: field.text === ""
                        text: "Search clipboard"
                        color: Theme.color.tertiary
                        font: field.font
                    }
                }
            }

            Segmented {
                id: kindPicker
                anchors.right: parent.right
                y: (search.height - height) / 2
                width: 210
                options: ["All", "Text", "Images"]
                current: dialog.kind
                onSelected: i => dialog.kind = i
            }

            component Heading: Text {
                property bool first: false
                leftPadding: 10
                topPadding: first ? 0 : 12
                bottomPadding: 4
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
            }

            // All and Text: a list, a heading wherever the section changes
            ListView {
                id: list
                visible: !dialog.grid
                anchors { top: search.bottom; topMargin: 12; left: parent.left; right: parent.right; bottom: footer.top; bottomMargin: 8 }
                clip: true
                model: dialog.grid ? [] : dialog.rows
                currentIndex: dialog.selected
                boundsBehavior: Flickable.StopAtBounds
                highlightMoveDuration: 0
                spacing: 2

                delegate: Column {
                    id: rowItem
                    required property var modelData
                    required property int index
                    readonly property bool header: index === 0 || dialog.rows[index - 1]?.section !== modelData.section
                    readonly property bool isSelected: index === dialog.selected
                    width: list.width

                    Heading {
                        visible: rowItem.header
                        first: rowItem.index === 0
                        text: rowItem.modelData.section
                    }

                    Squircle {
                        width: parent.width
                        height: rowItem.modelData.image ? 168 : 44
                        radius: Theme.radius.control
                        color: rowItem.isSelected ? Qt.alpha(Theme.accent, Theme.dark ? 0.32 : 0.16) : rowMouse.containsMouse ? Theme.color.fill : "transparent"

                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onEntered: dialog.selected = rowItem.index
                            onClicked: m => dialog.activate(rowItem.index, false, m.modifiers & Qt.ShiftModifier)
                        }

                        readonly property bool isColor: /^#[0-9a-f]{6}$/i.test(rowItem.modelData.text.trim())
                        Rectangle {
                            visible: parent.isColor
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: 16
                            height: 16
                            radius: 4
                            color: parent.isColor ? rowItem.modelData.text.trim() : "transparent"
                            border.width: 1
                            border.color: Theme.color.separator
                        }
                        Icon {
                            visible: !rowItem.modelData.image && !parent.isColor
                            x: 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: 20
                            name: /^https?:\/\//.test(rowItem.modelData.text) ? "link" : "text-align-left"
                            size: 16
                            color: Theme.color.tertiary
                        }
                        // The picture, about twice as tall as it used to be,
                        // its own shape (wide pictures capped at most of the row)
                        Preview {
                            id: rowPreview
                            visible: !!rowItem.modelData.image
                            x: 8
                            anchors.verticalCenter: parent.verticalCenter
                            height: 152
                            width: Math.max(96, Math.min(parent.width - 120, aspect > 0 ? height * aspect : 200))
                            path: rowItem.modelData.image ?? ""
                            pinned: rowItem.modelData.pinned
                        }

                        Text {
                            visible: !rowItem.modelData.image
                            anchors { left: parent.left; leftMargin: 40; right: actions.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            text: rowItem.modelData.text.replace(/\s+/g, " ").trim()
                            elide: Text.ElideRight
                            maximumLineCount: 1
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }

                        Row {
                            id: actions
                            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            spacing: 2
                            opacity: rowItem.isSelected || rowItem.modelData.pinned ? 1 : 0
                            IconButton {
                                icon: "push-pin"
                                fill: rowItem.modelData.pinned
                                iconColor: rowItem.modelData.pinned ? Theme.accent : Theme.color.secondary
                                onClicked: dialog.togglePin(rowItem.index)
                            }
                            IconButton {
                                visible: rowItem.isSelected
                                icon: "trash"
                                onClicked: dialog.remove(rowItem.index)
                            }
                        }
                    }
                }
            }

            // Images: a two-column grid per section
            Flickable {
                id: gridView
                visible: dialog.grid
                anchors { top: search.bottom; topMargin: 12; left: parent.left; right: parent.right; bottom: footer.top; bottomMargin: 8 }
                clip: true
                contentHeight: sections.implicitHeight
                boundsBehavior: Flickable.StopAtBounds
                readonly property real cellW: Math.floor((width - 8) / 2)
                readonly property real cellH: Math.round(cellW * 0.72)
                // Keep the selected tile in view
                function reveal(item) {
                    const y = item.mapToItem(sections, 0, 0).y;
                    if (y < contentY)
                        contentY = Math.max(0, y - 24);
                    else if (y + item.height > contentY + height)
                        contentY = Math.min(contentHeight - height, y + item.height - height + 8);
                }

                Column {
                    id: sections
                    width: gridView.width
                    Repeater {
                        model: dialog.grid ? dialog.groups : []
                        Column {
                            id: group
                            required property var modelData
                            required property int index
                            width: sections.width
                            Heading {
                                first: group.index === 0
                                text: group.modelData.title
                            }
                            Grid {
                                columns: 2
                                spacing: 8
                                Repeater {
                                    model: group.modelData.count
                                    Squircle {
                                        id: tile
                                        required property int index
                                        readonly property int row: group.modelData.start + index
                                        readonly property var entry: dialog.rows[row] ?? ({})
                                        readonly property bool isSelected: row === dialog.selected
                                        width: gridView.cellW
                                        height: gridView.cellH
                                        radius: 12
                                        color: "transparent"
                                        border.width: isSelected ? 2 : 0
                                        border.color: Theme.accent
                                        onIsSelectedChanged: if (isSelected) Qt.callLater(() => gridView.reveal(tile))

                                        Preview {
                                            anchors { fill: parent; margins: 2 }
                                            path: tile.entry.image ?? ""
                                            pinned: !!tile.entry.pinned
                                        }
                                        MouseArea {
                                            id: tileMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            onEntered: dialog.selected = tile.row
                                            onClicked: m => dialog.activate(tile.row, false, m.modifiers & Qt.ShiftModifier)
                                        }
                                        // Pin and remove, on a dark chip so they read over any picture
                                        Squircle {
                                            anchors { top: parent.top; right: parent.right; margins: 8 }
                                            visible: tile.isSelected || tile.entry.pinned === true
                                            width: tileActions.implicitWidth + 4
                                            height: 32
                                            radius: 10
                                            color: Qt.rgba(0.09, 0.09, 0.11, 0.72)
                                            Row {
                                                id: tileActions
                                                anchors.centerIn: parent
                                                IconButton {
                                                    icon: "push-pin"
                                                    fill: tile.entry.pinned === true
                                                    iconColor: tile.entry.pinned ? Theme.accent : "white"
                                                    onClicked: dialog.togglePin(tile.row)
                                                }
                                                IconButton {
                                                    visible: tile.isSelected
                                                    icon: "trash"
                                                    iconColor: "white"
                                                    onClicked: dialog.remove(tile.row)
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
            ScrollIndicator {
                target: dialog.grid ? gridView : list
                anchors { right: parent.right; top: list.top; bottom: list.bottom; rightMargin: -6 }
            }

            // An image entry's picture, rounded (10px), cropped to fill; its
            // natural shape in `aspect` (the list sizes it from that)
            component Preview: Item {
                id: prev
                property string path
                property bool pinned: false
                readonly property real aspect: img.implicitHeight > 0 ? img.implicitWidth / img.implicitHeight : 0
                Squircle {
                    id: prevMask
                    anchors.fill: parent
                    radius: 10
                    visible: false
                    layer.enabled: true
                }
                Squircle {
                    anchors.fill: parent
                    radius: 10
                    color: Theme.color.fill
                }
                Image {
                    id: img
                    anchors.fill: parent
                    source: prev.path && (prev.pinned || Clipboard.previewReady[prev.path]) ? "file://" + prev.path : ""
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: Qt.size(640, 640)
                    asynchronous: true
                    cache: true
                    layer.enabled: true
                    layer.effect: MultiEffect {
                        maskEnabled: true
                        maskSource: prevMask
                        maskThresholdMin: 0.5
                        maskSpreadAtMin: 1
                    }
                }
            }

            // Empty and no-results states
            Column {
                anchors.centerIn: list
                visible: dialog.rows.length === 0
                spacing: 8
                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: field.text === "" ? "clipboard-text" : "magnifying-glass"
                    size: 32
                    color: Theme.color.tertiary
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: field.text !== "" ? "No matches for “" + field.text + "”" : dialog.kind === 2 ? "No images copied yet" : "Nothing copied yet"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: field.text !== "" ? "Try a different word, or switch to All." : "Text and images you copy appear here."
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
            }

            // Footer: shortcut hints and clear history
            Item {
                id: footer
                anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                height: 28

                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 14
                    Repeater {
                        model: [["↵", "Paste"], ["⇧↵", "Copy"], ["Alt ↵", "Plain text"], ["Ctrl P", "Pin"], ["Del", "Remove"]]
                        Row {
                            required property var modelData
                            spacing: 6
                            KeyCombo { keys: [parent.modelData[0]] }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: parent.modelData[1]
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                    }
                }

                // Clearing can't be undone (cliphist wipe), so it asks once, inline
                Squircle {
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    visible: Clipboard.items.length > 0
                    width: clearLabel.implicitWidth + 20
                    height: 28
                    radius: Theme.radius.control - 2
                    color: dialog.confirmClear ? Theme.color.danger : clearMouse.containsMouse ? Theme.color.fill : "transparent"
                    Text {
                        id: clearLabel
                        anchors.centerIn: parent
                        text: dialog.confirmClear ? "Clear " + Clipboard.items.length + " items" : "Clear history"
                        color: dialog.confirmClear ? "white" : Theme.color.danger
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                    }
                    MouseArea {
                        id: clearMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (dialog.confirmClear) {
                                Clipboard.clear();
                                dialog.confirmClear = false;
                            } else {
                                dialog.confirmClear = true;
                            }
                        }
                        onExited: dialog.confirmClear = false
                    }
                }
            }
        }
    }
}
