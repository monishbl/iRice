import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import qs.common
import qs.modules.controlcenter
import qs.services

// AI chat: model picker, conversation, composer. Enter sends, Shift+Enter
// adds a line. /new (or /clear), /model <id>, /key <key> work in the composer.
// Pictures (up to 4 a message): the + button, a screenshot, Ctrl+V or a drop.
// The clock beside the pencil shows past chats (ChatHistory) in its place.
Item {
    id: root

    property bool active: false
    property var panelScreen: null
    onActiveChanged: {
        if (active)
            Qt.callLater(() => composer.forceActiveFocus());
        else
            historyOpen = false;
    }

    // Past chats, in place of the chat (back arrow or Escape returns)
    property bool historyOpen: false
    readonly property bool canShowHistory: Ai.keepHistory && (Ai.sessions.length > 0 || Ai.pendingDelete !== null)
    function showHistory(on) {
        modelMenu.visible = false;
        shotMenu.visible = false;
        historyOpen = on;
        if (on)
            pastChats.enter();
        else
            Qt.callLater(() => composer.forceActiveFocus());
    }
    Connections {
        target: Ai
        function onWantChat() { root.showHistory(false); }
        function onKeepHistoryChanged() { if (!Ai.keepHistory) root.showHistory(false); }
    }

    // A round header button: tonal in Material, the fill in Glass
    component HeaderButton: Rectangle {
        id: hb
        property string icon
        property string label
        signal clicked
        width: header.height
        height: header.height
        transform: Theme.material ? hbFx.effects : []
        ElasticControl { id: hbFx; pressed: hbMouse.pressed; enabled: Theme.material }
        radius: Theme.material && hbMouse.pressed ? 8 : height / 2
        Behavior on radius { enabled: Theme.material; MaterialSpring { speed: "fast"; epsilon: 0.1 } }
        color: Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, hbMouse.pressed ? 0.1 : hbMouse.containsMouse ? 0.08 : 0))
            : hbMouse.pressed ? Theme.color.fillPressed : hbMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
        activeFocusOnTab: visible
        Keys.onPressed: e => {
            if (!e.isAutoRepeat && (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter)) {
                hb.clicked();
                e.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: label
        Accessible.onPressAction: hb.clicked()
        FocusRing { radius: hb.radius + 4 }
        Icon { anchors.centerIn: parent; name: hb.icon; size: 15; color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label }
        MouseArea {
            id: hbMouse
            anchors { fill: parent; margins: -6 } // (44 px to hit)
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: hb.clicked()
        }
    }

    // Header: model picker, past chats and new chat (in past chats: back)
    Item {
        id: header
        width: parent.width
        height: Theme.material ? 32 : 30

        // (Material: a chip, 12 px corners, turning a pill in secondary
        // container while its menu is open)
        Rectangle {
            id: picker
            visible: !root.historyOpen
            height: header.height
            width: pickerRow.implicitWidth + 24
            radius: Theme.material ? (modelMenu.visible ? height / 2 : 12) : 15
            Behavior on radius { enabled: Theme.material; MaterialSpring { speed: "fast"; epsilon: 0.1 } }
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

        // In past chats: back to the chat, and what this is
        IconButton {
            id: backButton
            visible: root.historyOpen
            anchors.verticalCenter: parent.verticalCenter
            x: -4
            width: Theme.material ? 40 : 30
            height: width
            icon: "arrow-left"
            iconSize: Theme.material ? 20 : 16
            iconColor: Theme.color.label
            tooltip: "Back to chat"
            onClicked: root.showHistory(false)
        }
        Text {
            visible: root.historyOpen
            anchors { left: backButton.right; leftMargin: Theme.material ? 8 : 4; verticalCenter: parent.verticalCenter }
            text: "Chats"
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Theme.material ? Font.Normal : Font.DemiBold }
            Accessible.role: Accessible.Heading
            Accessible.name: "Past chats"
        }

        Row {
            anchors.right: parent.right
            spacing: 8
            // (Material: tonal icon buttons)
            HeaderButton {
                visible: root.canShowHistory && !root.historyOpen
                icon: "clock-counter-clockwise"
                label: "Show past chats"
                onClicked: root.showHistory(true)
            }
            HeaderButton {
                visible: Ai.chat.count > 0
                icon: "note-pencil"
                label: "Start a new chat"
                onClicked: {
                    Ai.newChat();
                    root.showHistory(false);
                }
            }
        }
    }

    // Empty state
    Column {
        anchors.centerIn: list
        width: parent.width - 40
        visible: Ai.chat.count === 0 && !root.historyOpen
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
        visible: !root.historyOpen

        Momentum {}
        anchors { top: header.bottom; topMargin: 12; left: parent.left; right: parent.right; bottom: blind.visible ? blind.top : pictures.visible ? pictures.top : composerBox.top; bottomMargin: 10 }
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

    // Pictures waiting, but the model can't read them: which of theirs can,
    // a tap away (the pictures stay: Send asks once they've switched), or
    // the web for a circled one
    Column {
        id: blind
        readonly property var canSee: Ai.visionModels.find(m => Ai.hasCredential(m)) ?? Ai.visionModels[0] ?? null
        readonly property string lensPicture: Ai.attachments.find(a => a.lens)?.path ?? ""
        visible: Ai.attachments.length > 0 && (!Ai.configured || !Ai.model.vision) && !root.historyOpen
        anchors { left: parent.left; right: parent.right; bottom: pictures.top; bottomMargin: 10 }
        spacing: 8
        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: !Ai.configured ? "Add an AI model that reads pictures to ask about this." : Ai.model.name + " can't read pictures. " + (blind.canSee ? blind.canSee.name + " can." : "Add one that can (Claude, Gemini or another vision model) in Settings → AI.")
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Flow {
            width: parent.width
            spacing: 8
            ChatButton {
                id: switchButton
                primary: true
                icon: blind.canSee ? "swap" : "plus"
                label: blind.canSee ? "Switch to " + blind.canSee.name : "Add a model"
                onClicked: {
                    if (!blind.canSee)
                        return Panels.openSettings("ai");
                    Ai.setModel(blind.canSee.key);
                    composer.forceActiveFocus();
                }
            }
            LensButton {
                visible: blind.lensPicture !== ""
                width: Math.max(160, blind.width - switchButton.width - 8) // (beside it, its note wrapping under it)
                picture: blind.lensPicture
            }
        }
    }

    // Pictures waiting to go with the next message, each with its remove button
    Row {
        id: pictures
        visible: Ai.attachments.length > 0 && !root.historyOpen
        anchors { left: parent.left; bottom: composerBox.top; bottomMargin: 8 }
        spacing: 8
        Repeater {
            model: Ai.attachments
            Item {
                id: pic
                required property var modelData
                required property int index
                width: 64
                height: 64
                Squircle {
                    anchors.fill: parent
                    radius: Theme.material ? 16 : 12
                    color: Theme.color.fill
                    clip: true
                    Image {
                        anchors.fill: parent
                        source: "file://" + pic.modelData.path
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(128, 128)
                        asynchronous: true
                    }
                }
                Rectangle {
                    anchors { right: parent.right; top: parent.top; margins: -6 }
                    width: 22
                    height: 22
                    radius: 11
                    color: Theme.material ? Theme.m3.inverseSurface : Qt.rgba(0.1, 0.1, 0.12, 0.85)
                    Icon { anchors.centerIn: parent; name: "x"; bold: true; size: 11; color: Theme.material ? Theme.m3.inverseOnSurface : "white" }
                    MouseArea {
                        anchors { fill: parent; margins: -11 } // (44 px to hit)
                        cursorShape: Qt.PointingHandCursor
                        Accessible.role: Accessible.Button
                        Accessible.name: "Remove picture " + (pic.index + 1)
                        onClicked: Ai.detach(pic.index)
                    }
                }
            }
        }
    }

    // Files dropped on the chat: the pictures among them
    DropArea {
        anchors.fill: parent
        enabled: !root.historyOpen
        keys: ["text/uri-list"]
        onEntered: drag => drag.accepted = drag.hasUrls
        onDropped: drop => {
            const files = drop.urls.map(u => String(u)).filter(u => u.startsWith("file://")).map(u => decodeURIComponent(u.slice(7)));
            if (files.length)
                Ai.attachFiles(files);
            drop.accept(Qt.CopyAction);
        }
        Rectangle {
            anchors.fill: parent
            visible: parent.containsDrag
            radius: Theme.material ? 24 : 16
            color: Theme.material ? Qt.alpha(Theme.m3.primary, 0.08) : Theme.accentAlpha(0.08)
            border.width: 2
            border.color: Theme.material ? Theme.m3.primary : Theme.accent
            Text {
                anchors.centerIn: parent
                text: "Drop pictures to add them"
                color: Theme.material ? Theme.m3.primary : Theme.accentInk
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
            }
        }
    }

    // Composer
    Squircle {
        id: composerBox
        visible: !root.historyOpen
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        // (Material: a pill in surface container high, no outline)
        height: Math.min(150, composer.contentHeight + (Theme.material ? 32 : 22))
        radius: Theme.material ? Math.min(height / 2, 28) : 18
        color: Theme.material ? Theme.m3.surfaceContainerHigh : Theme.color.fill
        border.width: Theme.material ? 0 : 1
        border.color: composer.activeFocus ? Theme.accentAlpha(0.6) : Theme.color.separator

        // Add pictures, take a screenshot (its menu: area or window)
        Row {
            id: tools
            anchors { left: parent.left; leftMargin: 6; bottom: parent.bottom; bottomMargin: Theme.material ? 6 : 3 }
            spacing: 0
            IconButton {
                icon: "plus"
                tooltip: "Add pictures"
                width: Theme.material ? 36 : 28
                height: width
                enabled: Ai.attachments.length < Ai.maxImages && !FilePicker.busy
                onClicked: FilePicker.pick({ multiple: true, title: "Add pictures", start: "~/Pictures", filter: "Pictures|*.png *.jpg *.jpeg *.webp *.gif *.heic *.avif *.bmp *.tif *.tiff" }, paths => {
                    if (paths && paths.length)
                        Ai.attachFiles(paths);
                })
            }
            IconButton {
                id: shotButton
                icon: "camera"
                tooltip: "Take a screenshot (right-click: area or window)"
                accessibleName: "Take a screenshot"
                width: Theme.material ? 36 : 28
                height: width
                enabled: Ai.attachments.length < Ai.maxImages
                onClicked: ScreenTools.askScreen()
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: shotMenu.visible = !shotMenu.visible
                }
                TapHandler {
                    onLongPressed: shotMenu.visible = true
                }
            }
            IconButton {
                icon: "caret-up"
                iconSize: 10
                tooltip: "More screenshot options"
                width: 18
                height: Theme.material ? 36 : 28
                enabled: Ai.attachments.length < Ai.maxImages
                onClicked: shotMenu.visible = !shotMenu.visible
            }
            // Circle to search: circle anything on the screen, and it's asked about
            IconButton {
                icon: "lasso"
                tooltip: "Circle to search"
                width: Theme.material ? 36 : 28
                height: width
                enabled: Ai.attachments.length < Ai.maxImages
                onClicked: ScreenTools.select("circle")
            }
        }

        Flickable {
            id: scroller

            Momentum {}
            anchors { left: tools.right; right: sendButton.left; top: parent.top; bottom: parent.bottom; leftMargin: 6; rightMargin: 6; topMargin: Theme.material ? 16 : 11; bottomMargin: Theme.material ? 16 : 11 }
            contentHeight: composer.contentHeight
            clip: true
            TextArea.flickable: TextArea {
                id: composer
                padding: 0
                wrapMode: TextEdit.Wrap
                color: Theme.color.label
                placeholderText: !Ai.configured ? "Add a model in Settings → AI" : Ai.needsKey ? "Add a credential to start" : Ai.attachments.length === 1 ? "Ask about this picture" : Ai.attachments.length > 1 ? "Ask about these pictures" : "Message " + Ai.model.name
                placeholderTextColor: Theme.color.tertiary
                background: null
                font { family: Theme.font.family; pixelSize: Theme.font.body }
                // Ctrl+V: a picture on the clipboard is added; otherwise text
                // pastes as usual (Ai.pastedText)
                Keys.onPressed: e => {
                    if (e.matches(StandardKey.Paste)) {
                        e.accepted = true;
                        Ai.attachClipboard();
                    }
                }
                Connections {
                    target: Ai
                    function onPastedText() { composer.paste(); }
                }
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
            transform: Theme.material ? pressFx2.effects : []
            ElasticControl { id: pressFx2; pressed: sendMouse.pressed; enabled: Theme.material }
            radius: Theme.material && sendMouse.pressed ? 10 : width / 2
            Behavior on radius { enabled: Theme.material; MaterialSpring { speed: "fast"; epsilon: 0.1 } }
            readonly property bool canSend: composer.text.trim() !== "" || Ai.attachments.length > 0
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

    // A chat deleted in past chats, still offered back
    UndoBar {
        visible: shown && !root.historyOpen
        anchors { left: parent.left; right: parent.right; bottom: composerBox.top; bottomMargin: 10 }
        z: 5
    }

    // Past chats
    ChatHistory {
        id: pastChats
        visible: root.historyOpen
        panelScreen: root.panelScreen
        anchors { top: header.bottom; topMargin: 12; left: parent.left; right: parent.right; bottom: parent.bottom }
        onBack: root.showHistory(false)
        onOpened: root.showHistory(false)
    }

    // Screenshot menu: above the composer's screenshot button
    Squircle {
        id: shotMenu
        visible: false
        anchors { left: parent.left; bottom: composerBox.top; bottomMargin: 6 }
        width: 220
        height: shotItems.implicitHeight + 10
        radius: Theme.material ? 16 : 14
        color: Theme.material ? Theme.m3.surfaceContainer : Theme.dark ? Qt.rgba(0.14, 0.14, 0.16, 0.97) : Qt.rgba(0.98, 0.98, 0.99, 0.98)
        border.width: Theme.material ? 0 : 1
        border.color: Theme.color.separator
        z: 10
        Column {
            id: shotItems
            x: 5
            y: 5
            width: parent.width - 10
            Repeater {
                model: [
                    { icon: "monitor", label: "Whole screen", run: () => ScreenTools.askScreen() },
                    { icon: "selection", label: "Area or window…", run: () => ScreenTools.select("ask") }
                ]
                Squircle {
                    required property var modelData
                    width: shotItems.width
                    height: Theme.material ? 44 : 30
                    radius: Theme.material ? 12 : 8
                    color: shotItemMouse.containsMouse ? (Theme.material ? Theme.color.highlight : Theme.accent) : "transparent"
                    Row {
                        x: Theme.material ? 12 : 8
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Theme.material ? 12 : 8
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: modelData.icon
                            size: Theme.material ? 18 : 14
                            color: shotItemMouse.containsMouse ? (Theme.material ? Theme.color.onHighlight : Theme.color.onAccent) : Theme.color.secondary
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.label
                            color: shotItemMouse.containsMouse ? (Theme.material ? Theme.color.onHighlight : Theme.color.onAccent) : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                    }
                    MouseArea {
                        id: shotItemMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            shotMenu.visible = false;
                            modelData.run();
                        }
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
                            : itemMouse.containsMouse ? (Theme.material ? Theme.color.highlight : Theme.accent) : "transparent"
                        Icon {
                            x: Theme.material ? 12 : 8
                            anchors.verticalCenter: parent.verticalCenter
                            visible: parent.modelData.key === Ai.model.key
                            name: "check"
                            bold: true
                            size: Theme.material ? 16 : 12
                            color: Theme.material && modelItem.chosen ? Theme.m3.onTertiaryContainer : itemMouse.containsMouse ? (Theme.material ? Theme.color.onHighlight : Theme.color.onAccent) : Theme.color.label
                        }
                        Text {
                            x: Theme.material ? 40 : 26
                            anchors.verticalCenter: parent.verticalCenter
                            text: parent.modelData.name + " · " + Ai.nameForProvider(parent.modelData.provider)
                            width: parent.width - x - 6
                            elide: Text.ElideRight
                            color: Theme.material && modelItem.chosen ? Theme.m3.onTertiaryContainer : itemMouse.containsMouse ? (Theme.material ? Theme.color.onHighlight : Theme.color.onAccent) : Theme.color.label
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
