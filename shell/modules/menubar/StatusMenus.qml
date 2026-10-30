import QtQuick
import Quickshell
import Quickshell.Services.UPower
import qs.common
import qs.glass
import qs.modules.controlcenter
import qs.modules.network
import qs.services

// Menus behind the menu bar's status items: Wi-Fi, Bluetooth, Sound, battery,
// resources and weather. Each is a glass popover under its item.
Scope {
    component StatusMenu: Variants {
        id: menu
        required property string panel
        property int width_: 300
        property Component body
        model: Quickshell.screens

        PanelLoader {
            id: slot
            required property var modelData
            panel: menu.panel
            screen: modelData

            Popover {
                id: pop
                screen: slot.modelData
                popoverName: menu.panel
                lens: 0.35 // a menu right under the menu bar (see MenuPopover)
                autoClose: 5000
                contentWidth: menu.width_
                targetX: Panels.anchorX - 20
                Loader {
                    width: pop.contentWidth
                    sourceComponent: menu.body
                }
            }
        }
    }

    StatusMenu {
        panel: "wifiMenu"
        width_: 300
        body: Component {
            Column {
                spacing: 12
                WifiPage { width: parent.width; showBack: false }
                Rectangle { width: parent.width; height: 1; color: Theme.color.separator }
                SpeedTestCard { width: parent.width; compact: true }
            }
        }
    }
    StatusMenu {
        panel: "bluetoothMenu"
        width_: 300
        body: Component { BluetoothPage { showBack: false } }
    }
    StatusMenu {
        panel: "soundMenu"
        width_: 300
        body: Component {
            Column {
                spacing: 12
                CCSlider {
                    width: parent.width
                    label: "Sound"
                    icon: Audio.icon
                    value: Audio.muted ? 0 : Audio.volume
                    onMoved: v => Audio.setVolume(v)
                }
                SoundPage {
                    width: parent.width
                    showBack: false
                    title: ""
                }
            }
        }
    }

    // Who uses the microphone, camera or screen right now
    StatusMenu {
        panel: "privacyMenu"
        width_: 260
        body: Component {
            Column {
                spacing: 10
                Text {
                    text: "In use"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                }
                Text {
                    visible: !Privacy.any
                    text: "Nothing is using your microphone, camera, location or screen."
                    width: parent.width
                    wrapMode: Text.Wrap
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Repeater {
                    model: [
                        ...Privacy.cameraApps.map(a => ({ app: a, icon: "video-camera", what: "Camera", color: "#30D158" })),
                        ...Privacy.micApps.map(a => ({ app: a, icon: "microphone", what: "Microphone", color: "#FF9F0A" })),
                        ...Privacy.locationApps.map(a => ({ app: a, icon: "navigation-arrow", what: "Location", color: "#0A84FF" })),
                        // Shared screens, listed here whichever way they're shown
                        ...Privacy.shares.map(x => ({ app: x.app, icon: x.window ? "app-window" : "screencast", what: "Sharing " + x.target, color: "#BF5AF2", stop: true }))
                    ]
                    Row {
                        required property var modelData
                        spacing: 10
                        Rectangle {
                            width: 28
                            height: 28
                            radius: 14
                            color: Qt.alpha(parent.modelData.color, 0.18)
                            Icon {
                                anchors.centerIn: parent
                                name: parent.parent.modelData.icon
                                fill: true
                                size: 15
                                color: parent.parent.modelData.color
                            }
                        }
                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            Text {
                                text: parent.parent.modelData.app
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                            }
                            Text {
                                text: parent.parent.modelData.what
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                    }
                }
                // Stop sharing (every shared screen at once)
                Rectangle {
                    visible: Privacy.screenShared
                    width: stopLabel.implicitWidth + 28
                    height: 28
                    radius: 14
                    color: stopMouse.pressed ? Qt.darker(Theme.color.danger, 1.15) : Theme.color.danger
                    opacity: Privacy.stopping ? 0.6 : 1
                    Text {
                        id: stopLabel
                        anchors.centerIn: parent
                        text: Privacy.stopping ? "Stopping…" : "Stop sharing"
                        color: "white"
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                    }
                    MouseArea {
                        id: stopMouse
                        anchors.fill: parent
                        enabled: !Privacy.stopping
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Privacy.stopSharing()
                    }
                }
                Text {
                    visible: Privacy.stopFailed
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: Privacy.screenApp + " is still sharing. Stop it in " + Privacy.screenApp + "."
                    color: Theme.color.danger
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
            }
        }
    }

    StatusMenu {
        panel: "batteryMenu"
        width_: 280
        body: Component {
            Column {
                id: bat
                readonly property var d: UPower.displayDevice
                readonly property real level: d.percentage > 1 ? d.percentage / 100 : d.percentage
                readonly property bool charging: d.state === UPowerDeviceState.Charging
                readonly property bool full: d.state === UPowerDeviceState.FullyCharged
                Component.onCompleted: BatteryHealth.refresh() // (the menu is made each time it opens)
                function hm(secs) {
                    const m = Math.round(secs / 60);
                    return m >= 60 ? Math.floor(m / 60) + " h " + (m % 60) + " min" : m + " min";
                }
                spacing: 10

                Item {
                    width: parent.width
                    height: 26
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Battery"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                    }
                    Text {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        text: Math.round(bat.level * 100) + "%"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold; features: { "tnum": 1 } }
                    }
                }
                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: bat.full ? "Fully charged" : bat.charging ? (bat.d.timeToFull > 0 ? "Charging · full in " + bat.hm(bat.d.timeToFull) : "Charging") : (bat.d.timeToEmpty > 0 ? "On battery · " + bat.hm(bat.d.timeToEmpty) + " left" : "On battery")
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                // Power going in or out right now
                Text {
                    visible: BatteryHealth.watts > 0.05 && !bat.full
                    text: (bat.charging ? "Charging at " : "Using ") + BatteryHealth.watts.toFixed(1) + " W"
                    color: Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                }

                // Battery health, like macOS: how much of its design capacity
                // it still holds, its cycles, and whether it needs service
                Text {
                    visible: BatteryHealth.capacity >= 0 || bat.d.healthSupported
                    text: "Battery health"
                    color: Theme.color.secondary
                    topPadding: 4
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                }
                Repeater {
                    model: [
                        { label: "Maximum capacity", value: BatteryHealth.capacity >= 0 ? Math.round(BatteryHealth.capacity * 100) + "%" : bat.d.healthSupported ? Math.round(bat.d.healthPercentage) + "%" : "" },
                        { label: "Cycle count", value: BatteryHealth.cycles > 0 ? String(BatteryHealth.cycles) : "" },
                        { label: "Condition", value: BatteryHealth.condition !== "" ? BatteryHealth.condition : bat.d.healthSupported ? (bat.d.healthPercentage >= 80 ? "Normal" : "Service recommended") : "" }
                    ].filter(r => r.value !== "")
                    Item {
                        required property var modelData
                        width: bat.width
                        height: 20
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: parent.modelData.label
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                        Text {
                            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                            text: parent.modelData.value
                            color: parent.modelData.value === "Service recommended" ? Theme.color.warning : Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                        }
                    }
                }
                Text {
                    text: "Energy mode"
                    color: Theme.color.secondary
                    topPadding: 4
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                }
                // (without power-profiles-daemon: who manages power instead)
                Text {
                    visible: !Backends.ppd
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: Backends.powerShort
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Segmented {
                    visible: Backends.ppd
                    width: parent.width
                    readonly property var values: [PowerProfile.PowerSaver, PowerProfile.Balanced, PowerProfile.Performance]
                    options: PowerProfiles.hasPerformanceProfile ? ["Low Power", "Balanced", "Performance"] : ["Low Power", "Balanced"]
                    current: Math.max(0, values.indexOf(PowerProfiles.profile))
                    onSelected: i => Power.setMode(Power.modeOf(values[i]))
                }
            }
        }
    }

    StatusMenu {
        panel: "resourcesMenu"
        width_: 280
        body: Component {
            Column {
                spacing: 12
                Repeater {
                    model: [
                        { label: "CPU", value: Resources.cpu, detail: Math.round(Resources.cpu * 100) + "%" + (Resources.temperature > 0 ? " · " + Math.round(Resources.temperature) + "°C" : "") },
                        { label: "Memory", value: Resources.memory, detail: Resources.memUsedGb.toFixed(1) + " of " + Resources.memTotalGb.toFixed(1) + " GB" },
                        { label: "Swap", value: Resources.swap, detail: Math.round(Resources.swap * 100) + "%" }
                    ]
                    Column {
                        id: metric
                        required property var modelData
                        width: parent.width
                        spacing: 5
                        Item {
                            width: parent.width
                            height: 16
                            Text { text: metric.modelData.label; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                            Text { anchors.right: parent.right; text: metric.modelData.detail; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } } }
                        }
                        Rectangle {
                            width: parent.width
                            height: 6
                            radius: 3
                            color: Theme.color.toggleOff
                            Rectangle {
                                width: parent.width * Math.min(1, metric.modelData.value)
                                height: parent.height
                                radius: 3
                                color: metric.modelData.value > 0.9 ? Theme.color.danger : Theme.accent
                                Behavior on width { NumberAnimation { duration: Theme.motion.popover } }
                            }
                        }
                    }
                }
            }
        }
    }

    StatusMenu {
        panel: "weatherMenu"
        width_: 300
        body: Component {
            Column {
                spacing: 8
                Text {
                    text: Weather.place
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                }
                Row {
                    spacing: 10
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: Weather.icon; fill: true; size: 34 }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: Weather.temp + "°"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: 40; features: { "tnum": 1 } }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        Text { text: Weather.condition; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                        Text { text: "Feels like " + Weather.feelsLike + "° · H:" + Weather.high + "° L:" + Weather.low + "°"; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.small } }
                    }
                }
                Row {
                    width: parent.width
                    topPadding: 6
                    Repeater {
                        model: Weather.hourly
                        Column {
                            id: h
                            required property var modelData
                            width: parent.width / Math.max(1, Weather.hourly.length)
                            spacing: 3
                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: h.modelData.time; color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.caption } }
                            Icon { anchors.horizontalCenter: parent.horizontalCenter; name: h.modelData.icon; fill: true; size: 16 }
                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: h.modelData.temp + "°"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } } }
                        }
                    }
                }
            }
        }
    }
}
