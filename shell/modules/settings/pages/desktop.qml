import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

Column {
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
            label: "World clock cities"
            detail: "City|Area/Zone, separated by commas (e.g. Mumbai|Asia/Kolkata)."
            TextField {
                width: 220
                text: Config.options.desktop.worldClocks.join(", ")
                onCommitted: t => Config.options.desktop.worldClocks = t.split(",").map(x => x.trim()).filter(x => x !== "")
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
}
