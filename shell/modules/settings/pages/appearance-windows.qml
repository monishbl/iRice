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
    readonly property string pageTitle: "Windows and Overview"

    Group {
        width: parent.width
        title: "Windows"
        footer: Config.options.appearance.decorations === "titlebars" && !Decorations.pluginLoaded ? "Title bars need the hyprbars plugin, which isn't installed." : Config.options.appearance.decorations === "controls" ? (Decorations.pluginControls ? "Hover the handle at the top of any window for close, minimize and zoom; rest on the green button to tile. Drawn by the compositor, so it moves with the window." : "Hover the handle at the top of the focused window for close, minimize and zoom. With the compositor glass plugin it's drawn on every window and never lags behind.") : Config.options.appearance.decorations === "titlebars" ? "macOS-style title bars with traffic lights. Double-click a title bar to zoom." : "Borderless windows; use the Window menu for window actions."
        Row {
            label: "Decorations"
            Segmented {
                width: 300
                readonly property var values: ["none", "controls", "titlebars"]
                options: ["None", "Window controls", "Title bars"]
                current: Math.max(0, values.indexOf(Config.options.appearance.decorations))
                onSelected: i => Config.options.appearance.decorations = values[i]
            }
        }
        Row {
            label: "Pointer"
            detail: Config.options.appearance.cursor === "bibata" ? "Bibata Modern Classic, like end4" : "The macOS pointer"
            Segmented {
                width: 300
                readonly property var values: ["macos", "bibata"]
                options: ["macOS", "Bibata"]
                current: Math.max(0, values.indexOf(Config.options.appearance.cursor))
                onSelected: i => Config.options.appearance.cursor = values[i]
            }
        }
        Row {
            label: "Window transparency"
            detail: Config.options.appearance.windowOpacity === "off" ? "Windows are fully opaque" : "Video, games and fullscreen stay opaque"
            Segmented {
                width: 300
                readonly property var values: ["off", "subtle", "more", "custom"]
                options: ["Off", "Subtle", "More", "Custom"]
                current: Math.max(0, values.indexOf(Config.options.appearance.windowOpacity))
                onSelected: i => Config.options.appearance.windowOpacity = values[i]
            }
        }
        Repeater {
            model: Config.options.appearance.windowOpacity === "custom" ? [
                { key: "windowOpacityActive", label: "Focused window" },
                { key: "windowOpacityInactive", label: "Other windows" }
            ] : []
            Row {
                required property var modelData
                label: modelData.label
                detail: Math.round(Config.options.appearance[modelData.key] * 100) + "% opaque"
                Slider {
                    width: 220
                    from: 0.6
                    to: 1
                    value: Config.options.appearance[parent.parent.modelData.key]
                    onMoved: v => Config.options.appearance[parent.parent.modelData.key] = Math.round(v * 100) / 100
                }
            }
        }
        Row {
            label: "Window tabs"
            detail: Config.options.appearance.windowTabs === "finder" ? "Finder-style tabs with icons, drawn by iRice" + (GlassPlugin.available ? "" : " (needs the glass plugin to know the front tab)") : "Hyprland's tab bar, restyled; smoothest"
            Segmented {
                width: 300
                readonly property var values: ["hyprland", "finder"]
                options: ["Hyprland", "Finder"]
                current: Math.max(0, values.indexOf(Config.options.appearance.windowTabs))
                onSelected: i => Config.options.appearance.windowTabs = values[i]
            }
        }
        Row {
            visible: Config.options.appearance.decorations === "titlebars" && !Decorations.pluginLoaded
            label: "hyprbars plugin"
            detail: "Builds the official plugin with hyprpm in a terminal."
            Rectangle {
                width: instLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: Theme.accent
                Text { id: instLabel; anchors.centerIn: parent; text: "Install…"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                MouseArea { anchors.fill: parent; onClicked: Decorations.installPlugin() }
            }
        }
    }

    Group {
        width: parent.width
        title: "Stage Manager"
        footer: "Keeps one group of windows in front and the others as thumbnails on the left. A new app opens in its own stage; Shift-click a thumbnail to add its windows to the stage in front. Also in Control Center."
        Row {
            label: "Stage Manager"
            Switch {
                checked: Config.options.stageManager.enabled
                onToggled: Config.options.stageManager.enabled = !checked
            }
        }
        Row {
            visible: Config.options.stageManager.enabled
            label: "Show recent stages"
            detail: "Hide them to give windows the full width; switch with the dock or Mission Control."
            Switch {
                checked: Config.options.stageManager.strip
                onToggled: Config.options.stageManager.strip = !checked
            }
        }
    }

    Group {
        width: parent.width
        title: "Overview"
        footer: "Window previews apply to the Overview, Mission Control and the Stage Manager strip. Live previews update up to 30 times a second and use more power while those are open; nothing is captured while they're closed."
        Row {
            label: "Style"
            detail: Config.options.overview.style === "macos" ? "Your spaces in a strip on top, a search in the middle" : "Your spaces as large previews, with a search on top"
            Segmented {
                width: 260
                readonly property var values: ["grid", "macos"]
                options: ["Grid", "macOS"]
                current: Math.max(0, values.indexOf(Config.options.overview.style))
                onSelected: i => Config.options.overview.style = values[i]
            }
        }
        Row {
            label: "Show the wallpaper in spaces"
            detail: "Behind the windows in each space's preview (Overview and Mission Control)"
            Switch { checked: Config.options.overview.wallpapers; onToggled: Config.options.overview.wallpapers = !checked }
        }
        Row {
            label: "Window previews"
            detail: Config.options.overview.previews === "live" ? "Every window keeps updating, up to 30 frames a second" : Config.options.overview.previews === "hybrid" ? "Windows on the current space keep updating (up to 30 frames a second); the rest are pictures" : "A picture of each window, taken when the Overview opens"
            Segmented {
                width: 260
                readonly property var values: ["snapshot", "hybrid", "live"]
                options: ["Snapshot", "Hybrid", "Live"]
                current: Math.max(0, values.indexOf(Config.options.overview.previews))
                onSelected: i => Config.options.overview.previews = values[i]
            }
        }
    }

}
