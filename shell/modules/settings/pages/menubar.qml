import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property var mb: Config.options.menubar
    function toggleIn(list, id) {
        return list.includes(id) ? list.filter(x => x !== id) : list.concat([id]);
    }

    component Toggle: Row {
        id: tr
        required property string key
        property string text
        property string hint
        label: text
        detail: hint
        Switch { checked: page.mb[tr.key]; onToggled: page.mb[tr.key] = !checked }
    }

    Group {
        width: parent.width
        title: "Behaviour"
        Toggle { key: "autoHide"; text: "Automatically hide the menu bar"; hint: "Move the pointer to the top edge to show it." }
        Toggle { key: "background"; text: "Show menu bar background" }
        Toggle { key: "scrollGestures"; text: "Scroll to change brightness and volume"; hint: "Left half: brightness. Right half: volume." }
        Row {
            label: "Screens"
            detail: "Show the menu bar on these screens."
            Flow {
                width: 260
                spacing: 6
                layoutDirection: Qt.RightToLeft
                Repeater {
                    model: Quickshell.screens
                    Rectangle {
                        required property var modelData
                        readonly property bool on: page.mb.screens.length === 0 || page.mb.screens.includes(modelData.name)
                        width: sName.implicitWidth + 20
                        height: 26
                        radius: 13
                        color: on ? Theme.accent : Theme.color.fill
                        Text { id: sName; anchors.centerIn: parent; text: parent.modelData.name; color: parent.on ? Theme.color.onAccent : Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                        MouseArea {
                            anchors.fill: parent
                            onClicked: {
                                const all = Quickshell.screens.map(s => s.name);
                                const cur = page.mb.screens.length === 0 ? all : page.mb.screens;
                                const next = page.toggleIn(cur, parent.modelData.name);
                                page.mb.screens = next.length === all.length ? [] : next;
                            }
                        }
                    }
                }
            }
        }
    }

    Group {
        width: parent.width
        SubpageLink {
            icon: "pill"
            label: "Dynamic island"
            detail: "Live activities, alerts, the music player and what shows when you point at it"
            value: Config.options.island.enabled ? "On" : "Off"
            page: "menubar/island"
        }
    }

    Group {
        width: parent.width
        title: "Left and centre"
        Toggle { key: "windowTitle"; text: "Show window title" }
        Row {
            label: "Spaces"
            Segmented {
                width: 240
                readonly property var values: ["off", "dots", "icons"]
                options: ["Off", "Dots", "App icons"]
                current: Math.max(0, values.indexOf(page.mb.workspaces))
                onSelected: i => page.mb.workspaces = values[i]
            }
        }
        Row {
            visible: page.mb.workspaces !== "off"
            label: "Spaces always shown"
            detail: page.mb.workspaceCount + (page.mb.workspaceCount === 1 ? " space" : " spaces") + "; more appear when used"
            Slider { width: 200; from: 1; to: 10; stepSize: 1; value: page.mb.workspaceCount; onMoved: v => page.mb.workspaceCount = v }
        }
        Row {
            visible: page.mb.workspaces !== "off"
            label: "When there are more spaces"
            detail: page.mb.workspaceOverflow === "dynamic" ? "The next spaces shrink and fade at the ends, and slide along as you move" : "Arrows at the ends; a dot under one means a space past it has windows"
            Segmented {
                width: 220
                readonly property var values: ["arrows", "dynamic"]
                options: ["Arrows", "Dynamic dots"]
                current: Math.max(0, values.indexOf(page.mb.workspaceOverflow))
                onSelected: i => page.mb.workspaceOverflow = values[i]
            }
        }
        Row {
            visible: page.mb.workspaces !== "off"
            label: "Spaces on other displays"
            detail: "Listed with a small mark; click to go there, right-click to move a space to another display"
            Switch { checked: page.mb.otherSpaces; onToggled: page.mb.otherSpaces = !checked }
        }
    }

    Group {
        width: parent.width
        title: "Status items"
        Toggle { key: "battery"; text: "Battery" }
        Row {
            visible: page.mb.battery
            label: "Battery percentage"
            detail: "Inside is cut into the battery. Beside it, it always shows at 20% or less."
            Segmented {
                width: 240
                readonly property var values: ["off", "inside", "outside"]
                options: ["Off", "Inside", "Beside"]
                current: Math.max(0, values.indexOf(({ "true": "outside", "false": "off" })[page.mb.batteryPercent] ?? page.mb.batteryPercent))
                onSelected: i => page.mb.batteryPercent = values[i]
            }
        }
        Row {
            label: "Screen sharing shows"
            detail: !Config.options.island.enabled ? "The island is off, so a share shows as the purple dot." : Config.options.privacy.screenShareIndicator === "island" ? "In the island while an app shares your screen, with a Stop button; no dot meanwhile." : Config.options.privacy.screenShareIndicator === "dot" ? "A purple dot with the other privacy dots; click it to see what's shared or stop it." : "In the island and as a purple dot."
            Segmented {
                width: 240
                readonly property var values: ["island", "dot", "both"]
                options: ["Island", "Dot", "Both"]
                current: Math.max(0, values.indexOf(Config.options.privacy.screenShareIndicator))
                onSelected: i => Config.options.privacy.screenShareIndicator = values[i]
            }
        }
        Toggle { key: "wifi"; text: "Wi-Fi" }
        Toggle { key: "bluetooth"; text: "Bluetooth" }
        Toggle { key: "sound"; text: "Sound" }
        Toggle { key: "resources"; text: "CPU and memory" }
        Toggle { key: "weather"; text: "Weather" }
        Toggle { key: "keyboardLayout"; text: "Keyboard layout"; hint: "Shown when you have more than one layout." }
        Toggle { key: "search"; text: "Search"; hint: "Spotlight still opens with " + KeyNames.combo("SUPER Space") + "." }
        Toggle { key: "ai"; text: "AI"; hint: "The sidebar still opens with " + KeyNames.combo("SUPER A") + "." }
        Toggle { key: "controlCenter"; text: "Control Center"; hint: "Still opens with " + KeyNames.combo("SUPER C") + "." }
        Toggle { key: "clock"; text: "Clock and notifications"; hint: "Notification Center still opens with " + KeyNames.combo("SUPER N") + "." }
        Toggle { key: "appMenu"; text: "App name menu"; hint: "On the left, next to the system menu." }
        Toggle { key: "windowMenu"; text: "Window menu"; hint: "On the left; window actions are also in " + KeyNames.combo("SUPER CTRL F") + " and friends." }
        Row {
            label: "Unread notifications"
            detail: "Shown next to the clock; hidden while a Focus mode is on."
            Segmented {
                width: 320
                readonly property var values: ["bellDot", "dot", "count", "bellCount", "off"]
                options: ["Bell + dot", "Dot", "Count", "Bell + count", "Off"]
                current: Math.max(0, values.indexOf(page.mb.notificationIndicator))
                onSelected: i => page.mb.notificationIndicator = values[i]
            }
        }
    }

    Group {
        width: parent.width
        title: "Buttons"
        Repeater {
            model: [
                ["screenshot", "Screenshot tools"], ["colorPicker", "Colour picker"], ["mic", "Microphone mute"],
                ["keyboard", "On-screen keyboard"], ["darkMode", "Dark mode"], ["powerProfile", "Energy mode"], ["record", "Screen recording"]
            ]
            Row {
                id: btn
                required property var modelData
                label: modelData[1]
                Switch {
                    checked: page.mb.utilButtons.includes(btn.modelData[0])
                    onToggled: page.mb.utilButtons = page.toggleIn(page.mb.utilButtons, btn.modelData[0])
                }
            }
        }
    }

    Group {
        width: parent.width
        title: "Clock"
        Toggle { key: "showDate"; text: "Show date" }
        Toggle { key: "showSeconds"; text: "Show seconds" }
    }
}
