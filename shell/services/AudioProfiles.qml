pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Sound outputs that need their card switched over first (audio-profiles.py):
// a plugged-in monitor's speakers while the laptop's are active, or the
// laptop's while the monitor's are. Control Center's Sound page lists them
// with the other outputs; picking one switches the card and makes its output
// the default. Read again when a card changes (Devices hears pactl's events)
// and whenever the Sound page opens.
Singleton {
    id: root

    property var alternatives: [] // [{ card, profile, name, kind }]
    property string switching: "" // the profile being switched to
    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/audio-profiles.py").toString().replace("file://", ""))

    function refresh() {
        if (Tools.has("pactl") && !lister.running)
            lister.running = true;
    }
    function pick(a) {
        if (switcher.running)
            return;
        switching = a.profile;
        switcher.command = ["python3", "-I", script, "switch", a.card, a.profile];
        switcher.running = true;
    }

    Process {
        id: lister
        command: ["python3", "-I", root.script, "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const next = JSON.parse(text);
                    if (JSON.stringify(next) !== JSON.stringify(root.alternatives))
                        root.alternatives = next;
                } catch (e) {}
            }
        }
    }
    Process {
        id: switcher
        onExited: {
            root.switching = "";
            root.refresh();
        }
    }
    Component.onCompleted: refresh()
}
