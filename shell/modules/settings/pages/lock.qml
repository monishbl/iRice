import QtQuick
import Quickshell
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
        title: "Transitions"
        Row {
            label: "Unlocking"
            detail: Config.options.lock.unlockTransition === "zoom" ? "The lock lifts away, widgets and icons zoom into place and windows fade in, like iPhone" : Config.options.lock.unlockTransition === "fade" ? "The lock fades away and the desktop fades in" : ""
            Choice { key: "unlockTransition"; values: ["zoom", "fade", "none"]; options: ["Zoom", "Fade", "None"]; width: 240 }
        }
        Row {
            label: "Locking"
            detail: Config.options.lock.lockTransition === "zoom" ? "The lock screen settles into place as it appears" : Config.options.lock.lockTransition === "fade" ? "The clock and cards fade in" : ""
            Choice { key: "lockTransition"; values: ["zoom", "fade", "none"]; options: ["Zoom", "Fade", "None"]; width: 240 }
        }
        Row {
            label: "Into StandBy"
            detail: Config.options.lock.standbyTransition === "morph" ? "The clock glides into place and thins" : ""
            enabled: Config.options.lock.standby
            Choice { key: "standbyTransition"; values: ["morph", "fade", "none"]; options: ["Morph", "Fade", "None"]; width: 240 }
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
        Row {
            label: "Wake with the mouse"
            detail: "Off: only a key or a click brings the lock screen back"
            enabled: Config.options.lock.standby
            Switch { checked: Config.options.lock.standbyWakeOnMouse; onToggled: Config.options.lock.standbyWakeOnMouse = !checked }
        }
        Row {
            label: "Clock brightness"
            detail: Math.round(Config.options.lock.standbyClockOpacity * 100) + "%"
            enabled: Config.options.lock.standby
            Slider {
                width: 220
                from: 0.2
                to: 1
                stepSize: 0.05
                value: Config.options.lock.standbyClockOpacity
                onMoved: v => Config.options.lock.standbyClockOpacity = v
            }
        }
        Row {
            label: "Clock weight"
            enabled: Config.options.lock.standby
            Choice { key: "standbyClockWeight"; values: ["thin", "regular", "bold"]; options: ["Thin", "Regular", "Bold"]; width: 240 }
        }
    }

    // Several displays: which show StandBy (the rest go dark), and whether
    // their clocks keep to the same spot
    Group {
        width: parent.width
        visible: Quickshell.screens.length > 1
        enabled: Config.options.lock.standby
        title: "StandBy on displays"
        footer: "Displays that are off stay dark while StandBy shows."
        Row {
            label: "Same clock position"
            detail: "Every display moves its clock to the same spot"
            Switch { checked: Config.options.lock.standbySameClock; onToggled: Config.options.lock.standbySameClock = !checked }
        }
        Repeater {
            model: Quickshell.screens
            Row {
                id: screenRow
                required property var modelData
                readonly property string name: modelData.name
                readonly property var list: Config.options.lock.standbyScreens
                // An empty list means every display
                readonly property bool on: list.length === 0 || list.includes(name)
                readonly property string model: modelData.model ?? ""
                label: model !== "" ? model + "  ·  " + name : name
                detail: modelData.width + " × " + modelData.height
                Switch {
                    checked: screenRow.on
                    onToggled: {
                        const row = screenRow;
                        const all = Quickshell.screens.map(s => s.name);
                        let next = row.list.length === 0 ? all : row.list.slice();
                        next = row.on ? next.filter(n => n !== row.name) : next.concat([row.name]);
                        // Every display on: back to "all" (so new displays join in)
                        if (all.every(n => next.includes(n)))
                            next = [];
                        Config.options.lock.standbyScreens = next;
                    }
                }
            }
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
