pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.common

// The laptop battery's health and power draw, read from the kernel
// (/sys/class/power_supply/BAT*), which knows more than UPower reports:
// maximum capacity (full charge vs the design's), cycle count, and the watts
// going in or out. Read when asked (refresh(): the battery menu opening, a
// hover), never on a timer of its own.
//   capacity  0..1 of the design capacity, -1 unknown
//   cycles    charge cycles, -1 unknown (many batteries report 0: unknown too)
//   watts     power in or out right now, -1 unknown
Singleton {
    id: root

    property real capacity: -1
    property int cycles: -1
    property real watts: -1
    readonly property string condition: capacity < 0 ? "" : capacity >= 0.8 ? "Normal" : "Service recommended"

    function refresh() {
        if (!reader.running)
            reader.running = true;
    }

    Process {
        id: reader
        command: ["sh", "-c", "for b in /sys/class/power_supply/BAT*; do [ -d \"$b\" ] || continue; for f in energy_full energy_full_design charge_full charge_full_design cycle_count power_now current_now voltage_now; do [ -r \"$b/$f\" ] && echo \"$f=$(cat \"$b/$f\")\"; done; break; done"]
        stdout: StdioCollector {
            onStreamFinished: {
                const v = {};
                for (const l of text.split("\n")) {
                    const m = l.match(/^(\w+)=(\d+)$/);
                    if (m)
                        v[m[1]] = +m[2];
                }
                const full = v.energy_full ?? v.charge_full, design = v.energy_full_design ?? v.charge_full_design;
                root.capacity = full > 0 && design > 0 ? Math.min(1, full / design) : -1;
                root.cycles = v.cycle_count > 0 ? v.cycle_count : -1;
                // power_now in µW, or current (µA) × voltage (µV)
                root.watts = v.power_now !== undefined ? v.power_now / 1e6 : v.current_now !== undefined && v.voltage_now !== undefined ? v.current_now * v.voltage_now / 1e12 : -1;
            }
        }
    }

    // The power mode, in words
    function modeName(p) {
        return p === PowerProfile.PowerSaver ? "Low Power" : p === PowerProfile.Performance ? "Performance" : "Balanced";
    }
    function hm(secs) {
        const m = Math.round(secs / 60);
        return Math.floor(m / 60) + ":" + String(m % 60).padStart(2, "0");
    }
    // One line for a hover: "84% · Charging · Balanced", "84% · 3:20 left · Low Power"
    function summary() {
        const d = UPower.displayDevice;
        const level = d.percentage > 1 ? d.percentage / 100 : d.percentage;
        const state = d.state === UPowerDeviceState.FullyCharged ? "Fully charged" : d.state === UPowerDeviceState.Charging ? (d.timeToFull > 0 ? "Full in " + hm(d.timeToFull) : "Charging") : d.state === UPowerDeviceState.PendingCharge ? "Not charging" : (d.timeToEmpty > 0 ? hm(d.timeToEmpty) + " left" : "On battery");
        return Math.round(level * 100) + "% · " + state + " · " + modeName(PowerProfiles.profile);
    }
}
