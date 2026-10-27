pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Which optional command-line tools are installed, so features that need them
// can hide instead of failing. Checked once at startup.
Singleton {
    id: root

    readonly property var names: ["easyeffects", "warp-cli", "songrec", "hyprpicker", "wf-recorder", "tesseract", "trans", "wtype", "checkupdates", "hyprpm", "mangohud", "grim", "slurp", "pactl", "udevadm", "fuser", "busctl", "gpu-screen-recorder", "swappy", "nmcli", "qalc", "rg", "plocate", "pdftoppm", "ffmpegthumbnailer", "matugen", "pw-play", "paplay", "canberra-gtk-play", "gdbus", "dbus-monitor", "python3", "wl-copy", "nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm-qt", "pcmanfm", "kitty", "foot", "alacritty", "wezterm", "konsole", "zen-browser", "firefox", "google-chrome-stable", "chromium", "brave", "code", "codium", "zed", "cursor", "windsurf", "gnome-text-editor", "kate", "mousepad", "libreoffice", "onlyoffice-desktopeditors", "pavucontrol", "pavucontrol-qt", "gnome-system-monitor", "plasma-systemmonitor", "btop", "htop"]
    property var available: ({})
    // Wrapper for long-lived watcher processes (see the script): they end
    // with the shell, however it ends. Use as [Tools.watcher, cmd, args...]
    // with stdinEnabled: true.
    readonly property string watcher: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/watch.sh").toString().replace("file://", ""))
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
