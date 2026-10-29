import QtQuick
import QtQuick as QQ
import Quickshell
import Quickshell.Services.UPower
import qs.common
import qs.modules.settings
import qs.services

// Battery & Power: the energy mode and who decided it, low and critical
// battery, idle timers on the charger and on battery, the lid, the password
// after sleep, and which apps used the most energy (services/Power.qml).
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property var cfg: Config.options.power
    readonly property bool battery: Power.hasBattery

    component Seconds: Dropdown {
        id: dd
        required property string key
        property bool allowNever: true
        width: 160
        options: (allowNever ? [{ value: 0, label: "Never" }] : []).concat([60, 120, 180, 300, 600, 900, 1800, 3600].map(s => ({ value: s, label: s < 3600 ? (s / 60) + (s === 60 ? " minute" : " minutes") : "1 hour" })))
        value: page.cfg[key]
        onChosen: v => Config.options.power[dd.key] = v
    }
    component Lid: Segmented {
        required property string key
        width: 260
        readonly property var values: ["sleep", "lock", "nothing"]
        options: ["Sleep", "Lock", "Do nothing"]
        current: Math.max(0, values.indexOf(page.cfg[key]))
        onSelected: i => Config.options.power[key] = values[i]
    }

    // ── Energy mode ─────────────────────────────────────────────────────
    Group {
        width: parent.width
        title: "Energy mode"
        footer: "Cool-down comes first while the computer is hot, then the battery saver, then your choice or Game Mode, whichever came last."
        Row {
            label: "Energy mode"
            detail: Power.why !== "" ? "Now " + Power.modeName + " · " + Power.why : PowerProfiles.hasPerformanceProfile ? "" : "This computer has no Performance mode"
            Segmented {
                width: PowerProfiles.hasPerformanceProfile ? 330 : 220
                readonly property var values: ["power-saver", "balanced", "performance"]
                options: PowerProfiles.hasPerformanceProfile ? ["Low Power", "Balanced", "Performance"] : ["Low Power", "Balanced"]
                current: Math.max(0, values.indexOf(Power.mode))
                onSelected: i => Power.setMode(values[i])
            }
        }
        Row {
            visible: page.battery
            label: "Offer Low Power when the battery is low"
            detail: "At " + page.cfg.batterySaverLevel + "% on battery, once each time it runs down, the island asks"
            Switch { checked: page.cfg.batterySaver; onToggled: Config.options.power.batterySaver = !checked }
        }
        Row {
            visible: page.battery && page.cfg.batterySaver
            label: "Turn it on automatically"
            detail: "Low Power comes on by itself, and the island offers Undo"
            Switch { checked: page.cfg.batterySaverAuto; onToggled: Config.options.power.batterySaverAuto = !checked }
        }
        Row {
            label: "Game Mode uses Performance"
            detail: PowerProfiles.hasPerformanceProfile ? "While Game Mode is on (Control Center)" : "This computer has no Performance mode"
            enabled: PowerProfiles.hasPerformanceProfile
            Switch { checked: page.cfg.gamePerformance; onToggled: Config.options.power.gamePerformance = !checked }
        }
        Row {
            visible: System.keepAwake
            label: "Keeping awake"
            detail: "Caffeine is on · " + System.keepAwakeStatus.toLowerCase()
            SettingsButton {
                text: "Turn off Caffeine"
                onClicked: System.setKeepAwake(false, 0)
            }
        }
    }

    // ── Battery ─────────────────────────────────────────────────────────
    Group {
        visible: page.battery
        width: parent.width
        title: "Battery"
        footer: Power.guardError !== "" ? Power.guardError : "It sleeps rather than shutting down, so your work is there when you plug in. This keeps working even if iRice isn't running."
        Row {
            label: "Critical battery"
            detail: "Warn at 10% and 5%, sleep at 3% (even with Caffeine on)"
            Switch { checked: page.cfg.critical; onToggled: Config.options.power.critical = !checked }
        }
    }

    // ── Idle ────────────────────────────────────────────────────────────
    Group {
        width: parent.width
        title: page.battery ? "On battery" : "When you're away"
        visible: !Power.ownHypridle
        Row { label: "Dim the screen after"; Seconds { key: page.battery ? "idleDimBattery" : "idleDimAc" } }
        Row { label: "Turn the screen off after"; Seconds { key: page.battery ? "idleOffBattery" : "idleOffAc" } }
        Row { label: "Lock after"; detail: "The password is needed from then on"; Seconds { key: page.battery ? "idleLockBattery" : "idleLockAc" } }
        Row { label: "Sleep after"; Seconds { key: page.battery ? "idleSleepBattery" : "idleSleepAc" } }
    }
    Group {
        width: parent.width
        title: "On the charger"
        visible: page.battery && !Power.ownHypridle
        footer: "Each step comes after the one before it. Apps that play video or calls keep the screen on."
        Row { label: "Dim the screen after"; Seconds { key: "idleDimAc" } }
        Row { label: "Turn the screen off after"; Seconds { key: "idleOffAc" } }
        Row { label: "Lock after"; Seconds { key: "idleLockAc" } }
        Row { label: "Sleep after"; Seconds { key: "idleSleepAc" } }
    }
    Group {
        width: parent.width
        visible: Power.ownHypridle
        title: "When you're away"
        footer: "You have your own hypridle.conf (" + Power.userHypridle.replace(Quickshell.env("HOME"), "~") + "), so its timers are used. Rename or remove it to set them here."
        Row { label: "Using your own idle settings" }
    }

    // ── Lid and password ────────────────────────────────────────────────
    Group {
        width: parent.width
        title: "Lid and waking"
        visible: page.battery
        footer: "Do nothing keeps the computer running with the lid closed; with another display, it carries on there. If iRice isn't running, closing the lid sleeps, as it always has."
        Row { label: "Closing the lid on battery"; Lid { key: "lidBattery" } }
        Row { label: "Closing the lid on the charger"; Lid { key: "lidAc" } }
        Row { label: "With another display connected"; detail: "On the charger"; Lid { key: "lidExternal" } }
        Row {
            label: "Require the password after sleep"
            Segmented {
                width: 330
                readonly property var values: [0, 60, 300]
                options: ["Immediately", "After 1 minute", "After 5 minutes"]
                current: Math.max(0, values.indexOf(page.cfg.passwordAfterSleep))
                onSelected: i => Config.options.power.passwordAfterSleep = values[i]
            }
        }
    }

    // ── Apps using significant energy ───────────────────────────────────
    Group {
        id: energy
        width: parent.width
        title: "Apps using significant energy"
        footer: !page.cfg.appEnergy ? "Off: nothing is recorded." : "CPU time over the last " + (Power.energyMinutes > 0 && Power.energyMinutes < 60 ? Power.energyMinutes + " minutes" : "hour") + ". A look every 5 minutes; kept only on this computer."
        // The report runs only while this shows
        Component.onCompleted: Power.energyUsers++
        Component.onDestruction: Power.energyUsers--
        Row {
            label: "Keep track"
            Switch {
                checked: page.cfg.appEnergy
                onToggled: {
                    Config.options.power.appEnergy = !checked;
                    if (checked === false)
                        Power.clearEnergy();
                }
            }
        }
        Row {
            visible: page.cfg.appEnergy && Power.energyApps.length === 0
            label: "Nothing has used much energy lately"
            detail: "Apps show up here after they've kept the processor busy"
        }
        Repeater {
            model: page.cfg.appEnergy ? Power.energyApps : []
            Row {
                id: appRow
                required property var modelData
                readonly property string kind: modelData.key.split(":")[0]
                readonly property string ident: modelData.key.slice(kind.length + 1)
                readonly property var entry: kind === "proc" ? null : Apps.entryFor(ident)
                readonly property real most: Power.energyApps[0]?.seconds || 1
                label: entry?.name || ident
                detail: Math.round(modelData.seconds / 60) >= 1 ? Math.round(modelData.seconds / 60) + " min of CPU" : Math.round(modelData.seconds) + " s of CPU"
                QQ.Row {
                    spacing: 12
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 120
                        height: 6
                        radius: 3
                        color: Theme.color.fill
                        Rectangle {
                            width: Math.max(6, parent.width * appRow.modelData.seconds / appRow.most)
                            height: parent.height
                            radius: 3
                            color: Theme.color.secondary
                        }
                    }
                    SettingsButton {
                        visible: appRow.kind !== "proc"
                        text: "Quit"
                        onClicked: Apps.quit(appRow.ident)
                    }
                }
            }
        }
    }
}
