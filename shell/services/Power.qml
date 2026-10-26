pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.UPower

// Battery warnings at 20%, 10% and 5% while discharging, and a note when
// fully charged. Each warning fires once per discharge.
Singleton {
    id: root

    readonly property var battery: UPower.displayDevice
    readonly property real level: battery.percentage > 1 ? battery.percentage / 100 : battery.percentage
    readonly property bool charging: battery.state === UPowerDeviceState.Charging || battery.state === UPowerDeviceState.FullyCharged || battery.state === UPowerDeviceState.PendingCharge
    property int lastWarned: 101

    onChargingChanged: if (charging) lastWarned = 101
    onLevelChanged: {
        if (!battery.isLaptopBattery || charging)
            return;
        const pct = Math.round(level * 100);
        for (const t of [5, 10, 20]) {
            if (pct <= t && lastWarned > t) {
                lastWarned = t;
                Quickshell.execDetached(["notify-send", "-a", "Battery", "-u", t <= 5 ? "critical" : "normal", "-i", "battery-caution", "Battery at " + pct + "%", t <= 5 ? "Plug in now — your computer will sleep soon." : "Plug in soon, or turn on Low Power in Control Center."]);
                return;
            }
        }
    }

    Connections {
        target: root.battery
        function onStateChanged() {
            if (root.battery.isLaptopBattery && root.battery.state === UPowerDeviceState.FullyCharged)
                Quickshell.execDetached(["notify-send", "-a", "Battery", "-i", "battery-full-charged", "Fully charged", "You can unplug your computer."]);
        }
    }
}
