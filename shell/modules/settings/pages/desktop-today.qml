import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Settings → Desktop & Widgets → Today (services/Today.qml): the morning
// screen on the first unlock of the day, and its widget.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22
    readonly property string pageTitle: "Today"
    readonly property var cfg: Config.options.today

    Group {
        width: parent.width
        footer: Today.mode === "ask" ? "Opens on your first unlock after 4 AM. Close it and it's gone until tomorrow; Pin for today keeps it on the desktop until the day ends." : Today.mode === "widget" ? "No morning screen. The Today widget stays on your desktop; remove it and it comes back tomorrow." : Today.mode === "both" ? "Opens on your first unlock after 4 AM, and the Today widget stays on your desktop." : "Today doesn't open by itself. You can still add the Today widget from the widget gallery."
        Row {
            label: "Show Today"
            Segmented {
                width: 340
                readonly property var values: ["ask", "widget", "both", "off"]
                options: ["Every morning", "Widget", "Both", "Off"]
                current: Math.max(0, values.indexOf(Today.mode))
                onSelected: i => page.cfg.mode = values[i]
            }
        }
        Row {
            label: "Entrance"
            detail: page.cfg.entrance === "shrink" ? "A full-screen glass sheet draws in to the panel" : page.cfg.entrance === "drop" ? "A drop falls from the top and spreads into the panel" : "The panel settles out of the frosted screen"
            Segmented {
                width: 300
                readonly property var values: ["bloom", "shrink", "drop"]
                options: ["Bloom", "Shrink", "Drop"]
                current: Math.max(0, values.indexOf(page.cfg.entrance))
                onSelected: i => page.cfg.entrance = values[i]
            }
        }
        Row {
            label: "Preview"
            detail: "Opens it now, with the chime"
            SettingsButton {
                text: "Show Today"
                icon: "play"
                onClicked: Today.show("")
            }
        }
    }

    Group {
        width: parent.width
        title: "Greeting"
        Row {
            label: "Your name"
            detail: "What the greeting calls you. Empty: " + (Today.accountName ? "\"" + Today.accountName.split(/\s+/)[0] + "\", from your account" : "no name")
            TextField {
                width: 220
                placeholder: Today.accountName.split(/\s+/)[0] || "Your name"
                text: page.cfg.name
                onCommitted: t => page.cfg.name = t.trim()
            }
        }
        Row {
            label: "Chime"
            detail: "From your sound style (Settings → Notifications → Sound effects)"
            Switch {
                checked: Config.options.sounds.today
                onToggled: Config.options.sounds.today = !checked
            }
        }
    }
}
