import QtQuick
import qs.common
import qs.services
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
    Squircle {
        id: shot
        // (a screenshot that's since been deleted leaves no empty box)
        visible: root.role === "user" && root.image !== "" && shotImage.status !== Image.Error
        anchors.right: parent.right
        width: 200
        height: 120
        radius: Theme.material ? 20 : 14
        color: Theme.color.fill
        clip: true
        Image {
            id: shotImage
            anchors.fill: parent
            source: root.role === "user" && root.image !== "" ? "file://" + root.image : ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(400, 240)
            asynchronous: true
        }
    }

    // User bubble
    Squircle {
        id: bubble
        visible: root.role === "user"
        anchors.right: parent.right
        y: shot.visible ? shot.height + (Theme.material ? 2 : 6) : 0
        width: Math.min(userText.implicitWidth + (Theme.material ? 32 : 28), root.width * 0.82)
        height: userText.implicitHeight + (Theme.material ? 20 : 18)
        radius: 18
        color: Theme.material ? "transparent" : Theme.accent
        // Material: a primary-container bubble, round but for the corner
        // nearest the speaker (bottom right), as Android's Messages draws it
        Rectangle {
            anchors.fill: parent
            visible: Theme.material
            radius: 20
            bottomRightRadius: 4
            topRightRadius: shot.visible ? 4 : 20
            color: Theme.m3.primaryContainer ?? "transparent"
        }
        TextEdit {
            id: userText
            x: Theme.material ? 16 : 14
            y: Theme.material ? 10 : 9
            width: parent.width - 2 * x
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            text: root.role === "user" ? root.text : ""
            color: Theme.material ? Theme.m3.onPrimaryContainer : Theme.color.onAccent
            selectionColor: Theme.material ? Qt.alpha(Theme.m3.primary, 0.3) : Qt.rgba(1, 1, 1, 0.35)
            selectedTextColor: Theme.material ? Theme.m3.onPrimaryContainer : Theme.color.onAccent
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
            // (Material: in a surface-container-high bubble, its corner
            // nearest the speaker, top left, small)
            padding: Theme.material ? 16 : 0
            topPadding: Theme.material ? 12 : 0
            bottomPadding: Theme.material ? 12 : 0
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            textFormat: TextEdit.MarkdownText
            text: root.role === "assistant" ? root.text : ""
            color: Theme.color.label
            selectionColor: Theme.material ? Qt.alpha(Theme.m3.primary, 0.3) : Theme.accentAlpha(0.4)
            font { family: Theme.font.family; pixelSize: Theme.font.body }
            onLinkActivated: link => Launch.openLink(link)
            Rectangle {
                z: -1
                anchors.fill: parent
                visible: Theme.material
                radius: 20
                topLeftRadius: 4
                color: Theme.m3.surfaceContainerHigh ?? "transparent"
            }
        }

        // Error / refusal
        Squircle {
            visible: root.status === "error" || root.status === "refused"
            width: parent.width
            height: errorRow.implicitHeight + (Theme.material ? 24 : 16)
            radius: Theme.material ? 16 : 12
            color: Theme.material ? Theme.m3.errorContainer : Qt.alpha(Theme.color.danger, 0.14)
            Row {
                id: errorRow
                x: Theme.material ? 16 : 10
                y: Theme.material ? 12 : 8
                width: parent.width - 2 * x
                spacing: 8
                Icon { name: "warning-circle"; fill: true; size: 15; color: Theme.material ? Theme.m3.onErrorContainer : Theme.color.danger }
                Text {
                    width: parent.width - 24
                    wrapMode: Text.Wrap
                    text: root.status === "refused" ? "The model declined to answer this. Try rephrasing your question." : root.text
                    color: Theme.material ? Theme.m3.onErrorContainer : Theme.color.label
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
                width: Theme.material ? 28 : 22
                height: width
                radius: width / 2
                color: Theme.material ? (copyMouse.pressed ? Qt.alpha(Theme.m3.onSurfaceVariant, 0.1) : copyMouse.containsMouse ? Qt.alpha(Theme.m3.onSurfaceVariant, 0.08) : "transparent")
                    : copyMouse.containsMouse ? Theme.color.fill : "transparent"
                Icon { id: copyIcon; anchors.centerIn: parent; name: "copy"; size: Theme.material ? 14 : 12; color: Theme.material ? Theme.m3.onSurfaceVariant : Theme.color.tertiary }
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
