import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.common
import qs.services

// NotificationCard in the Material look: Android 16's notification. The
// app's icon in a tonal circle (a sender's picture fills it), "App · time"
// small, the title emphasized, the body, a wavy bar for a notification that
// reports progress, then the actions as tonal pills. Reply opens a pill
// field with a round send button inside it (Enter sends, Esc cancels; the
// draft lives in Notifications.drafts like NotificationCard's). The
// background is up to the parent. Same properties and signals as
// NotificationCard, plus the group's expand button.
Item {
    id: root

    required property var notification
    property bool hovered: hover.hovered
    signal closeRequested
    signal acted // one of its buttons was used

    // A stack of this app's notifications: the count and a chevron (top
    // right) expand or collapse it; `tapExpands` makes a click on the card
    // expand it too (a collapsed stack), instead of opening the notification
    property int groupCount: 1
    property bool groupOpen: false
    property bool tapExpands: false
    signal groupToggled

    // The composer: open, sent (briefly), or what went wrong
    NotificationReply {
        id: reply
        notification: root.notification
        text: replyInput.text
        onFocusField: {
            replyInput.cursorPosition = replyInput.length;
            replyInput.forceActiveFocus();
        }
    }
    readonly property alias replying: reply.replying
    readonly property alias sent: reply.sent
    readonly property alias replyError: reply.error
    readonly property alias canReply: reply.canReply
    function openReply() { reply.open(); }
    function cancelReply() { reply.cancel(); }
    function sendReply() { reply.send(); }

    // Progress reported by the app (the "value" hint, 0 to 100)
    readonly property real progress: {
        const v = root.notification?.hints?.value;
        return typeof v === "number" && v >= 0 ? Math.min(100, v) / 100 : -1;
    }

    implicitHeight: column.implicitHeight + 32

    HoverHandler { id: hover }

    TapHandler {
        enabled: !root.replying
        onTapped: {
            if (root.tapExpands) {
                root.groupToggled();
                return;
            }
            const def = root.notification?.actions.find(a => a.identifier === "default");
            if (def)
                def.invoke();
            root.notification?.dismiss();
        }
    }

    // The app's icon on a tonal circle, or the sender's picture filling it
    Item {
        id: avatar
        x: 16
        y: 16
        width: 40
        height: 40
        readonly property bool picture: !!root.notification?.image
        Rectangle {
            anchors.fill: parent
            visible: !avatar.picture
            radius: width / 2
            color: Theme.m3.secondaryContainer
        }
        IconImage {
            id: iconImage
            anchors.centerIn: parent
            implicitSize: avatar.picture ? 40 : 24
            source: Notifications.iconFor(root.notification)
            asynchronous: true
        }
        Loader {
            anchors.fill: parent
            active: avatar.picture
            sourceComponent: MaterialShape {
                shape: "circle"
                color: Theme.m3.secondaryContainer
                source: iconImage
            }
        }
    }

    // Top right: close (on hover) and the stack's count and chevron
    MaterialCardCorner {
        id: corner
        anchors { right: parent.right; rightMargin: 12; top: parent.top; topMargin: 12 }
        hovered: root.hovered
        closeName: "Dismiss " + (root.notification?.summary || "notification")
        groupCount: root.groupCount
        groupOpen: root.groupOpen
        onCloseClicked: root.closeRequested()
        onGroupToggled: root.groupToggled()
    }

    Column {
        id: column
        anchors { left: avatar.right; leftMargin: 12; right: parent.right; rightMargin: 16; top: parent.top; topMargin: 16 }
        spacing: 2

        // App · time (beside the corner buttons)
        Text {
            width: parent.width - (corner.width > 0 ? corner.width + 4 : 0)
            height: 20
            verticalAlignment: Text.AlignVCenter
            text: {
                const t = Notifications.timeLabel(root.notification, clock.date);
                return (root.notification?.appName || "Notification") + (t ? " · " + t : "");
            }
            elide: Text.ElideRight
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
            SystemClock { id: clock; precision: SystemClock.Minutes }
        }
        Text {
            width: parent.width
            textFormat: Text.PlainText
            text: root.notification?.summary ?? ""
            visible: text !== ""
            elide: Text.ElideRight
            color: Theme.m3.onSurface
            font { family: Theme.font.family; pixelSize: 14; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            text: Notifications.safeMarkup(root.notification?.body)
            visible: text !== ""
            textFormat: Text.StyledText
            wrapMode: Text.Wrap
            maximumLineCount: 4
            elide: Text.ElideRight
            color: Theme.m3.onSurfaceVariant
            linkColor: Theme.m3.primary
            lineHeight: 20
            lineHeightMode: Text.FixedHeight
            font { family: Theme.font.family; pixelSize: 14 }
            onLinkActivated: link => Launch.openLink(link)
        }
        // Progress the app reports (downloads, copies)
        Item {
            visible: root.progress >= 0
            width: parent.width
            height: 18
            WavyProgress {
                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                value: root.progress
            }
        }

        // Reply (the composer below), then the app's own buttons
        Flow {
            visible: (actionRepeater.count > 0 || root.canReply) && !root.replying
            width: parent.width
            topPadding: 10
            spacing: 8
            Pill {
                visible: root.canReply && !root.sent
                label: "Reply"
                onClicked: root.openReply()
            }
            Repeater {
                id: actionRepeater
                // (the app's own "inline-reply" action is the Reply above)
                model: root.notification?.actions.filter(a => a.identifier !== "default" && a.identifier !== "inline-reply") ?? []
                Pill {
                    required property var modelData
                    label: modelData.text
                    onClicked: {
                        modelData.invoke();
                        root.acted();
                    }
                }
            }
        }

        // The composer: a pill field with the send button inside it
        Item {
            visible: root.replying
            width: parent.width
            height: 52
            Rectangle {
                id: field
                anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                height: 40
                radius: 20
                color: Theme.m3.surfaceContainerHighest
                border.width: replyInput.activeFocus ? 2 : 0
                border.color: Theme.m3.primary
                TextInput {
                    id: replyInput
                    anchors { left: parent.left; leftMargin: 16; right: send.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    color: Theme.m3.onSurface
                    selectionColor: Qt.alpha(Theme.m3.primary, 0.3)
                    selectedTextColor: Theme.m3.onSurface
                    selectByMouse: true
                    clip: true
                    font { family: Theme.font.family; pixelSize: 14 }
                    text: Notifications.draft(root.notification)
                    onTextChanged: if (root.replying) Notifications.setDraft(root.notification, text)
                    onAccepted: root.sendReply()
                    Keys.onEscapePressed: root.cancelReply()
                    Accessible.role: Accessible.EditableText
                    Accessible.name: root.notification?.inlineReplyPlaceholder || "Reply"
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width
                        visible: replyInput.text === ""
                        elide: Text.ElideRight
                        text: root.notification?.inlineReplyPlaceholder || "Reply"
                        color: Theme.m3.onSurfaceVariant
                        font: replyInput.font
                    }
                }
                // Send: filled once there's something to send
                Rectangle {
                    id: send
                    readonly property bool ready: replyInput.text.trim() !== ""
                    anchors { right: parent.right; rightMargin: 4; verticalCenter: parent.verticalCenter }
                    width: 32
                    height: 32
                    transform: pressFx1.effects
                    ElasticControl { id: pressFx1; pressed: sendMouse.pressed }
                    radius: sendMouse.pressed && ready ? 8 : 16
                    Behavior on radius { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }
                    color: !ready ? "transparent" : sendMouse.pressed ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, 0.1)) : sendMouse.containsMouse ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, 0.08)) : Theme.m3.primary
                    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                    Accessible.role: Accessible.Button
                    Accessible.name: "Send reply"
                    Icon {
                        anchors.centerIn: parent
                        name: "paper-plane-tilt"
                        fill: send.ready
                        size: 18
                        color: send.ready ? Theme.m3.onPrimary : Qt.alpha(Theme.m3.onSurface, 0.38)
                    }
                    MouseArea {
                        id: sendMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: send.ready ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onClicked: if (send.ready) root.sendReply()
                    }
                }
            }
        }
        Text {
            visible: root.replyError !== "" || root.sent
            width: parent.width
            topPadding: 6
            wrapMode: Text.Wrap
            text: root.replyError !== "" ? root.replyError : "Sent"
            color: root.replyError !== "" ? Theme.m3.error : Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }

    // An action: a tonal pill (secondaryContainer) whose corners tighten
    // while it's pressed, like Material's buttons
    component Pill: Rectangle {
        id: pill
        property string label
        property string icon
        signal clicked
        width: pillRow.implicitWidth + 32
        height: 32
        transform: pressFx2.effects
        ElasticControl { id: pressFx2; pressed: pillMouse.pressed }
        radius: pillMouse.pressed ? 8 : 16
        Behavior on radius { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }
        color: pillMouse.pressed ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.1)) : pillMouse.containsMouse ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.08)) : Theme.m3.secondaryContainer
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        activeFocusOnTab: visible
        Keys.onReturnPressed: pill.clicked()
        Keys.onSpacePressed: pill.clicked()
        Accessible.role: Accessible.Button
        Accessible.name: label
        FocusRing { radius: pill.radius + 5 }
        Row {
            id: pillRow
            anchors.centerIn: parent
            spacing: 6
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                visible: pill.icon !== ""
                name: pill.icon
                size: 18
                color: Theme.m3.onSecondaryContainer
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: pill.label
                color: Theme.m3.onSecondaryContainer
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
            }
        }
        MouseArea {
            id: pillMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: pill.clicked()
        }
    }
}
