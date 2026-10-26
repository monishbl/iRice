pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Which optional command-line tools are installed, so features that need them
// can hide instead of failing. Checked once at startup.
Singleton {
    id: root

    readonly property var names: ["easyeffects", "warp-cli", "songrec", "hyprpicker", "wf-recorder", "tesseract", "trans", "wtype", "checkupdates", "hyprpm", "mangohud", "grim", "slurp", "pactl", "udevadm"]
    property var available: ({})
    property bool ready: false

    function has(name) {
        return available[name] === true;
    }

    Process {
        running: true
        command: ["sh", "-c", "for c in \"$@\"; do command -v \"$c\" >/dev/null 2>&1 && echo \"$c\"; done", "sh"].concat(root.names)
        stdout: StdioCollector {
            onStreamFinished: {
                const a = {};
                for (const l of text.split("\n"))
                    if (l.trim())
                        a[l.trim()] = true;
                root.available = a;
                root.ready = true;
            }
        }
    }
}
