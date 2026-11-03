import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
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
        height: Theme.material ? 32 : 30

        // (Material: a chip, 12 px corners, turning a pill in secondary
        // container while its menu is open)
        Rectangle {
            id: picker
            height: header.height
            width: pickerRow.implicitWidth + 24
            radius: Theme.material ? (modelMenu.visible ? height / 2 : 12) : 15
            Behavior on radius { enabled: Theme.material; SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
            color: Theme.material ? (modelMenu.visible ? Theme.m3.secondaryContainer : Qt.tint(Theme.m3.surfaceContainerHigh, Qt.alpha(Theme.m3.onSurface, pickerMouse.pressed ? 0.1 : pickerMouse.containsMouse ? 0.08 : 0)))
                : pickerMouse.containsMouse || modelMenu.visible ? Theme.color.fillHover : Theme.color.fill
            Row {
                id: pickerRow
                anchors.centerIn: parent
                spacing: 6
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "sparkle"; fill: true; size: 14; color: Theme.accent }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: Ai.model.name
                    color: Theme.material && modelMenu.visible ? Theme.m3.onSecondaryContainer : Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
                }
                Icon { anchors.verticalCenter: parent.verticalCenter; name: Theme.material && modelMenu.visible ? "caret-up" : "caret-down"; bold: true; size: 11; color: Theme.material && modelMenu.visible ? Theme.m3.onSecondaryContainer : Theme.color.secondary }
            }
            MouseArea {
                id: pickerMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: Ai.configured ? modelMenu.visible = !modelMenu.visible : Panels.openSettings("ai")
            }
        }

        // (Material: a tonal icon button)
        Rectangle {
            anchors.right: parent.right
            width: header.height
            height: header.height
            transform: pressFx1.effects
            ElasticControl { id: pressFx1; pressed: newMouse.pressed }
            radius: Theme.material && newMouse.pressed ? 8 : height / 2
            Behavior on radius { enabled: Theme.material; SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
            visible: Ai.chat.count > 0
            color: Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, newMouse.pressed ? 0.1 : newMouse.containsMouse ? 0.08 : 0))
                : newMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
            Icon { anchors.centerIn: parent; name: "note-pencil"; size: 15; color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label }
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

        Icon { anchors.horizontalCenter: parent.horizontalCenter; visible: !Theme.material; name: "sparkle"; fill: true; size: 34; color: Theme.color.tertiary }
        // Material: the sparkle on a cookie, Android's hero shape
        Loader {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: Theme.material
            active: Theme.material
            sourceComponent: Item {
                implicitWidth: 72
                implicitHeight: 80 // (a little room below the shape)
                MaterialShape {
                    width: 72
                    height: 72
                    shape: "cookie9"
                    color: Theme.m3.primaryContainer
                    Icon { anchors.centerIn: parent; name: "sparkle"; fill: true; size: 30; color: Theme.m3.onPrimaryContainer }
                }
            }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: !Ai.configured ? "Add your AI model" : Ai.needsKey ? "Connect " + Ai.providerName : "Ask " + Ai.model.name
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.large : Theme.font.title; weight: Theme.material ? Font.Normal : Font.DemiBold }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: !Ai.configured ? "Choose your provider and model in Settings → AI. Click Add a model above to get started." : Ai.needsKey ? "Add your " + Ai.providerName + " " + Ai.credentialLabel.toLowerCase() + " in Settings → AI, or paste it here." : "Questions, commands, writing help. Type /model to switch models."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }

        // Key entry
        Row {
            visible: Ai.needsKey
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 8
            topPadding: 8
            Squircle {
                width: 220
                height: Theme.material ? 40 : 32
                radius: Theme.material ? height / 2 : Theme.radius.control
                color: Theme.material ? Theme.m3.surfaceContainerHigh : Theme.color.fill
                border.width: Theme.material ? (keyField.activeFocus ? 2 : 0) : 1
                border.color: keyField.activeFocus ? Theme.accent : Theme.color.separator
                TextInput {
                    id: keyField
                    anchors { fill: parent; leftMargin: Theme.material ? 16 : 10; rightMargin: Theme.material ? 16 : 10 }
                    verticalAlignment: TextInput.AlignVCenter
                    echoMode: TextInput.Password
                    color: Theme.color.label
                    clip: true
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                    onAccepted: saveKey.save()
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: keyField.text === ""
                        text: Ai.credentialLabel
                        color: Theme.color.tertiary
                        font: keyField.font
                    }
                }
            }
            Squircle {
                id: saveKey
                function save() {
                    if (keyField.text.trim() === "")
                        return;
                    Ai.saveCredential(Ai.model, keyField.text);
                    keyField.text = "";
                    composer.forceActiveFocus();
                }
                // (Material: a filled pill; disabled, 12% and 38% of onSurface)
                width: Theme.material ? saveLabel.implicitWidth + 32 : 64
                height: Theme.material ? 40 : 32
                radius: Theme.material ? height / 2 : Theme.radius.control
                color: keyField.text.trim() !== "" ? Theme.accent : Theme.material ? Qt.alpha(Theme.m3.onSurface, 0.12) : Theme.color.toggleOff
                Text {
                    id: saveLabel
                    anchors.centerIn: parent
                    text: "Save key"
                    color: keyField.text.trim() !== "" ? Theme.color.onAccent : Theme.material ? Qt.alpha(Theme.m3.onSurface, 0.38) : Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.body : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
                }
                MouseArea { anchors.fill: parent; onClicked: saveKey.save() }
            }
        }
    }

    ListView {
        id: list

        Momentum {}
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
    Squircle {
        id: attachmentChip
        visible: Ai.attachment !== null
        anchors { left: parent.left; bottom: composerBox.top; bottomMargin: 8 }
        width: chipRow.implicitWidth + 12
        height: 48
        radius: Theme.material ? 16 : Theme.radius.control
        color: Theme.material ? Theme.m3.surfaceContainerHigh : Theme.color.fill
        border.width: Theme.material ? 0 : 1
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
    Squircle {
        id: composerBox
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        // (Material: a pill in surface container high, no outline)
        height: Math.min(150, composer.contentHeight + (Theme.material ? 32 : 22))
        radius: Theme.material ? Math.min(height / 2, 28) : 18
        color: Theme.material ? Theme.m3.surfaceContainerHigh : Theme.color.fill
        border.width: Theme.material ? 0 : 1
        border.color: composer.activeFocus ? Theme.accentAlpha(0.6) : Theme.color.separator

        Flickable {
            id: scroller

            Momentum {}
            anchors { left: parent.left; right: sendButton.left; top: parent.top; bottom: parent.bottom; leftMargin: Theme.material ? 20 : 14; rightMargin: 6; topMargin: Theme.material ? 16 : 11; bottomMargin: Theme.material ? 16 : 11 }
            contentHeight: composer.contentHeight
            clip: true
            TextArea.flickable: TextArea {
                id: composer
                padding: 0
                wrapMode: TextEdit.Wrap
                color: Theme.color.label
                placeholderText: !Ai.configured ? "Add a model in Settings → AI" : Ai.needsKey ? "Add a credential to start" : Ai.attachment ? "Ask about this screenshot" : "Message " + Ai.model.name
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
            // (Material: a filled primary circle, squaring a little while
            // pressed; disabled, 12% and 38% of onSurface)
            width: Theme.material ? 36 : 28
            height: width
            transform: pressFx2.effects
            ElasticControl { id: pressFx2; pressed: sendMouse.pressed }
            radius: Theme.material && sendMouse.pressed ? 10 : width / 2
            Behavior on radius { enabled: Theme.material; SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
            readonly property bool canSend: composer.text.trim() !== "" || Ai.attachment !== null
            color: Ai.busy || canSend ? (Theme.material ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, sendMouse.pressed ? 0.1 : sendMouse.containsMouse ? 0.08 : 0)) : Theme.accent)
                : Theme.material ? Qt.alpha(Theme.m3.onSurface, 0.12) : Theme.color.toggleOff
            Icon {
                anchors.centerIn: parent
                name: Ai.busy ? "stop" : "arrow-up"
                bold: !Ai.busy
                fill: Ai.busy
                size: Theme.material ? 18 : 14
                color: Ai.busy || sendButton.canSend ? Theme.color.onAccent : Theme.material ? Qt.alpha(Theme.m3.onSurface, 0.38) : Theme.color.tertiary
            }
            MouseArea {
                id: sendMouse
                anchors.fill: parent
                hoverEnabled: Theme.material
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
    // (Material: a menu, surface container with 16 px corners and a soft shadow)
    Loader {
        active: Theme.material && modelMenu.visible
        anchors.fill: modelMenu
        z: 9
        sourceComponent: RectangularShadow {
            radius: 16
            offset.y: 2
            blur: 12
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.4 : 0.16)
        }
    }
    Squircle {
        id: modelMenu
        visible: false
        anchors { top: header.bottom; topMargin: 6; left: parent.left }
        width: 280
        height: Math.min(340, menuColumn.implicitHeight + 10)
        radius: Theme.material ? 16 : 14
        color: Theme.material ? Theme.m3.surfaceContainer : Theme.dark ? Qt.rgba(0.14, 0.14, 0.16, 0.97) : Qt.rgba(0.98, 0.98, 0.99, 0.98)
        border.width: Theme.material ? 0 : 1
        border.color: Theme.color.separator
        z: 10

        Flickable {

            Momentum {}
            anchors { fill: parent; margins: 5 }
            clip: true
            contentHeight: menuColumn.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: menuColumn
                width: parent.width
                Repeater {
                    model: Ai.models
                    Squircle {
                        id: modelItem
                        required property var modelData
                        // (Material: 44 px items, the chosen one in tertiary container)
                        readonly property bool chosen: modelData.key === Ai.model.key
                        width: menuColumn.width
                        height: Theme.material ? 44 : 30
                        radius: Theme.material ? 12 : 8
                        color: Theme.material && chosen ? Qt.tint(Theme.m3.tertiaryContainer, Qt.alpha(Theme.m3.onTertiaryContainer, itemMouse.containsMouse ? 0.08 : 0))
                            : itemMouse.containsMouse ? Theme.color.highlight : "transparent"
                        Icon {
                            x: Theme.material ? 12 : 8
                            anchors.verticalCenter: parent.verticalCenter
                            visible: parent.modelData.key === Ai.model.key
                            name: "check"
                            bold: true
                            size: Theme.material ? 16 : 12
                            color: Theme.material && modelItem.chosen ? Theme.m3.onTertiaryContainer : itemMouse.containsMouse ? Theme.color.onHighlight : Theme.color.label
                        }
                        Text {
                            x: Theme.material ? 40 : 26
                            anchors.verticalCenter: parent.verticalCenter
                            text: parent.modelData.name + " · " + Ai.nameForProvider(parent.modelData.provider)
                            width: parent.width - x - 6
                            elide: Text.ElideRight
                            color: Theme.material && modelItem.chosen ? Theme.m3.onTertiaryContainer : itemMouse.containsMouse ? Theme.color.onHighlight : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                        MouseArea {
                            id: itemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                Ai.setModel(parent.modelData.key);
                                modelMenu.visible = false;
                            }
                        }
                    }
                }
                Squircle {
                    width: menuColumn.width
                    height: Theme.material ? 44 : 30
                    radius: Theme.material ? 12 : 8
                    color: manageMouse.containsMouse ? (Theme.material ? Theme.color.highlight : Theme.color.fillHover) : "transparent"
                    Text {
                        anchors { left: parent.left; leftMargin: Theme.material ? 40 : 12; verticalCenter: parent.verticalCenter }
                        text: "Manage models…"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                    MouseArea {
                        id: manageMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: { modelMenu.visible = false; Panels.openSettings("ai"); }
                    }
                }
            }
        }
    }
}
