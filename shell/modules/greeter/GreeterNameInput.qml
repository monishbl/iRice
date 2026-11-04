import QtQuick

// "Other user…": the username, typed in. Enter or Tab goes on to the
// password, Up and Down to the other people. Each style draws the box
// around it (this fills its height) and sets its colours and font.
TextInput {
    id: root
    required property var win
    property color placeholderColor
    property int placeholderSize: font.pixelSize
    height: parent?.height ?? 0
    verticalAlignment: TextInput.AlignVCenter
    text: win.auth.otherName
    onTextEdited: win.auth.otherName = text
    readOnly: win.auth.busy
    clip: true
    inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
    onAccepted: win.toPassword()
    Keys.onTabPressed: win.toPassword()
    Keys.onUpPressed: win.nextUser(-1)
    Keys.onDownPressed: win.nextUser(1)
    Keys.onEscapePressed: win.cancel()
    Accessible.name: "Username"
    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.text === ""
        text: "Username"
        color: root.placeholderColor
        font { family: root.font.family; pixelSize: root.placeholderSize }
    }
}
