import QtQuick
import qs.common
import qs.modules.controlcenter

// One chat row. User turns are accent bubbles on the right; assistant turns are
// Markdown with an optional collapsible reasoning summary; notes are captions.
Item {
    id: root

    required property string role
    required property string text
    required property string thinking
    required property string status
    required property string model
    required property string image

    property bool showThinking: false
    implicitHeight: role === "user" ? bubble.y + bubble.height : role === "note" ? noteText.implicitHeight : assistant.implicitHeight

    // Attached screenshot, above the bubble
    Rectangle {
        id: shot
        visible: root.role === "user" && root.image !== ""
        anchors.right: parent.right
        width: 200
        height: 120
        radius: 14
        color: Theme.color.fill
        clip: true
        Image {
            anchors.fill: parent
            source: shot.visible ? "file://" + root.image : ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(400, 240)
            asynchronous: true
        }
    }

    // User bubble
    Rectangle {
        id: bubble
        visible: root.role === "user"
        anchors.right: parent.right
        y: shot.visible ? shot.height + 6 : 0
        width: Math.min(userText.implicitWidth + 28, root.width * 0.82)
        height: userText.implicitHeight + 18
        radius: 18
        color: Theme.accent
        TextEdit {
            id: userText
            x: 14
            y: 9
            width: parent.width - 28
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            text: root.role === "user" ? root.text : ""
            color: Theme.color.onAccent
            selectionColor: Qt.rgba(1, 1, 1, 0.35)
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
    }

    // Note
    Text {
        id: noteText
        visible: root.role === "note"
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: root.role === "note" ? root.text : ""
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    // Assistant
    Column {
        id: assistant
        visible: root.role === "assistant"
        width: parent.width
        spacing: 6

        // Reasoning summary (collapsible)
        Item {
            visible: root.thinking !== "" || (root.status === "streaming" && root.text === "")
            width: thinkingRow.implicitWidth
            height: thinkingRow.implicitHeight

            Row {
                id: thinkingRow
                spacing: 6
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "brain"
                    size: 13
                    color: Theme.color.secondary
                    SequentialAnimation on opacity {
                        running: root.status === "streaming" && root.text === ""
                        loops: Animation.Infinite
                        NumberAnimation { to: 0.35; duration: 600 }
                        NumberAnimation { to: 1; duration: 600 }
                    }
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.status === "streaming" && root.text === "" ? "Thinking…" : "Thought process"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: root.thinking !== ""
                    name: root.showThinking ? "caret-up" : "caret-down"
                    bold: true
                    size: 10
                    color: Theme.color.secondary
                }
            }
            MouseArea {
                anchors.fill: parent
                enabled: root.thinking !== ""
                onClicked: root.showThinking = !root.showThinking
            }
        }
        Text {
            visible: root.showThinking && root.thinking !== ""
            width: parent.width
            leftPadding: 10
            wrapMode: Text.Wrap
            textFormat: Text.MarkdownText
            text: root.thinking
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
            Rectangle { width: 2; height: parent.height; radius: 1; color: Theme.color.separator }
        }

        TextEdit {
            visible: text !== "" && root.status !== "error"
            width: parent.width
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            textFormat: TextEdit.MarkdownText
            text: root.role === "assistant" ? root.text : ""
            color: Theme.color.label
            selectionColor: Qt.alpha(Theme.accent, 0.4)
            font { family: Theme.font.family; pixelSize: Theme.font.body }
            onLinkActivated: link => Qt.openUrlExternally(link)
        }

        // Error / refusal
        Rectangle {
            visible: root.status === "error" || root.status === "refused"
            width: parent.width
            height: errorRow.implicitHeight + 16
            radius: 12
            color: Qt.alpha(Theme.color.danger, 0.14)
            Row {
                id: errorRow
                x: 10
                y: 8
                width: parent.width - 20
                spacing: 8
                Icon { name: "warning-circle"; fill: true; size: 15; color: Theme.color.danger }
                Text {
                    width: parent.width - 24
                    wrapMode: Text.Wrap
                    text: root.status === "refused" ? "The model declined to answer this. Try rephrasing your question." : root.text
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
            }
        }

        Row {
            visible: root.status === "done" || root.status === "stopped"
            spacing: 8
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.model + (root.status === "stopped" ? " · stopped" : "")
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
            Rectangle {
                width: 22
                height: 22
                radius: 11
                color: copyMouse.containsMouse ? Theme.color.fill : "transparent"
                Icon { id: copyIcon; anchors.centerIn: parent; name: "copy"; size: 12; color: Theme.color.tertiary }
                MouseArea {
                    id: copyMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                        System.run(["wl-copy", root.text]);
                        copyIcon.name = "check";
                        copyReset.restart();
                    }
                }
                Timer { id: copyReset; interval: 1500; onTriggered: copyIcon.name = "copy" }
            }
        }
    }
}
