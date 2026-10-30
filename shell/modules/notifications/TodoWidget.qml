import QtQuick
import qs.common
import qs.services
import qs.modules.controlcenter

// To-do list: add with Enter, tick to complete, double-click to rename,
// remove with the × on hover (Undo for a few seconds). Saved in UiState.
// Also inside the desktop Calendar widget's To-do tab (no padding of its
// own, a fixed height: the list scrolls).
Platter {
    id: root

    property int padding: 12
    property bool fitHeight: false
    readonly property var todos: Todos.list
    property var undo: null // { item }, offered back for a few seconds
    implicitHeight: column.implicitHeight + 2 * padding

    function remove(todo) {
        const removed = Todos.remove(todo);
        undo = removed ? { item: removed } : null;
        undoTimer.restart();
    }
    Timer {
        id: undoTimer
        interval: 8000
        onTriggered: root.undo = null
    }

    Column {
        id: column
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: root.padding }
        spacing: 6

        Squircle {
            id: addRow
            width: parent.width
            height: 32
            radius: Theme.radius.control
            color: Theme.color.fill
            Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: "plus"; bold: true; size: 12; color: Theme.color.secondary }
            TextInput {
                id: input
                anchors { fill: parent; leftMargin: 30; rightMargin: 10 }
                verticalAlignment: TextInput.AlignVCenter
                color: Theme.color.label
                clip: true
                font { family: Theme.font.family; pixelSize: Theme.font.body }
                onAccepted: {
                    if (text.trim() === "")
                        return;
                    if (Todos.add(text))
                        text = "";
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: input.text === ""
                    width: parent.width
                    elide: Text.ElideRight
                    text: "Add a to-do (e.g. call mum tomorrow 6pm)"
                    color: Theme.color.tertiary
                    font: input.font
                }
            }
        }

        Text {
            id: emptyText
            visible: root.todos.length === 0 && !root.undo
            width: parent.width
            topPadding: 8
            bottomPadding: 8
            horizontalAlignment: Text.AlignHCenter
            text: "Nothing to do. Nice."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }

        // (fitHeight: scrolls in what's left)
        Flickable {
            visible: root.todos.length > 0
            width: parent.width
            height: root.fitHeight ? Math.max(30, Math.min(contentHeight, root.height - 2 * root.padding - addRow.height - column.spacing - (undoRow.visible ? undoRow.height + column.spacing : 0))) : contentHeight
            contentHeight: todoList.implicitHeight
            interactive: root.fitHeight && contentHeight > height
            clip: root.fitHeight
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: todoList
                width: parent.width
                spacing: 6
                Repeater {
                    model: root.todos
                    Item {
                        id: item
                        required property var modelData
                        required property int index
                        property bool renaming: false
                        width: todoList.width
                        height: Todos.dueLabel(modelData) ? 40 : 30

                        Rectangle {
                            id: box
                            anchors.verticalCenter: parent.verticalCenter
                            width: 18
                            height: 18
                            radius: 9
                            color: item.modelData.done ? Theme.accent : "transparent"
                            border.width: item.modelData.done ? 0 : 1.5
                            border.color: Theme.color.tertiary
                            Icon { anchors.centerIn: parent; visible: item.modelData.done; name: "check"; bold: true; size: 10; color: Theme.color.onAccent }
                            MouseArea {
                                anchors { fill: parent; margins: -6 }
                                onClicked: Todos.update(item.modelData, { done: !item.modelData.done })
                            }
                        }
                        Column {
                            anchors { left: box.right; leftMargin: 10; right: del.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                            Item {
                                width: parent.width
                                height: label.implicitHeight
                                Text {
                                    id: label
                                    visible: !item.renaming
                                    width: parent.width
                                    text: item.modelData.text
                                    elide: Text.ElideRight
                                    color: item.modelData.done ? Theme.color.tertiary : Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.body; strikeout: item.modelData.done }
                                    MouseArea {
                                        anchors.fill: parent
                                        onDoubleClicked: {
                                            item.renaming = true;
                                            renameField.text = item.modelData.text;
                                            renameField.forceActiveFocus();
                                            renameField.selectAll();
                                        }
                                    }
                                }
                                // Rename in place: Enter or clicking away keeps it, Esc cancels
                                TextInput {
                                    id: renameField
                                    visible: item.renaming
                                    width: parent.width
                                    color: Theme.color.label
                                    selectionColor: Theme.accentAlpha(0.4)
                                    selectByMouse: true
                                    clip: true
                                    font: label.font
                                    function finish(keep) {
                                        if (!item.renaming)
                                            return;
                                        item.renaming = false;
                                        if (keep && text.trim() !== "" && text.trim() !== item.modelData.text)
                                            Todos.update(item.modelData, { text: text.trim() });
                                    }
                                    onAccepted: finish(true)
                                    onActiveFocusChanged: if (!activeFocus) finish(true)
                                    Keys.onEscapePressed: finish(false)
                                }
                            }
                            // When it's due; red once overdue
                            Text {
                                visible: text !== ""
                                text: [Todos.dueLabel(item.modelData), item.modelData.google ? "Google · " + item.modelData.listName : ""].filter(Boolean).join(" · ")
                                color: Todos.overdue(item.modelData) ? Theme.color.danger : Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                        Icon {
                            id: del
                            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                            visible: rowHover.hovered && !item.renaming
                            name: "x"
                            bold: true
                            size: 12
                            color: Theme.color.tertiary
                            MouseArea {
                                anchors { fill: parent; margins: -6 }
                                onClicked: root.remove(item.modelData)
                            }
                        }
                        HoverHandler { id: rowHover }
                    }
                }
            }
        }

        // Deleted: offer it back
        Row {
            id: undoRow
            visible: root.undo !== null
            spacing: 8
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Deleted “" + (root.undo?.item?.text ?? "") + "”"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Undo"
                color: Theme.accentInk
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        Todos.restore(root.undo.item);
                        root.undo = null;
                    }
                }
            }
        }
    }
}
