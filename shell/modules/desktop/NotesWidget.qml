import QtQuick
import qs.common

// A note on the desktop. Click to type; it's saved as you write.
Item {
    id: root

    property string size: "s"
    property bool preview: false
    property string widgetId: "" // its own text; "" = the first note (older layouts)
    readonly property string savedText: widgetId === "" ? UiState.data.desktopNote : (UiState.data.desktopNotes[widgetId] ?? "")
    function store(t) {
        if (widgetId === "")
            UiState.data.desktopNote = t;
        else
            UiState.data.desktopNotes = Object.assign({}, UiState.data.desktopNotes, { [widgetId]: t });
    }

    Row {
        id: header
        spacing: 8
        Rectangle {
            width: 26
            height: 26
            radius: 7
            color: Theme.accents.yellow
            Icon { anchors.centerIn: parent; name: "note-pencil"; fill: true; size: 15; color: "#3A2E00" }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Note"
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
    }

    Flickable {
        anchors { left: parent.left; right: parent.right; top: header.bottom; topMargin: 10; bottom: parent.bottom }
        contentHeight: edit.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        TextEdit {
            id: edit
            width: parent.width
            text: root.savedText
            wrapMode: TextEdit.Wrap
            color: Theme.color.label
            selectionColor: Qt.alpha(Theme.accent, 0.4)
            readOnly: root.preview
            font { family: Theme.font.family; pixelSize: Theme.font.body }
            onTextChanged: if (activeFocus) saveTimer.restart()
            Keys.onEscapePressed: focus = false
            Text {
                visible: edit.text === "" && !edit.activeFocus
                text: "Click to write a note"
                color: Theme.color.tertiary
                font: edit.font
            }
        }
    }
    Timer {
        id: saveTimer
        interval: 500
        onTriggered: root.store(edit.text)
    }
}
