pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// One `pactl subscribe` for the whole shell (Privacy's microphone dot,
// Devices' output alerts): each line is passed on as event(). Started again
// after it ends (the sound server restarted, or isn't up yet).
Singleton {
    id: root

    signal event(string line)
    readonly property bool running: watch.running

    KeptProcess {
        id: watch
        wanted: Tools.has("pactl")
        firstWait: 5000
        command: [Tools.watcher, "pactl", "subscribe"]
        stdinEnabled: true // (Tools.watcher: it ends with the shell)
        stdout: SplitParser {
            onRead: line => root.event(line)
        }
    }
}
