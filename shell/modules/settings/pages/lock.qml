import QtQuick
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    component Choice: Segmented {
        required property string key
        required property var values
        width: 300
        current: Math.max(0, values.indexOf(Config.options.lock[key]))
        onSelected: i => Config.options.lock[key] = values[i]
    }

    Group {
        width: parent.width
        footer: Config.options.lock.layout === "macos" ? "The clock up top; your picture, name and the password field at the bottom." : "A large clock with small widgets, Now Playing and your notifications, like iPhone."
        Row {
            label: "Layout"
            Choice { key: "layout"; values: ["ios", "macos"]; options: ["iOS", "macOS"] }
        }
        Row {
            label: "Cards and field"
            detail: "What notifications, Now Playing and the password field are made of"
            Choice { key: "material"; values: ["glass", "frost", "solid", "system"]; options: ["Glass", "Frost", "Solid", "System"] }
        }
        Row {
            label: "Text"
            detail: "Automatic picks light or dark text for the wallpaper"
            Choice { key: "text"; values: ["light", "dark", "auto"]; options: ["Light", "Dark", "Automatic"] }
        }
        Row {
            label: "Show notification previews"
            detail: "Off shows only which app sent each notification"
            Switch { checked: Config.options.lock.previews; onToggled: Config.options.lock.previews = !checked }
        }
    }

    Group {
        width: parent.width
        title: "StandBy"
        footer: "While charging and left alone, the screen shows a large clock, your widgets or photos instead of going dark. Swipe or scroll to change; any key brings your desktop back."
        Row {
            label: "StandBy"
            Switch { checked: Config.options.lock.standby; onToggled: Config.options.lock.standby = !checked }
        }
        Row {
            label: "Prevent burn-in"
            detail: "Moves the clock every minute and turns the screen dark for a minute every 15 minutes"
            enabled: Config.options.lock.standby
            Switch { checked: Config.options.lock.standbyShift; onToggled: Config.options.lock.standbyShift = !checked }
        }
    }

    Group {
        width: parent.width
        Row {
            label: "Preview"
            detail: "Locks the screen now"
            Rectangle {
                width: lockText.implicitWidth + 28
                height: 28
                radius: Theme.radius.control
                color: lockMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                Text { id: lockText; anchors.centerIn: parent; text: "Lock Screen"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                MouseArea { id: lockMouse; anchors.fill: parent; hoverEnabled: true; onClicked: System.power("lock") }
            }
        }
    }
}
