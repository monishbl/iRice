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
        title: "Dynamic island"
        footer: "The spaces at the top centre become an island: it widens for something ongoing (music, a timer, recording) and springs open for a moment when something happens. Point at it to see what's live."
        Row {
            label: "Dynamic island"
            Switch { checked: Config.options.island.enabled; onToggled: Config.options.island.enabled = !checked }
        }
        Row {
            visible: Config.options.island.enabled
            label: "Live activities"
            detail: "Music, timers and recording when the island is closed"
            Segmented {
                width: 270
                readonly property var values: ["replace", "split", "beside"]
                options: ["Replace spaces", "Around", "Beside"]
                current: Math.max(0, values.indexOf(Config.options.island.activityLayout))
                onSelected: i => Config.options.island.activityLayout = values[i]
            }
        }
        Row {
            visible: Config.options.island.enabled
            label: "When several are live"
            detail: Config.options.island.activityMode === "all" ? "The open island lists them all" : "One at a time; scroll over the island to switch"
            Segmented {
                width: 200
                readonly property var values: ["one", "all"]
                options: ["One at a time", "All"]
                current: Math.max(0, values.indexOf(Config.options.island.activityMode))
                onSelected: i => Config.options.island.activityMode = values[i]
            }
        }
        Row {
            visible: Config.options.island.enabled
            label: "Music player"
            detail: "In the open island"
            Segmented {
                width: 220
                readonly property var values: ["overlay", "side"]
                options: ["Over the art", "Beside the art"]
                current: Math.max(0, values.indexOf(Config.options.island.playerLayout))
                onSelected: i => Config.options.island.playerLayout = values[i]
            }
        }
        // What each live activity shows when the island is closed
        Repeater {
            model: Config.options.island.enabled ? [
                { kind: "music", label: "Music shows", pieces: [["art", "Album art"], ["title", "Title"], ["scroll", "Scroll long titles"], ["bars", "Playing bars"], ["prev", "Previous"], ["play", "Play/Pause"], ["next", "Next"]] },
                { kind: "timer", label: "Focus timer shows", pieces: [["icon", "Icon"], ["label", "Focus/Break"], ["time", "Time left"], ["play", "Pause"], ["skip", "Skip"]] },
                { kind: "stopwatch", label: "Stopwatch shows", pieces: [["icon", "Icon"], ["time", "Time"], ["play", "Pause"], ["reset", "Reset"]] },
                { kind: "recording", label: "Recording shows", pieces: [["icon", "Red dot"], ["time", "Time"], ["stop", "Stop"]] },
                { kind: "transfer", label: "Transfers show", pieces: [["icon", "Icon"], ["name", "File name"], ["time", "Progress"], ["open", "Open Downloads"], ["cancel", "Cancel copy"]] },
                { kind: "call", label: "Calls show", pieces: [["icon", "Icon"], ["name", "App"], ["time", "Duration"], ["mute", "Mute"], ["open", "Open"], ["leave", "Leave"]] }
            ] : []
            Row {
                id: pieceRow
                required property var modelData
                label: modelData.label
                Flow { // (Row here is the settings row)
                    width: 320
                    spacing: 6
                    Repeater {
                        model: pieceRow.modelData.pieces
                        Rectangle {
                            id: chip
                            required property var modelData
                            readonly property string key: pieceRow.modelData.kind + "." + modelData[0]
                            readonly property bool on: { Config.options.island.livePieces; Config.options.island.known; return Island.shows(pieceRow.modelData.kind, modelData[0]); }
                            width: chipText.implicitWidth + 20
                            height: 26
                            radius: 13
                            color: on ? Qt.alpha(Theme.accent, 0.28) : chipMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                            border.width: on ? 1 : 0
                            border.color: Theme.accent
                            Text {
                                id: chipText
                                anchors.centerIn: parent
                                text: chip.modelData[1]
                                color: chip.on ? Theme.color.label : Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: chip.on ? Font.Medium : Font.Normal }
                            }
                            MouseArea {
                                id: chipMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: Island.setShows(pieceRow.modelData.kind, chip.modelData[0], !chip.on)
                            }
                        }
                    }
                }
            }
        }
        Row {
            visible: Config.options.island.enabled && Island.wants("temperature")
            label: "Running hot at"
            detail: Config.options.island.hotTemp + " °C · the CPU package is " + (Resources.temperature > 0 ? Math.round(Resources.temperature) + " °C now" : "not reporting")
            Slider {
                width: 220
                from: 70
                to: 100
                stepSize: 1
                value: Config.options.island.hotTemp
                onMoved: v => Config.options.island.hotTemp = Math.round(v)
            }
        }
        Row {
            visible: Config.options.island.enabled
            label: "Keep the island open"
            detail: "Also with the pin in the island, or right-click it"
            Switch { checked: Config.options.island.pinned; onToggled: Config.options.island.pinned = !checked }
        }
        Row {
            visible: Config.options.island.enabled
            label: "Volume and brightness in the island"
            detail: "Off: a separate HUD below the menu bar"
            Switch { checked: Config.options.island.hud; onToggled: Config.options.island.hud = !checked }
        }
        Repeater {
            model: Config.options.island.enabled ? [["music", "New song"], ["charging", "Charger connected or removed"], ["battery", "Low battery"], ["timers", "Focus timer"], ["recording", "Screen recording"], ["focus", "Focus mode"], ["bluetooth", "Bluetooth devices connect or disconnect"], ["usb", "USB devices and drives"], ["displays", "Displays connect or disconnect"], ["audio", "Sound moves to another output"], ["network", "Wi-Fi, VPN, hotspot and Ethernet changes"], ["transfers", "Downloads and file copies (live)"], ["calls", "Calls and meetings (live)"], ["resources", "CPU or memory stays high"], ["temperature", "CPU running hot (alert, and the island turns red)"], ["notifications", "Notifications as cards in the island (instead of banners)"]] : []
            Row {
                id: ev
                required property var modelData
                label: modelData[1]
                Switch {
                    checked: Island.wants(ev.modelData[0])
                    onToggled: {
                        const k = ev.modelData[0];
                        const l = Config.options.island.events.filter(x => x !== k);
                        Config.options.island.events = Island.wants(k) ? l : l.concat([k]);
                        if (!Config.options.island.known.includes(k))
                            Config.options.island.known = Config.options.island.known.concat([k]);
                    }
                }
            }
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
    }

    Group {
        width: parent.width
        title: "Status items"
        Toggle { key: "battery"; text: "Battery" }
        Toggle { key: "batteryPercent"; text: "Battery percentage"; hint: "Always shown below 20%." }
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
