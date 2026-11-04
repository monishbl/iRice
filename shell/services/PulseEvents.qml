pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// One `pactl subscribe` for the whole shell (Privacy's microphone dot,
// Devices' output alerts): each line is passed on as event(). Started again
// after it ends (the sound server restarted, or isn't up yet). irice-events
// (services/Events) subscribes itself when it's there; its events come as
// the same lines.
Singleton {
    id: root

    signal event(string line)
    readonly property bool running: Events.covers("pulse") ? Events.pulseUp : watch.running

    EventWatch {
        source: "pulse"
    }
    Connections {
        target: Events
        function onPulse(line) {
            root.event(line);
        }
    }
    KeptProcess {
        id: watch
        wanted: Tools.has("pactl") && Events.fallsBack("pulse")
        firstWait: 5000
        command: [Tools.watcher, "pactl", "subscribe"]
        stdinEnabled: true // (Tools.watcher: it ends with the shell)
        stdout: SplitParser {
            onRead: line => root.event(line)
        }
    }
}
