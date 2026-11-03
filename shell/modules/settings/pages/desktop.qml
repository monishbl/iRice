import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: root
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        title: "Widgets"
        footer: "Drag a widget to move it; it snaps to the grid. Right-click a widget to change its size, set what it shows (Widget Options) or remove it, or right-click the desktop."
        Row {
            label: "Edit Widgets"
            detail: Config.options.desktop.widgets.length + (Config.options.desktop.widgets.length === 1 ? " widget" : " widgets") + " on the desktop"
            Rectangle {
                width: editLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: Theme.accent
                Text { id: editLabel; anchors.centerIn: parent; text: "Edit…"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        Panels.settingsOpen = false;
                        Panels.screen = Quickshell.screens[0];
                        Panels.widgetEditing = true;
                    }
                }
            }
        }
        Row {
            label: "Files app"
            detail: Files.installed.length === 0 ? "No file manager installed (e.g. sudo pacman -S nautilus)" : "Opens folders, Downloads and the Trash"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" + (Files.installed[0] ? " (" + Files.installed[0].name + ")" : "") }].concat(Files.installed.map(m => ({ value: m.bin, label: m.name })))
                value: Config.options.desktop.fileManager
                onChosen: v => Config.options.desktop.fileManager = v
            }
        }
    }

    Group {
        width: parent.width
        title: "Weather"
        footer: Weather.ready ? "Showing " + Weather.place + ": " + Weather.temp + "°, " + Weather.condition.toLowerCase() + "." : (Weather.error || "Loading…")
        Row {
            label: "Location"
            detail: "Leave empty to detect it from your network."
            TextField {
                width: 220
                text: Config.options.weather.location
                placeholder: "City, e.g. Bengaluru"
                onCommitted: t => Config.options.weather.location = t.trim()
            }
        }
        Row {
            label: "Units"
            Segmented {
                width: 200
                options: ["Celsius", "Fahrenheit"]
                current: Config.options.weather.units === "imperial" ? 1 : 0
                onSelected: i => Config.options.weather.units = i === 1 ? "imperial" : "metric"
            }
        }
    }

    readonly property var cornerActions: [{ value: "none", label: "None" }, { value: "overview", label: "Overview" }, { value: "missionControl", label: "All Windows" }, { value: "desktop", label: "Show desktop" }, { value: "notificationCenter", label: "Notification Center" }, { value: "launchpad", label: "App Drawer" }, { value: "lock", label: "Lock screen" }, { value: "standby", label: "Start Bedside Mode" }, { value: "quickNote", label: "Quick note" }]
    Group {
        width: parent.width
        title: "Hot corners"
        footer: "Move the pointer into a corner and hold it there. Corners do nothing while a full-screen window is in front."
        Row {
            label: "Top left"
            Dropdown {
                width: 220
                options: root.cornerActions
                value: Config.options.hotCorners.topLeft
                onChosen: v => Config.options.hotCorners.topLeft = v
            }
        }
        Row {
            label: "Top right"
            Dropdown {
                width: 220
                options: root.cornerActions
                value: Config.options.hotCorners.topRight
                onChosen: v => Config.options.hotCorners.topRight = v
            }
        }
        Row {
            label: "Bottom left"
            Dropdown {
                width: 220
                options: root.cornerActions
                value: Config.options.hotCorners.bottomLeft
                onChosen: v => Config.options.hotCorners.bottomLeft = v
            }
        }
        Row {
            label: "Bottom right"
            Dropdown {
                width: 220
                options: root.cornerActions
                value: Config.options.hotCorners.bottomRight
                onChosen: v => Config.options.hotCorners.bottomRight = v
            }
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        // ── Drop shelf (services/Shelf) ─────────────────────────────────────
        Group {
            width: parent.width
            title: "Drop shelf"
            footer: "A place to park files while you get to where they're going: drop them on the shelf, then drag them out again, one at a time or all together. Super+Alt+D opens it too."
            Row {
                label: "Shake while dragging"
                detail: "Shake the pointer quickly from side to side with files held"
                Switch { checked: Config.options.shelf.shake; onToggled: Config.options.shelf.shake = !checked }
            }
            Row {
                label: "Drag to the side of the screen"
                detail: "Hold the files at the left or right edge for a moment"
                Switch { checked: Config.options.shelf.edge; onToggled: Config.options.shelf.edge = !checked }
            }
            Row {
                label: "Menu bar icon"
                detail: "While the shelf holds something, and while you drag: drop files on it"
                Switch { checked: Config.options.shelf.menubar; onToggled: Config.options.shelf.menubar = !checked }
            }
            Row {
                label: "Open the shelf"
                detail: Shelf.count > 0 ? Shelf.count + (Shelf.count === 1 ? " item on it" : " items on it") : "Empty"
                SettingsButton {
                    text: Shelf.opened ? "Hide Shelf" : "Show Shelf"
                    onClicked: Shelf.opened ? Shelf.hide() : Shelf.show("settings", null)
                }
            }
        }
    }

}
