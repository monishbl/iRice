pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Built-in display brightness through brightnessctl. Writes are coalesced so
// dragging a slider doesn't spawn a process per pixel.
Singleton {
    id: root

    property real value: 0.5
    property bool available: false
    property real pending: -1

    function set(v) {
        value = Math.max(0.01, Math.min(1, v));
        pending = value;
        if (!writer.running)
            flush();
    }

    function flush() {
        if (pending < 0)
            return;
        writer.command = ["brightnessctl", "-q", "set", Math.round(pending * 100) + "%"];
        pending = -1;
        writer.running = true;
    }

    Process {
        id: writer
        onExited: root.flush()
    }

    Process {
        running: true
        command: ["brightnessctl", "-m", "-c", "backlight"]
        stdout: StdioCollector {
            onStreamFinished: {
                const f = text.trim().split(",");
                if (f.length >= 5) {
                    root.value = parseInt(f[2]) / parseInt(f[4]);
                    root.available = true;
                }
            }
        }
    }
}
