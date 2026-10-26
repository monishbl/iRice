import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// Clipboard manager (Super+V): search, pins, image previews. Enter pastes
// into the previous window, Shift+Enter only copies, Alt+Enter pastes as
// plain text, Ctrl+P pins,
// Delete removes. Tab switches All / Text / Images.
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
            Rectangle {
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
                    selectionColor: Qt.alpha(Theme.accent, 0.4)
                    clip: true
                    font { family: Theme.font.family; pixelSize: Theme.font.body + 1 }
                    onTextChanged: dialog.selected = 0
                    Keys.onDownPressed: dialog.selected = Math.min(dialog.rows.length - 1, dialog.selected + 1)
                    Keys.onUpPressed: dialog.selected = Math.max(0, dialog.selected - 1)
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

            ListView {
                id: list
                anchors { top: search.bottom; topMargin: 12; left: parent.left; right: parent.right; bottom: footer.top; bottomMargin: 8 }
                clip: true
                model: dialog.rows
                currentIndex: dialog.selected
                boundsBehavior: Flickable.StopAtBounds
                highlightMoveDuration: 0
                spacing: 2

                delegate: Column {
                    id: rowItem
                    required property var modelData
                    required property int index
                    readonly property bool header: index === 0 || dialog.rows[index - 1].pinned !== modelData.pinned
                    readonly property bool isSelected: index === dialog.selected
                    width: list.width

                    Text {
                        visible: rowItem.header
                        leftPadding: 10
                        topPadding: rowItem.index === 0 ? 0 : 12
                        bottomPadding: 4
                        text: rowItem.modelData.pinned ? "Pinned" : "History"
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                    }

                    Rectangle {
                        width: parent.width
                        height: rowItem.modelData.image ? 92 : 44
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
                        Rectangle {
                            visible: !!rowItem.modelData.image
                            x: 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: 120
                            height: 76
                            radius: 8
                            color: Theme.color.fill
                            clip: true
                            Image {
                                anchors.fill: parent
                                source: rowItem.modelData.image && (rowItem.modelData.pinned || Clipboard.previewVersion > 0) ? "file://" + rowItem.modelData.image + "?v=" + Clipboard.previewVersion : ""
                                fillMode: Image.PreserveAspectCrop
                                sourceSize: Qt.size(240, 152)
                                asynchronous: true
                                cache: false
                            }
                        }

                        Text {
                            anchors { left: parent.left; leftMargin: rowItem.modelData.image ? 142 : 40; right: actions.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            text: rowItem.modelData.image ? "Image" : rowItem.modelData.text.replace(/\s+/g, " ").trim()
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
                Rectangle {
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
