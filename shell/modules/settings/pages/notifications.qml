import QtQuick
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        footer: "While on, banners are hidden (critical alerts still show). Everything still arrives in Notification Center."
        Row {
            label: "Do Not Disturb"
            Switch { checked: Focus.on; onToggled: checked ? Focus.deactivate() : Focus.activate("dnd", 0, false) }
        }
    }
    Group {
        width: parent.width
        Row {
            label: "Notification history"
            detail: Notifications.list.length + " kept"
            Rectangle {
                width: clearText.implicitWidth + 24
                height: 26
                radius: 13
                color: Theme.color.fill
                opacity: Notifications.list.length > 0 ? 1 : 0.4
                Text { id: clearText; anchors.centerIn: parent; text: "Clear all"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                MouseArea { anchors.fill: parent; enabled: Notifications.list.length > 0; onClicked: Notifications.clearAll() }
            }
        }
    }
}
