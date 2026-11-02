import QtQuick

// Single-line text field with placeholder. Commits on Enter or focus loss.
// Material: a filled field (4 px top corners, an underline that turns primary
// and thicker while typing).
Squircle {
    id: root

    property alias text: input.text
    property string placeholder
    property bool secret: false
    property bool commitOnBlur: true
    property string accessibleName: placeholder
    signal committed(string text)
    readonly property bool editing: input.activeFocus // being typed in
    function release() { // stop typing (focus goes back to the window)
        input.focus = false;
        root.Window.window?.contentItem?.forceActiveFocus();
    }
    // Put the cursor in the field (focusing the field itself wouldn't)
    function focusInput() {
        input.cursorPosition = input.length;
        input.forceActiveFocus();
    }

    implicitWidth: 220
    implicitHeight: Theme.material ? 36 : 28
    radius: Theme.radius.control - 2
    color: Theme.material ? "transparent" : Theme.color.fill
    border.width: Theme.material ? 0 : 1
    border.color: input.activeFocus ? Theme.accent : Theme.color.separator

    Loader {
        anchors.fill: parent
        active: Theme.material
        sourceComponent: Rectangle {
            topLeftRadius: 4
            topRightRadius: 4
            color: hover.hovered ? Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHighest
            HoverHandler {
                id: hover
            }
            Rectangle {
                anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                height: input.activeFocus ? 2 : 1
                color: input.activeFocus ? Theme.m3.primary : Theme.m3.onSurfaceVariant
            }
        }
    }

    TextInput {
        id: input
        anchors { fill: parent; leftMargin: Theme.material ? 12 : 10; rightMargin: Theme.material ? 12 : 10 }
        verticalAlignment: TextInput.AlignVCenter
        echoMode: root.secret ? TextInput.Password : TextInput.Normal
        color: Theme.color.label
        selectionColor: Theme.material ? Qt.alpha(Theme.m3.primary, 0.3) : Theme.accentAlpha(0.4)
        selectByMouse: true
        activeFocusOnTab: true // Tab moves on to the next control
        clip: true
        font { family: Theme.font.family; pixelSize: Theme.font.body }
        onAccepted: root.committed(text)
        Accessible.role: Accessible.EditableText
        Accessible.name: root.accessibleName
        Accessible.passwordEdit: root.secret
        onActiveFocusChanged: {
            if (activeFocus)
                return;
            cursorPosition = 0; // unfocused, a long value shows its start
            if (root.commitOnBlur)
                root.committed(text);
        }
        onTextChanged: if (!activeFocus) cursorPosition = 0

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: input.text === "" && !input.activeFocus
            text: root.placeholder
            color: Theme.color.tertiary
            font: input.font
        }
    }
}
