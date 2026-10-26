import QtQuick

// Single-line text field with placeholder. Commits on Enter or focus loss.
Rectangle {
    id: root

    property alias text: input.text
    property string placeholder
    property bool secret: false
    property bool commitOnBlur: true
    signal committed(string text)
    // Put the cursor in the field (focusing the field itself wouldn't)
    function focusInput() {
        input.forceActiveFocus();
    }

    implicitWidth: 220
    implicitHeight: 28
    radius: Theme.radius.control - 2
    color: Theme.color.fill
    border.width: 1
    border.color: input.activeFocus ? Theme.accent : Theme.color.separator

    TextInput {
        id: input
        anchors { fill: parent; leftMargin: 10; rightMargin: 10 }
        verticalAlignment: TextInput.AlignVCenter
        echoMode: root.secret ? TextInput.Password : TextInput.Normal
        color: Theme.color.label
        selectionColor: Qt.alpha(Theme.accent, 0.4)
        selectByMouse: true
        clip: true
        font { family: Theme.font.family; pixelSize: Theme.font.body }
        onAccepted: root.committed(text)
        onActiveFocusChanged: if (!activeFocus && root.commitOnBlur) root.committed(text)

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: input.text === "" && !input.activeFocus
            text: root.placeholder
            color: Theme.color.tertiary
            font: input.font
        }
    }
}
