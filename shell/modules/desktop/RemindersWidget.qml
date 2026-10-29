import QtQuick
import qs.common
import qs.services

// Reminders (the to-do list in Notification Center). Tick to complete.
// Soonest due first, with when (red once overdue). Large also has a field
// to add one ("call mum tomorrow 6pm" gets a due time).
Item {
    id: root

    property string size: "m"
    property bool preview: false
    readonly property var todos: Todos.list
    readonly property var open: Todos.sorted(todos.filter(t => !t.done))
    readonly property int rows: size === "l" ? 8 : size === "m" ? 3 : 2

    function toggle(t) {
        if (preview)
            return;
        Todos.update(t, { done: !t.done });
    }

    Row {
        id: header
        spacing: 8
        width: parent.width
        Rectangle {
            width: 26
            height: 26
            radius: 13
            color: Theme.accent
            Icon { anchors.centerIn: parent; name: "list-checks"; fill: true; size: 15; color: Theme.color.onAccent }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Reminders"
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
    }
    Text {
        anchors { right: parent.right; top: parent.top }
        text: root.open.length
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold; features: { "tnum": 1 } }
    }

    Column {
        anchors { left: parent.left; right: parent.right; top: header.bottom; topMargin: 12 }
        spacing: 8
        Text {
            visible: root.open.length === 0
            text: root.todos.length === 0 ? "No reminders" : "All done"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Repeater {
            model: root.open.slice(0, root.rows)
            Row {
                id: item
                required property var modelData
                spacing: 8
                width: parent.width
                Rectangle {
                    width: 16
                    height: 16
                    radius: 8
                    color: "transparent"
                    border.width: 1.5
                    border.color: Theme.color.tertiary
                    anchors.verticalCenter: parent.verticalCenter
                    MouseArea {
                        anchors { fill: parent; margins: -6 }
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.toggle(item.modelData)
                    }
                }
                Text {
                    id: todoText
                    width: parent.width - 24 - (due.visible ? due.implicitWidth + 6 : 0)
                    text: item.modelData.text
                    elide: Text.ElideRight
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Text {
                    id: due
                    visible: text !== "" && root.size !== "s"
                    anchors.baseline: todoText.baseline
                    text: Todos.dueLabel(item.modelData)
                    color: Todos.overdue(item.modelData) ? Theme.color.danger : Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                }
            }
        }
        Text {
            visible: root.open.length > root.rows
            text: (root.open.length - root.rows) + " more"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }

    // Large: add one
    Rectangle {
        visible: root.size === "l"
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 32
        radius: 10
        color: Theme.color.fill
        Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: "plus"; bold: true; size: 12; color: Theme.color.secondary }
        TextInput {
            id: input
            anchors { fill: parent; leftMargin: 30; rightMargin: 10 }
            verticalAlignment: TextInput.AlignVCenter
            color: Theme.color.label
            clip: true
            enabled: !root.preview
            font { family: Theme.font.family; pixelSize: Theme.font.small }
            onAccepted: {
                if (text.trim() === "")
                    return;
                if (Todos.add(text))
                    text = "";
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: input.text === "" && !input.activeFocus
                text: "New reminder"
                color: Theme.color.tertiary
                font: input.font
            }
        }
    }
}
