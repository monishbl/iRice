import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: root
    width: parent?.width ?? 0
    spacing: 22

    readonly property bool on: Config.options.dock.enabled

    Group {
        width: parent.width
        footer: root.on ? "" : "The App Drawer stays on Super+Ctrl+Space, and windows use the space at the bottom."
        Row {
            label: "Show the dock"
            Switch { checked: Config.options.dock.enabled; onToggled: Config.options.dock.enabled = !checked }
        }
    }

    Group {
        width: parent.width
        enabled: root.on
        opacity: root.on ? 1 : 0.45
        Row {
            label: "Size"
            detail: (Theme.material ? Math.max(36, Math.min(56, Config.options.dock.iconSize)) : Config.options.dock.iconSize) + " px"
            // (Material's dock keeps its icons between 36 and 56)
            Slider {
                width: 220
                from: Theme.material ? 36 : 32
                to: Theme.material ? 56 : 72
                stepSize: 2
                value: Config.options.dock.iconSize
                onMoved: v => Config.options.dock.iconSize = v
            }
        }
        Row {
            label: "Apps button"
            detail: "Opens the App Drawer (also Super+Ctrl+Space)"
            Switch { checked: Config.options.dock.appsButton; onToggled: Config.options.dock.appsButton = !checked }
        }
        Row {
            label: "Automatically hide"
            detail: Config.options.dock.autoHide === "windows" ? "Hides when a window overlaps it; point at the bottom edge to show it." : Config.options.dock.autoHide === "always" ? "Point at the bottom edge of the screen to show it." : ""
            Segmented {
                width: 260
                readonly property var values: ["off", "windows", "always"]
                options: ["Never", "Over windows", "Always"]
                current: Math.max(0, values.indexOf(Config.options.dock.autoHide))
                onSelected: i => Config.options.dock.autoHide = values[i]
            }
        }
    }

    Group {
        width: parent.width
        enabled: root.on
        opacity: root.on ? 1 : 0.45
        title: "Kept in Dock"
        footer: "Running apps that aren't kept appear after the divider."
        Repeater {
            model: Config.options.dock.pinned
            Row {
                id: pinnedRow
                required property string modelData
                readonly property var entry: Apps.byId(modelData)
                label: entry?.name ?? modelData
                detail: entry ? "" : "Not installed"
                Rectangle { // (Material: a 40 px tonal icon button)
                    width: Theme.material ? 40 : 26
                    height: width
                    radius: width / 2
                    color: Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, removeMouse.containsMouse ? 0.08 : 0)) : removeMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                    Icon { anchors.centerIn: parent; name: "minus"; bold: !Theme.material; size: Theme.material ? 20 : 12; color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label }
                    MouseArea {
                        id: removeMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: Config.options.dock.pinned = Config.options.dock.pinned.filter(p => p !== pinnedRow.modelData)
                    }
                }
            }
        }
    }

    Group {
        width: parent.width
        enabled: root.on
        opacity: root.on ? 1 : 0.45
        title: "Add to Dock"
        Row {
            label: "Search apps"
            TextField {
                id: filter
                width: 240
                placeholder: "App name"
            }
        }
        Repeater {
            model: filter.text.trim().length < 2 ? [] : Apps.entries.filter(a => !a.noDisplay && !Config.options.dock.pinned.includes(a.id) && a.name.toLowerCase().includes(filter.text.trim().toLowerCase())).slice(0, 6)
            Row {
                id: addRow
                required property var modelData
                label: modelData.name
                Rectangle { // (Material: a 40 px filled button)
                    width: addLabel.implicitWidth + (Theme.material ? 32 : 20)
                    height: Theme.material ? 40 : 26
                    radius: height / 2
                    color: Theme.accent
                    Text { id: addLabel; anchors.centerIn: parent; text: "Add"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold } }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: Config.options.dock.pinned = Config.options.dock.pinned.concat([addRow.modelData.id])
                    }
                }
            }
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            Row {
                label: "Space between icons"
                detail: Math.round(Config.options.dock.iconSize * (Config.options.dock.gap ?? 0.18)) + " px"
                Slider {
                    width: 220
                    from: 0.05
                    to: 0.4
                    value: Config.options.dock.gap ?? 0.18
                    onMoved: v => Config.options.dock.gap = Math.round(v * 100) / 100
                }
            }
            Row {
                visible: Theme.material // (the Material App Drawer's option)
                label: "App names in the App Drawer"
                detail: "Off shows only the icons"
                Switch { checked: Config.options.dock.appNames ?? true; onToggled: Config.options.dock.appNames = !checked }
            }
            Row {
                label: "Recent apps"
                detail: "Up to three apps you used lately, after a divider"
                Switch { checked: Config.options.dock.recentApps; onToggled: Config.options.dock.recentApps = !checked }
            }
            Row {
                label: "Downloads"
                Switch { checked: Config.options.dock.downloads; onToggled: Config.options.dock.downloads = !checked }
            }
            Row {
                label: "Trash"
                Switch { checked: Config.options.dock.trash; onToggled: Config.options.dock.trash = !checked }
            }
            // One slider: its left end is off (the size you had is kept for
            // when it's on again)
            Row {
                visible: !Theme.material // (Material's dock doesn't magnify)
                label: "Magnification"
                detail: Config.options.dock.magnification ? Math.round(Config.options.dock.magnifiedScale * 100) + "% when you point at an icon" : "Off"
                Column {
                    width: 220
                    spacing: 2
                    Slider {
                        width: parent.width
                        from: 1
                        to: 2
                        stepSize: 0.05
                        value: Config.options.dock.magnification ? Config.options.dock.magnifiedScale : 1
                        onMoved: v => {
                            if (v < 1.025) { // (only the very end: one step right turns it on)
                                Config.options.dock.magnification = false;
                                return;
                            }
                            Config.options.dock.magnifiedScale = Math.max(1.1, Math.round(v * 100) / 100);
                            Config.options.dock.magnification = true;
                        }
                    }
                    Item {
                        width: parent.width
                        height: magOff.implicitHeight
                        Text { id: magOff; text: "Off"; color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption } }
                        Text { anchors.right: parent.right; text: "Large"; color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption } }
                    }
                }
            }
        }
    }
}
