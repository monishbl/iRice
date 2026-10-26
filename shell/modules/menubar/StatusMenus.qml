import QtQuick
import Quickshell
import Quickshell.Services.UPower
import qs.common
import qs.glass
import qs.modules.controlcenter
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
        body: Component { WifiPage { showBack: false } }
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
                Text {
                    visible: bat.d.healthSupported
                    text: "Battery health " + Math.round(bat.d.healthPercentage) + "%"
                    color: Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Text {
                    text: "Energy mode"
                    color: Theme.color.secondary
                    topPadding: 4
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                }
                Segmented {
                    width: parent.width
                    readonly property var values: [PowerProfile.PowerSaver, PowerProfile.Balanced, PowerProfile.Performance]
                    options: PowerProfiles.hasPerformanceProfile ? ["Low Power", "Balanced", "Performance"] : ["Low Power", "Balanced"]
                    current: Math.max(0, values.indexOf(PowerProfiles.profile))
                    onSelected: i => PowerProfiles.profile = values[i]
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
