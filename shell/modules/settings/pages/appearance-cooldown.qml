import QtQuick
import QtQuick.Controls as QQC
import Quickshell
import Quickshell.Io
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22
    readonly property string pageTitle: "Cool-down and performance"

    Group {
        width: parent.width
        title: "Cool-down"
        footer: "Less work for the processor and graphics, so the laptop runs cooler and lasts longer: all of this comes back when cool-down ends. Also in Control Center."
        Row {
            label: "Cool-down"
            detail: CoolDown.active ? "On · " + CoolDown.why : CoolDown.why !== "" ? CoolDown.why : "Off"
            Switch { checked: CoolDown.active; onToggled: CoolDown.toggle() }
        }
        Row {
            label: "When the CPU is hot"
            detail: "At the island's temperature alert (Menu Bar → Dynamic Island)"
            Switch { checked: Config.options.cooling.whenHot; onToggled: Config.options.cooling.whenHot = !checked }
        }
        Row {
            visible: Config.options.cooling.whenHot
            label: "When it gets hot"
            detail: Config.options.cooling.hotAction === "ask" ? "The island asks first; the question stays until you answer. Not now keeps it off until the laptop cools." : "Starts by itself; ends after 3 minutes well below the alert temperature"
            Segmented {
                width: 240
                readonly property var values: ["auto", "ask"]
                options: ["Automatically", "Ask first"]
                current: Math.max(0, values.indexOf(Config.options.cooling.hotAction))
                onSelected: i => Config.options.cooling.hotAction = values[i]
            }
        }
        Row {
            label: "On battery"
            Switch { checked: Config.options.cooling.onBattery; onToggled: Config.options.cooling.onBattery = !checked }
        }
        Row {
            label: "Glass"
            detail: "During cool-down"
            Segmented {
                width: 260
                readonly property var values: ["keep", "frosted", "solid"]
                options: ["Keep", "Frosted", "Solid"]
                current: Math.max(0, values.indexOf(Config.options.cooling.glass))
                onSelected: i => Config.options.cooling.glass = values[i]
            }
        }
        Row {
            label: "Lighter blur"
            Switch { checked: Config.options.cooling.lightBlur; onToggled: Config.options.cooling.lightBlur = !checked }
        }
        Row {
            label: "Shorter animations"
            Switch { checked: Config.options.cooling.shortAnimations; onToggled: Config.options.cooling.shortAnimations = !checked }
        }
        Row {
            label: "Lower refresh rate"
            detail: "The laptop screen's lowest rate (48 Hz on most); apps draw fewer frames too. The screen goes black for a moment when it switches."
            Switch { checked: Config.options.cooling.refreshDrop; onToggled: Config.options.cooling.refreshDrop = !checked }
        }
        Row {
            label: "Pause live wallpaper"
            Switch { checked: Config.options.cooling.pauseLiveWallpaper; onToggled: Config.options.cooling.pauseLiveWallpaper = !checked }
        }
    }

    Group {
        width: parent.width
        title: "Performance"
        Row {
            label: "Keep panels loaded"
            detail: "Spotlight, Control Center and Notification Center stay ready after their first open: faster and lighter on the processor, about 50 MB more memory"
            Switch {
                checked: Config.options.performance.keepPanelsLoaded
                onToggled: Config.options.performance.keepPanelsLoaded = !checked
            }
        }
    }
}
