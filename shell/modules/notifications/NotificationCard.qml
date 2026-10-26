import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.common
import qs.services

// Content of one notification: icon, app and time, summary, body, actions.
// A close button appears on hover. Background is up to the parent.
Item {
    id: root

    required property var notification
    property bool hovered: hover.hovered
    signal closeRequested

    implicitHeight: column.implicitHeight + 24

    HoverHandler { id: hover }

    TapHandler {
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
            visible: actionRepeater.count > 0
            topPadding: 6
            spacing: 6
            Repeater {
                id: actionRepeater
                model: root.notification?.actions.filter(a => a.identifier !== "default")
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
                        onClicked: parent.modelData.invoke()
                    }
                }
            }
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
