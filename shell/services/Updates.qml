pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Available package updates (pacman via checkupdates, which never touches the
// real package database). Checked hourly.
Singleton {
    id: root

    property int count: 0
    property bool available: false // checkupdates installed

    function install() {
        // Never from the dev sandbox: it would update the real machine
        if (Quickshell.env("IRICE_SANDBOX") === "1") {
            Quickshell.execDetached(["notify-send", "-a", "iRice", "Sandbox", "Install Updates would run: sudo pacman -Syu"]);
            return;
        }
        Launch.terminal(["sh", "-c", "command -v yay >/dev/null && yay -Syu || sudo pacman -Syu; read -p 'Press Enter to close'"], false);
    }

    Timer {
        interval: 60 * 60 * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: check.running = true
    }
    Process {
        id: check
        command: ["sh", "-c", "command -v checkupdates >/dev/null || exit 3; checkupdates 2>/dev/null | wc -l"]
        stdout: StdioCollector {
            onStreamFinished: root.count = parseInt(text.trim()) || 0
        }
        onExited: code => root.available = code !== 3
    }
}
