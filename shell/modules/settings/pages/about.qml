import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        Row { label: "iRice"; Text { text: "1.0"; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "System"; Text { text: SystemInfo.osName; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "Computer name"; Text { text: SystemInfo.hostname; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row {
            label: "Your name"
            detail: "Today greets you by it. Empty: your account's name"
            TextField {
                width: 220
                placeholder: Today.accountName.split(/\s+/)[0] || "Your name"
                text: Config.options.today.name
                onCommitted: t => Config.options.today.name = t.trim()
            }
        }
        Row { label: "Processor"; Text { text: SystemInfo.cpu; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "Graphics"; Text { text: SystemInfo.gpu; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
        Row { label: "Memory"; Text { text: SystemInfo.memory; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.body } } }
    }
    Group {
        width: parent.width
        SubpageLink {
            icon: "heartbeat"
            label: "System health"
            detail: "Whether the services iRice uses are working, and how to fix them"
            value: Health.checks.length === 0 ? "" : Health.attention.length === 0 ? "All good" : Health.attention.length + " to check"
            page: "about/health"
        }
    }
    Group {
        width: parent.width
        footer: "Settings live in ~/.config/irice/config.json and apply as you change them."
        Row {
            label: "Configuration file"
            Rectangle { // (Material: a 40 px tonal button)
                width: openText.implicitWidth + (Theme.material ? 32 : 24)
                height: Theme.material ? 40 : 26
                radius: height / 2
                color: Theme.material ? Theme.m3.secondaryContainer : Theme.color.fill
                Text { id: openText; anchors.centerIn: parent; text: "Open"; color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.Medium } }
                MouseArea { anchors.fill: parent; onClicked: Launch.open("file://" + Config.dir + "/config.json") }
            }
        }
    }
}
