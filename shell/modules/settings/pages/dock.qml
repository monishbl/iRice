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

    Group {
        width: parent.width
        Row {
            label: "Size"
            detail: Config.options.dock.iconSize + " px"
            Slider {
                width: 220
                from: 32
                to: 72
                stepSize: 2
                value: Config.options.dock.iconSize
                onMoved: v => Config.options.dock.iconSize = v
            }
        }
        Row {
            label: "Apps button"
            detail: "Opens the App Library (also Super+Shift+Space)"
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
        Row {
            label: "Magnification"
            Switch { checked: Config.options.dock.magnification; onToggled: Config.options.dock.magnification = !checked }
        }
        Row {
            visible: Config.options.dock.magnification
            label: "Magnified size"
            Slider {
                width: 220
                from: 1.1
                to: 2
                stepSize: 0.05
                value: Config.options.dock.magnifiedScale
                onMoved: v => Config.options.dock.magnifiedScale = v
            }
        }
    }

    Group {
        width: parent.width
        title: "Kept in Dock"
        footer: "Running apps that aren't kept appear after the divider."
        Repeater {
            model: Config.options.dock.pinned
            Row {
                id: pinnedRow
                required property string modelData
                readonly property var entry: DesktopEntries.byId(modelData)
                label: entry?.name ?? modelData
                detail: entry ? "" : "Not installed"
                Rectangle {
                    width: 26
                    height: 26
                    radius: 13
                    color: removeMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                    Icon { anchors.centerIn: parent; name: "minus"; bold: true; size: 12 }
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
            model: filter.text.trim().length < 2 ? [] : DesktopEntries.applications.values.filter(a => !a.noDisplay && !Config.options.dock.pinned.includes(a.id) && a.name.toLowerCase().includes(filter.text.trim().toLowerCase())).slice(0, 6)
            Row {
                id: addRow
                required property var modelData
                label: modelData.name
                Rectangle {
                    width: addLabel.implicitWidth + 20
                    height: 26
                    radius: 13
                    color: Theme.accent
                    Text { id: addLabel; anchors.centerIn: parent; text: "Add"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: Config.options.dock.pinned = Config.options.dock.pinned.concat([addRow.modelData.id])
                    }
                }
            }
        }
    }
}
