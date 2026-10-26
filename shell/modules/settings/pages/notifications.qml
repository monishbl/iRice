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
        footer: "Without it, notifications and the calendar, to-do and focus widgets are each their own piece of glass, like Control Center's controls."
        Row {
            label: "Notification Center panel"
            Switch { checked: Config.options.notificationCenter.background; onToggled: Config.options.notificationCenter.background = !checked }
        }
    }
    Group {
        width: parent.width
        title: "Sound effects"
        Row {
            label: "Play sound effects"
            Switch { checked: Config.options.sounds.enabled; onToggled: Config.options.sounds.enabled = !checked }
        }
        Row {
            visible: Config.options.sounds.enabled
            label: "When you log in"
            Switch { checked: Config.options.sounds.startup; onToggled: Config.options.sounds.startup = !checked }
        }
        Row {
            visible: Config.options.sounds.enabled
            label: "Screenshots"
            Switch { checked: Config.options.sounds.screenshot; onToggled: Config.options.sounds.screenshot = !checked }
        }
        Row {
            visible: Config.options.sounds.enabled
            label: "Emptying the Trash"
            Switch { checked: Config.options.sounds.trash; onToggled: Config.options.sounds.trash = !checked }
        }
        Row {
            visible: Config.options.sounds.enabled
            label: "Volume"
            detail: Math.round(Config.options.sounds.volume * 100) + "%"
            Slider {
                width: 220
                from: 0
                to: 1
                value: Config.options.sounds.volume
                onMoved: v => Config.options.sounds.volume = v
                onPressedChanged: if (!pressed) Sounds.play("screenshot") // hear the level
            }
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
