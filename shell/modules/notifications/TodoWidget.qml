import QtQuick
import qs.common
import qs.modules.controlcenter

// To-do list: add with Enter, tick to complete, remove with the × on hover.
// Saved in UiState.
Platter {
    id: root

    readonly property var todos: UiState.data.todos ?? []
    implicitHeight: column.implicitHeight + 24

    function save(list) {
        UiState.data.todos = list;
    }

    Column {
        id: column
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 12 }
        spacing: 6

        Rectangle {
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
                    root.save(root.todos.concat([{ text: text.trim(), done: false }]));
                    text = "";
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: input.text === ""
                    text: "Add a to-do"
                    color: Theme.color.tertiary
                    font: input.font
                }
            }
        }

        Text {
            visible: root.todos.length === 0
            width: parent.width
            topPadding: 8
            bottomPadding: 8
            horizontalAlignment: Text.AlignHCenter
            text: "Nothing to do. Nice."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }

        Repeater {
            model: root.todos
            Item {
                id: item
                required property var modelData
                required property int index
                width: column.width
                height: 30

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
                        onClicked: {
                            const l = root.todos.slice();
                            l[item.index] = { text: item.modelData.text, done: !item.modelData.done };
                            root.save(l);
                        }
                    }
                }
                Text {
                    anchors { left: box.right; leftMargin: 10; right: del.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                    text: item.modelData.text
                    elide: Text.ElideRight
                    color: item.modelData.done ? Theme.color.tertiary : Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.body; strikeout: item.modelData.done }
                }
                Icon {
                    id: del
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    visible: rowHover.hovered
                    name: "x"
                    bold: true
                    size: 12
                    color: Theme.color.tertiary
                    MouseArea {
                        anchors { fill: parent; margins: -6 }
                        onClicked: root.save(root.todos.filter((_, i) => i !== item.index))
                    }
                }
                HoverHandler { id: rowHover }
            }
        }
    }
}
