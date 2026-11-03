import QtQuick
import qs.services

// A notification card's Reply, for both looks' cards: the composer open,
// sent (briefly), or what went wrong. It opens itself when the island's
// Reply picks its notification (Notification Center shows the card). The
// card gives it what was typed (`text`) and puts the cursor in its field
// when asked (focusField).
Item {
    id: root

    required property var notification
    property string text
    signal focusField

    property bool replying: false
    property bool sent: false
    property string error: ""
    readonly property bool canReply: Notifications.canReply(notification)
    function open() {
        if (!canReply)
            return;
        error = "";
        sent = false;
        replying = true;
        Qt.callLater(() => root.focusField());
    }
    function cancel() {
        replying = false;
        error = "";
    }
    function send() {
        const err = Notifications.reply(notification, text);
        if (err !== "") {
            error = err;
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
    Connections {
        target: Notifications
        function onReplyTargetChanged() { root.takeTarget(); }
    }
    Component.onCompleted: takeTarget()
    function takeTarget() {
        if (notification && Notifications.replyTarget === notification.id) {
            Notifications.replyTarget = -1;
            open();
        }
    }
}
