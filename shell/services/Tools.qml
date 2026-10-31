pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Which optional command-line tools are installed, so features that need them
// can hide instead of failing. Checked at startup, and again with recheck()
// (System health refreshing, after an install).
Singleton {
    id: root

    readonly property var names: ["cliphist", "easyeffects", "warp-cli", "songrec", "hyprpicker", "wf-recorder", "tesseract", "trans", "wtype", "checkupdates", "hyprpm", "mangohud", "grim", "slurp", "pactl", "udevadm", "fuser", "busctl", "gpu-screen-recorder", "ddcutil", "nvidia-smi", "faillock", "swappy", "nmcli", "qalc", "rg", "plocate", "pdftoppm", "ffmpegthumbnailer", "matugen", "pw-play", "paplay", "canberra-gtk-play", "gdbus", "dbus-monitor", "python3", "wl-copy", "nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm-qt", "pcmanfm", "kitty", "foot", "alacritty", "wezterm", "konsole", "ghostty", "gnome-terminal", "kgx", "ptyxis", "xfce4-terminal", "xterm", "hyprsunset", "ip", "iw", "rfkill", "yay", "paru", "zen-browser", "firefox", "google-chrome-stable", "chromium", "brave", "code", "codium", "zed", "cursor", "windsurf", "gnome-text-editor", "kate", "mousepad", "libreoffice", "onlyoffice-desktopeditors", "pavucontrol", "pavucontrol-qt", "gnome-system-monitor", "plasma-systemmonitor", "btop", "htop", "avahi-browse", "yt-dlp"]
    property var available: ({})
    // Wrapper for long-lived watcher processes (see the script): they end
    // with the shell, however it ends. Use as [Tools.watcher, cmd, args...]
    // with stdinEnabled: true.
    readonly property string watcher: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/watch.sh").toString().replace("file://", ""))
    property bool ready: false

    function has(name) {
        return available[name] === true;
    }
    // Look again (after installing something)
    function recheck() {
        if (probe.running)
            again = true;
        else
            probe.running = true;
    }
    property bool again: false

    // Watchers an earlier shell left behind (it crashed, or was killed
    // outright): end them before this shell starts its own
    Process {
        running: true
        command: ["bash", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/reap-watchers.sh").toString().replace("file://", ""))]
    }

    Process {
        id: probe
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
        onExited: if (root.again) {
            root.again = false;
            running = true;
        }
    }
}
