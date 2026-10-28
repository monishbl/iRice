import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22
    readonly property string pageTitle: "Dynamic island"

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
            detail: Config.options.island.playerLayout === "side" ? "Small album on the left, the song and its progress beside it" : Config.options.island.playerArt === "faded" ? "The album art fades out behind the controls" : "The album art fills the island and melts into its glass"
            Segmented {
                width: 240
                readonly property var values: ["island", "faded", "small"]
                options: ["Island", "Faded", "Small"]
                current: Config.options.island.playerLayout === "side" ? 2 : Math.max(0, values.indexOf(Config.options.island.playerArt))
                onSelected: i => {
                    if (values[i] === "small") {
                        Config.options.island.playerLayout = "side";
                    } else {
                        Config.options.island.playerArt = values[i];
                        Config.options.island.playerLayout = "overlay";
                    }
                }
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
                { kind: "call", label: "Calls show", pieces: [["icon", "Icon"], ["name", "App"], ["time", "Duration"], ["mute", "Mute"], ["open", "Open"], ["leave", "Leave"]] },
                { kind: "screenShare", label: "Screen sharing shows", pieces: [["icon", "Icon"], ["name", "App"], ["time", "Duration"], ["stop", "Stop"]] },
                { kind: "sysupdate", label: "System updates show", pieces: [["icon", "Icon"], ["name", "Updating system"], ["time", "Progress"], ["details", "Details"], ["cancel", "Cancel"]] }
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
                            color: on ? Theme.accentTint : chipMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
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
            model: Config.options.island.enabled ? [["music", "New song"], ["charging", "Charger connected or removed"], ["battery", "Low battery"], ["timers", "Focus timer"], ["recording", "Screen recording"], ["focus", "Focus mode"], ["bluetooth", "Bluetooth devices connect or disconnect"], ["usb", "USB devices and drives"], ["displays", "Displays connect or disconnect"], ["audio", "Sound moves to another output"], ["network", "Wi-Fi, VPN, hotspot and Ethernet changes"], ["transfers", "Downloads and file copies (live)"], ["calls", "Calls and meetings (live)"], ["sysupdate", "System updates (live, and when they finish)"], ["resources", "CPU or memory stays high"], ["temperature", "CPU running hot (alert, and the island turns red)"], ["notifications", "Notifications as cards in the island (instead of banners)"]] : []
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
}
