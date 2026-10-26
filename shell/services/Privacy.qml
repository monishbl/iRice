pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// Who is using the microphone, the camera, or sharing the screen, for the
// menu bar's privacy dots (orange mic, green camera, purple screen).
//   mic     PipeWire/PulseAudio recording streams (not monitors of outputs),
//           refreshed when `pactl subscribe` reports a change
//   camera  processes holding a /dev/video* device (checked every 3 s by one
//           small loop that only prints when the set of apps changes)
//   screen  Hyprland's screencast event
Singleton {
    id: root

    property var micApps: []
    property var micStreams: [] // [{ name, bin, pid }] (calls read these)
    property var cameraApps: []
    property bool screenShared: false
    property string screenApp: ""
    readonly property bool mic: micApps.length > 0
    readonly property bool camera: cameraApps.length > 0
    readonly property bool any: mic || camera || screenShared


    function nice(name) {
        const n = (name || "").replace(/\.(exe|bin)$/, "");
        return n.length ? n.charAt(0).toUpperCase() + n.slice(1) : "An app";
    }

    // ── Microphone ──────────────────────────────────────────────────────
    Process {
        id: pactlWatch
        running: Tools.has("pactl")
        command: [Tools.watcher, "pactl", "subscribe"]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: line => {
                if (line.includes("source-output") || line.includes("'source'"))
                    micRefresh.restart();
            }
        }
        onExited: pactlRetry.start()
    }
    Timer {
        id: pactlRetry
        interval: 5000
        onTriggered: pactlWatch.running = Tools.has("pactl")
    }
    Timer {
        id: micRefresh
        interval: 150 // a burst of events, one look
        triggeredOnStart: false
        onTriggered: micList.running = true
    }
    Connections {
        target: Tools
        function onReadyChanged() {
            if (Tools.has("pactl"))
                micList.running = true;
        }
    }
    Process {
        id: micList
        command: ["sh", "-c", "pactl -f json list source-outputs; echo; pactl -f json list sources short"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const parts = text.split("\n").filter(l => l.trim().startsWith("["));
                    const outs = JSON.parse(parts[0] ?? "[]");
                    const sources = JSON.parse(parts[1] ?? "[]");
                    const monitor = new Set(sources.filter(s => (s.name ?? "").endsWith(".monitor")).map(s => s.index));
                    const apps = new Set();
                    const streams = [];
                    for (const o of outs) {
                        const p = o.properties ?? {};
                        const bin = p["application.process.binary"] ?? "";
                        if (monitor.has(o.source) || bin === "qs" || bin === "quickshell" || p["media.name"] === "Peak detect")
                            continue;
                        apps.add(root.nice(p["application.name"] || bin));
                        streams.push({ name: p["application.name"] ?? "", bin: bin, pid: parseInt(p["application.process.id"] ?? "0") || 0 });
                    }
                    root.micApps = [...apps];
                    root.micStreams = streams;
                } catch (e) {
                    root.micApps = [];
                    root.micStreams = [];
                }
            }
        }
    }

    // ── Camera ──────────────────────────────────────────────────────────
    Process {
        running: Tools.has("fuser")
        command: [Tools.watcher, "bash", "-c", "prev=x; while :; do cur=$(fuser /dev/video* 2>/dev/null | tr -s ' ' '\\n' | grep -o '[0-9]\\+' | sort -u | while read -r p; do cat /proc/$p/comm 2>/dev/null; done | sort -u | paste -sd,); [ \"$cur\" != \"$prev\" ] && { echo \"[$cur]\"; prev=$cur; }; sleep 3; done"]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: line => {
                const list = line.replace(/^\[|\]$/g, "").split(",").filter(s => s !== "");
                root.cameraApps = list.map(root.nice);
            }
        }
    }

    // ── Screen sharing ──────────────────────────────────────────────────
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            // screencast>>STATE,OWNER  (newer: screencastv2>>STATE,OWNER,NAME)
            if (event.name !== "screencast" && event.name !== "screencastv2")
                return;
            const f = event.data.split(",");
            root.screenShared = f[0] === "1";
            if (f.length > 2)
                root.screenApp = root.screenShared ? root.nice(f.slice(2).join(",")) : "";
            else if (!root.screenShared)
                root.screenApp = "";
        }
    }
}
