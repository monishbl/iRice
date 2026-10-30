pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Built-in display brightness through brightnessctl. Writes are coalesced so
// dragging a slider doesn't spawn a process per pixel. With several backlight
// devices (a laptop with both intel_backlight and acpi_video0, say), the one
// with the finest steps (largest max_brightness) is used, as it's normally the
// one that really drives the panel. Without any (a desktop with external
// displays), `available` stays false and set() does nothing.
Singleton {
    id: root

    property real value: 0.5
    property bool available: false
    property string device: ""
    property real pending: -1

    function set(v) {
        if (!available)
            return;
        value = Math.max(0.01, Math.min(1, v));
        pending = value;
        if (!writer.running)
            flush();
    }

    function flush() {
        if (pending < 0)
            return;
        writer.command = ["brightnessctl", "-q"].concat(device !== "" ? ["-d", device] : [], ["set", Math.round(pending * 100) + "%"]);
        pending = -1;
        writer.running = true;
    }

    Process {
        id: writer
        onExited: root.flush()
    }

    // device,class,current,percent,max per line
    Process {
        running: true
        command: ["brightnessctl", "-m", "-l", "-c", "backlight"]
        stdout: StdioCollector {
            onStreamFinished: {
                let best = null;
                for (const l of text.trim().split("\n")) {
                    const f = l.split(",");
                    if (f.length < 5 || !(parseInt(f[4]) > 0))
                        continue;
                    if (!best || parseInt(f[4]) > parseInt(best[4]))
                        best = f;
                }
                if (best) {
                    root.device = best[0];
                    root.value = parseInt(best[2]) / parseInt(best[4]);
                    root.available = true;
                }
            }
        }
    }
}
