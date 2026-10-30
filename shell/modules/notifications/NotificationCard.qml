import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.common
import qs.services

// Content of one notification: icon, app and time, summary, body, actions.
// A close button appears on hover. Background is up to the parent.
// Messaging apps' notifications get Reply: a one-line composer under the
// actions (Enter or Send reply sends, Esc cancels). What's typed is kept in
// Notifications.drafts, so it survives the card moving from a banner into
// Notification Center.
Item {
    id: root

    required property var notification
    property bool hovered: hover.hovered
    signal closeRequested
    signal acted // one of its buttons was used

    // The composer: open, sent (briefly), or what went wrong
    property bool replying: false
    property bool sent: false
    property string replyError: ""
    readonly property bool canReply: Notifications.canReply(notification)
    function openReply() {
        if (!canReply)
            return;
        replyError = "";
        sent = false;
        replying = true;
        Qt.callLater(() => replyField.focusInput());
    }
    function cancelReply() {
        replying = false;
        replyError = "";
    }
    function sendReply() {
        const err = Notifications.reply(notification, replyField.text);
        if (err !== "") {
            replyError = err;
            return;
        }
        replying = false;
        sent = true;
    }
    onSentChanged: if (sent) sentTimer.restart()
    Timer {
        id: sentTimer
        interval: 2500
        onTriggered: root.sent = false
    }
    // Opened from the island's Reply (Notification Center shows this card)
    Connections {
        target: Notifications
        function onReplyTargetChanged() { root.takeTarget(); }
    }
    Component.onCompleted: takeTarget()
    function takeTarget() {
        if (notification && Notifications.replyTarget === notification.id) {
            Notifications.replyTarget = -1;
            openReply();
        }
    }

    implicitHeight: column.implicitHeight + 24

    HoverHandler { id: hover }

    TapHandler {
        enabled: !root.replying
        onTapped: {
            const def = root.notification?.actions.find(a => a.identifier === "default");
            if (def)
                def.invoke();
            root.notification?.dismiss();
        }
    }

    IconImage {
        id: icon
        x: 12
        y: 12
        implicitSize: 36
        source: Notifications.iconFor(root.notification)
        asynchronous: true
    }

    Column {
        id: column
        anchors { left: icon.right; leftMargin: 10; right: parent.right; rightMargin: 12; top: parent.top; topMargin: 12 }
        spacing: 2

        Item {
            width: parent.width
            height: appName.implicitHeight
            Text {
                id: appName
                anchors { left: parent.left; right: time.left; rightMargin: 8 }
                text: root.notification?.appName || "Notification"
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold; capitalization: Font.AllUppercase; letterSpacing: 0.3 }
            }
            Text {
                id: time
                anchors.right: parent.right
                text: Notifications.timeLabel(root.notification, clock.date)
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
                SystemClock { id: clock; precision: SystemClock.Minutes }
            }
        }
        Text {
            width: parent.width
            text: root.notification?.summary
            visible: text !== ""
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            text: root.notification?.body
            visible: text !== ""
            textFormat: Text.StyledText
            wrapMode: Text.Wrap
            maximumLineCount: 4
            elide: Text.ElideRight
            color: Theme.color.label
            opacity: 0.85
            font { family: Theme.font.family; pixelSize: Theme.font.body }
            onLinkActivated: link => Launch.open(link)
        }

        Row {
            visible: (actionRepeater.count > 0 || root.canReply) && !root.replying
            topPadding: 6
            spacing: 6
            // Reply (the composer below), before the app's own buttons
            Rectangle {
                visible: root.canReply && !root.sent
                width: replyLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: replyMouse.pressed ? Theme.color.fillPressed : replyMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                activeFocusOnTab: true
                Keys.onReturnPressed: root.openReply()
                Keys.onSpacePressed: root.openReply()
                border.width: activeFocus ? 2 : 0
                border.color: Theme.accent
                Text {
                    id: replyLabel
                    anchors.centerIn: parent
                    text: "Reply"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                MouseArea {
                    id: replyMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.openReply()
                }
            }
            Repeater {
                id: actionRepeater
                // (the app's own "inline-reply" action is the Reply above)
                model: root.notification?.actions.filter(a => a.identifier !== "default" && a.identifier !== "inline-reply")
                Rectangle {
                    required property var modelData
                    width: actionLabel.implicitWidth + 24
                    height: 28
                    radius: 14
                    color: actionMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                    Text {
                        id: actionLabel
                        anchors.centerIn: parent
                        text: parent.modelData.text
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                    }
                    MouseArea {
                        id: actionMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: {
                            parent.modelData.invoke();
                            root.acted();
                        }
                    }
                }
            }
        }

        // The composer
        Row {
            visible: root.replying
            width: parent.width
            topPadding: 6
            spacing: 6
            TextField {
                id: replyField
                width: parent.width - sendButton.width - parent.spacing
                height: 30
                placeholder: root.notification?.inlineReplyPlaceholder || "Reply"
                commitOnBlur: false
                text: Notifications.draft(root.notification)
                onTextChanged: if (root.replying) Notifications.setDraft(root.notification, text)
                onCommitted: root.sendReply()
                Keys.onEscapePressed: root.cancelReply()
            }
            Rectangle {
                id: sendButton
                readonly property bool ready: replyField.text.trim() !== ""
                width: sendLabel.implicitWidth + 24
                height: 30
                radius: 15
                color: !ready ? Theme.color.fill : sendMouse.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent
                Text {
                    id: sendLabel
                    anchors.centerIn: parent
                    text: "Send reply"
                    color: sendButton.ready ? Theme.color.onAccent : Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea {
                    id: sendMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: sendButton.ready ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: root.sendReply()
                }
            }
        }
        Text {
            visible: root.replyError !== "" || root.sent
            width: parent.width
            topPadding: 4
            wrapMode: Text.Wrap
            text: root.replyError !== "" ? root.replyError : "Sent"
            color: root.replyError !== "" ? Theme.color.danger : Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }

    // Close button, top left like macOS
    Rectangle {
        x: -6
        y: -6
        width: 22
        height: 22
        radius: 11
        visible: root.hovered
        color: Theme.dark ? Qt.rgba(0.22, 0.22, 0.25, 1) : "white"
        border.width: 1
        border.color: Theme.color.separator
        Icon {
            anchors.centerIn: parent
            name: "x"
            bold: true
            size: 11
            color: Theme.color.secondary
        }
        MouseArea {
            anchors.fill: parent
            onClicked: root.closeRequested()
        }
    }
}
