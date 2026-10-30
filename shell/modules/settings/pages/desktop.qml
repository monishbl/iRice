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
        footer: "Drag a widget to move it; it snaps to the grid. Right-click a widget to change its size or remove it, or right-click the desktop."
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
            label: "Photos folder"
            detail: "For the Photos widget. Leave empty for ~/Pictures."
            TextField {
                width: 220
                text: Config.options.desktop.photosFolder
                placeholder: "~/Pictures"
                onCommitted: t => Config.options.desktop.photosFolder = t.trim().replace(/^~/, Quickshell.env("HOME"))
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
        Row {
            label: "World clock cities"
            detail: "City|Area/Zone, separated by commas (e.g. Mumbai|Asia/Kolkata)."
            TextField {
                width: 220
                text: Config.options.desktop.worldClocks.join(", ")
                onCommitted: t => Config.options.desktop.worldClocks = t.split(",").map(x => x.trim()).filter(x => x !== "")
            }
        }
        Row {
            label: "Stocks"
            detail: "Symbols for the Stocks widget, separated by commas (e.g. AAPL, ^GSPC, BTC-USD, RELIANCE.NS)."
            TextField {
                width: 220
                text: Config.options.desktop.stocks.join(", ")
                onCommitted: t => Config.options.desktop.stocks = t.split(",").map(x => x.trim().toUpperCase()).filter(x => x !== "")
            }
        }
        Row {
            label: "Smart Rotate"
            detail: "Smart Stacks bring forward what matters now: music, the next event, a low battery."
            Switch {
                checked: Config.options.desktop.smartRotate
                onToggled: Config.options.desktop.smartRotate = !checked
            }
        }
        Row {
            label: "Resource graphs"
            detail: "How the System widget, the island and menu bar popovers and Control Center draw CPU, memory and network."
            Segmented {
                width: 200
                options: ["Area", "Bars", "Line"]
                current: Math.max(0, ["area", "bars", "line"].indexOf(Config.options.resources.graphStyle))
                onSelected: i => Config.options.resources.graphStyle = ["area", "bars", "line"][i]
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

    Group {
        width: parent.width
        title: "Default apps"
        footer: "Used by keyboard shortcuts and the shell. Automatic picks the first one installed. The Files app is set above."
        Row {
            label: "Terminal"
            detail: "Super+Return"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "gnome-terminal", "kgx", "ptyxis", "xfce4-terminal", "xterm"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.terminal
                onChosen: v => Config.options.defaultApps.terminal = v
            }
        }
        Row {
            label: "Browser"
            detail: "Super+W"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic (system default)" }].concat(["zen-browser", "firefox", "google-chrome-stable", "chromium", "brave"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.browser
                onChosen: v => Config.options.defaultApps.browser = v
            }
        }
        Row {
            label: "Code editor"
            detail: "Super+C"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["code", "codium", "zed", "cursor", "windsurf"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.code
                onChosen: v => Config.options.defaultApps.code = v
            }
        }
        Row {
            label: "Text editor"
            detail: "Super+X"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["gnome-text-editor", "kate", "mousepad"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.text
                onChosen: v => Config.options.defaultApps.text = v
            }
        }
        Row {
            label: "Volume mixer"
            detail: "Ctrl+Super+V"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["pavucontrol", "pavucontrol-qt"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.mixer
                onChosen: v => Config.options.defaultApps.mixer = v
            }
        }
        Row {
            label: "Task manager"
            detail: "Ctrl+Shift+Escape"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["gnome-system-monitor", "plasma-systemmonitor", "kitty -e btop", "kitty -e htop"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.tasks
                onChosen: v => Config.options.defaultApps.tasks = v
            }
        }
    }

    readonly property var cornerActions: [{ value: "none", label: "None" }, { value: "overview", label: "Overview" }, { value: "missionControl", label: "Mission Control" }, { value: "desktop", label: "Show desktop" }, { value: "notificationCenter", label: "Notification Center" }, { value: "launchpad", label: "App Library" }, { value: "lock", label: "Lock screen" }, { value: "standby", label: "Start StandBy" }, { value: "quickNote", label: "Quick note" }]
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

    Group {
        width: parent.width
        SubpageLink {
            icon: "sun-horizon"
            label: "Today"
            detail: "The morning screen: a greeting, your events and to-dos, the weather"
            value: ({ ask: "Every morning", widget: "Widget", both: "Both", off: "Off" })[Today.mode] ?? ""
            page: "desktop/today"
        }
        SubpageLink {
            icon: "record"
            label: "Screen recording"
            detail: "Frame rate, format, quality, sound and where recordings go"
            value: Recorder.gsr ? "" : "Basic"
            page: "desktop/recording"
        }
    }
}
