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
    }

    Group {
        width: parent.width
        title: "Status items"
        // One choice: off, or the battery with its percentage where you
        // want it ("true"/"false" from before mean beside/none)
        Row {
            label: "Battery"
            detail: !page.mb.battery ? "" : "The percentage inside is cut into the battery; with the icon alone, it shows beside it at 20% or less."
            Segmented {
                width: 300
                readonly property string percent: ({ "true": "outside", "false": "off" })[page.mb.batteryPercent] ?? page.mb.batteryPercent
                readonly property var values: ["off", "icon", "inside", "outside"]
                options: ["Off", "Icon", "% inside", "% beside"]
                current: !page.mb.battery ? 0 : Math.max(1, values.indexOf(percent))
                onSelected: i => {
                    if (i > 0)
                        page.mb.batteryPercent = i === 1 ? "off" : values[i];
                    page.mb.battery = i > 0;
                }
            }
        }
        Toggle { key: "wifi"; text: "Wi-Fi" }
        Toggle { key: "bluetooth"; text: "Bluetooth" }
        Toggle { key: "sound"; text: "Sound" }
        Toggle { key: "resources"; text: "CPU and memory" }
        Toggle { key: "weather"; text: "Weather" }
        Toggle { key: "keyboardLayout"; text: "Keyboard layout"; hint: "Shown when you have more than one layout." }
        Toggle { key: "collapseTray"; text: "Tuck away app icons"; hint: "The icons apps put in the menu bar wait behind an arrow; click it to show them." }
        Toggle { key: "search"; text: "Search"; hint: "Spotlight still opens with " + KeyNames.combo("SUPER Space") + "." }
        Toggle { key: "ai"; text: "AI"; hint: "The sidebar still opens with " + KeyNames.combo("SUPER A") + "." }
        Toggle { key: "controlCenter"; text: "Control Center"; hint: "Still opens with " + KeyNames.combo("SUPER C") + "." }
        Toggle { key: "clock"; text: "Clock and notifications"; hint: "Notification Center still opens with " + KeyNames.combo("SUPER N") + "." }
        Toggle { key: "appMenu"; text: "App name menu"; hint: "On the left, next to the system menu." }
        Toggle { key: "windowMenu"; text: "Window menu"; hint: "On the left; window actions are also in " + KeyNames.combo("SUPER CTRL F") + " and friends." }
    }

    Group {
        width: parent.width
        title: "Clock"
        Toggle { key: "showDate"; text: "Show date" }
        Toggle { key: "showSeconds"; text: "Show seconds" }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            title: "Behaviour"
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
            title: "Left and centre"
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
                detail: "Listed after this display's own, under their display's name; right-click a space to move it to another display"
                Switch { checked: page.mb.otherSpaces; onToggled: page.mb.otherSpaces = !checked }
            }
        }
        Group {
            width: parent.width
            title: "Status items"
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
    }

    // ── Parts: other pages shown here (SettingsMap.parts) ──────────────
    PagePart {
        title: "Dock"
        source: Qt.resolvedUrl("dock.qml")
    }
    PagePart {
        title: "Control Center"
        source: Qt.resolvedUrl("controlcenter.qml")
    }
}
