import QtQuick
import QtQuick.Controls.Basic
import qs.common
import qs.modules.controlcenter
import qs.services

// AI chat: model picker, conversation, composer. Enter sends, Shift+Enter
// adds a line. /clear, /model <id>, /key <key> work in the composer.
Item {
    id: root

    property bool active: false
    onActiveChanged: if (active) Qt.callLater(() => composer.forceActiveFocus())

    // Header: model picker + new chat
    Item {
        id: header
        width: parent.width
        height: 30

        Rectangle {
            id: picker
            height: 30
            width: pickerRow.implicitWidth + 24
            radius: 15
            color: pickerMouse.containsMouse || modelMenu.visible ? Theme.color.fillHover : Theme.color.fill
            Row {
                id: pickerRow
                anchors.centerIn: parent
                spacing: 6
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "sparkle"; fill: true; size: 14; color: Theme.accent }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: Ai.model.name
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "caret-down"; bold: true; size: 11; color: Theme.color.secondary }
            }
            MouseArea {
                id: pickerMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: modelMenu.visible = !modelMenu.visible
            }
        }

        Rectangle {
            anchors.right: parent.right
            width: 30
            height: 30
            radius: 15
            visible: Ai.chat.count > 0
            color: newMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
            Icon { anchors.centerIn: parent; name: "note-pencil"; size: 15 }
            MouseArea {
                id: newMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: Ai.clear()
            }
        }
    }

    // Empty state
    Column {
        anchors.centerIn: list
        width: parent.width - 40
        visible: Ai.chat.count === 0
        spacing: 8

        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "sparkle"; fill: true; size: 34; color: Theme.color.tertiary }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: Ai.needsKey ? "Connect " + Ai.providerName : "Ask " + Ai.model.name
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: Ai.needsKey ? "Paste your " + Ai.providerName + " API key. It's stored in your system keyring." : "Questions, commands, writing help. Type /model to switch models."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }

        // Key entry
        Row {
            visible: Ai.needsKey
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 8
            topPadding: 8
            Rectangle {
                width: 220
                height: 32
                radius: Theme.radius.control
                color: Theme.color.fill
                border.width: 1
                border.color: keyField.activeFocus ? Theme.accent : Theme.color.separator
                TextInput {
                    id: keyField
                    anchors { fill: parent; leftMargin: 10; rightMargin: 10 }
                    verticalAlignment: TextInput.AlignVCenter
                    echoMode: TextInput.Password
                    color: Theme.color.label
                    clip: true
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                    onAccepted: saveKey.save()
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: keyField.text === ""
                        text: "API key"
                        color: Theme.color.tertiary
                        font: keyField.font
                    }
                }
            }
            Rectangle {
                id: saveKey
                function save() {
                    if (keyField.text.trim() === "")
                        return;
                    AiKeys.set(Ai.model.provider, keyField.text);
                    keyField.text = "";
                    composer.forceActiveFocus();
                }
                width: 64
                height: 32
                radius: Theme.radius.control
                color: keyField.text.trim() !== "" ? Theme.accent : Theme.color.toggleOff
                Text {
                    anchors.centerIn: parent
                    text: "Save key"
                    color: keyField.text.trim() !== "" ? Theme.color.onAccent : Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea { anchors.fill: parent; onClicked: saveKey.save() }
            }
        }
    }

    ListView {
        id: list
        anchors { top: header.bottom; topMargin: 12; left: parent.left; right: parent.right; bottom: attachmentChip.visible ? attachmentChip.top : composerBox.top; bottomMargin: 10 }
        clip: true
        spacing: 14
        model: Ai.chat
        boundsBehavior: Flickable.StopAtBounds
        onCountChanged: Qt.callLater(positionViewAtEnd)
        onContentHeightChanged: if (Ai.busy) positionViewAtEnd()

        delegate: ChatMessage {
            width: list.width
        }
    }

    // Screenshot waiting to be sent with the next message
    Rectangle {
        id: attachmentChip
        visible: Ai.attachment !== null
        anchors { left: parent.left; bottom: composerBox.top; bottomMargin: 8 }
        width: chipRow.implicitWidth + 12
        height: 48
        radius: Theme.radius.control
        color: Theme.color.fill
        border.width: 1
        border.color: Theme.color.separator
        Row {
            id: chipRow
            x: 6
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10
            Image {
                width: 56
                height: 36
                source: Ai.attachment ? "file://" + Ai.attachment.path : ""
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(112, 72)
                asynchronous: true
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Screenshot"
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                icon: "x"
                iconSize: 13
                onClicked: Ai.attach(null)
            }
        }
    }

    // Composer
    Rectangle {
        id: composerBox
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: Math.min(150, composer.contentHeight + 22)
        radius: 18
        color: Theme.color.fill
        border.width: 1
        border.color: composer.activeFocus ? Theme.accentAlpha(0.6) : Theme.color.separator

        Flickable {
            id: scroller
            anchors { left: parent.left; right: sendButton.left; top: parent.top; bottom: parent.bottom; leftMargin: 14; rightMargin: 6; topMargin: 11; bottomMargin: 11 }
            contentHeight: composer.contentHeight
            clip: true
            TextArea.flickable: TextArea {
                id: composer
                padding: 0
                wrapMode: TextEdit.Wrap
                color: Theme.color.label
                placeholderText: Ai.needsKey ? "Add a key to start" : Ai.attachment ? "Ask about this screenshot" : "Message " + Ai.model.name
                placeholderTextColor: Theme.color.tertiary
                background: null
                font { family: Theme.font.family; pixelSize: Theme.font.body }
                Keys.onReturnPressed: e => {
                    if (e.modifiers & Qt.ShiftModifier) {
                        e.accepted = false;
                        return;
                    }
                    Ai.submit(text);
                    text = "";
                }
            }
        }

        Rectangle {
            id: sendButton
            anchors { right: parent.right; rightMargin: 6; bottom: parent.bottom; bottomMargin: 6 }
            width: 28
            height: 28
            radius: 14
            readonly property bool canSend: composer.text.trim() !== "" || Ai.attachment !== null
            color: Ai.busy || canSend ? Theme.accent : Theme.color.toggleOff
            Icon {
                anchors.centerIn: parent
                name: Ai.busy ? "stop" : "arrow-up"
                bold: !Ai.busy
                fill: Ai.busy
                size: 14
                color: Ai.busy || sendButton.canSend ? Theme.color.onAccent : Theme.color.tertiary
            }
            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (Ai.busy) {
                        Ai.stop();
                    } else if (sendButton.canSend) {
                        Ai.submit(composer.text);
                        composer.text = "";
                    }
                }
            }
        }
    }

    // Model menu
    Rectangle {
        id: modelMenu
        visible: false
        anchors { top: header.bottom; topMargin: 6; left: parent.left }
        width: 240
        height: menuColumn.implicitHeight + 10
        radius: 14
        color: Theme.dark ? Qt.rgba(0.14, 0.14, 0.16, 0.97) : Qt.rgba(0.98, 0.98, 0.99, 0.98)
        border.width: 1
        border.color: Theme.color.separator
        z: 10

        Column {
            id: menuColumn
            anchors { fill: parent; margins: 5 }
            Repeater {
                model: Ai.models
                Rectangle {
                    required property var modelData
                    width: menuColumn.width
                    height: 30
                    radius: 8
                    color: itemMouse.containsMouse ? Theme.accent : "transparent"
                    Icon {
                        x: 8
                        anchors.verticalCenter: parent.verticalCenter
                        visible: parent.modelData.id === Ai.model.id
                        name: "check"
                        bold: true
                        size: 12
                        color: itemMouse.containsMouse ? Theme.color.onAccent : Theme.color.label
                    }
                    Text {
                        x: 26
                        anchors.verticalCenter: parent.verticalCenter
                        text: parent.modelData.name
                        color: itemMouse.containsMouse ? Theme.color.onAccent : Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                    MouseArea {
                        id: itemMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            Ai.setModel(parent.modelData.id);
                            modelMenu.visible = false;
                        }
                    }
                }
            }
        }
    }
}
