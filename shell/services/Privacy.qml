pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Who is using the microphone, the camera or your location, and who is
// sharing the screen: the menu bar's privacy dots (orange mic, green camera,
// blue location, purple screen) and the island's "Sharing screen" activity.
//   mic     PipeWire/PulseAudio recording streams (not monitors of outputs),
//           refreshed when `pactl subscribe` reports a change
//   camera  processes holding a /dev/video* device (assets/scripts/camera.sh:
//           looked for only when a camera is opened or closed, with inotify)
//   location  GeoClue clients that are active (assets/scripts/location.sh:
//           when GeoClue signals a change; nothing is started to ask)
//   screen  PipeWire: the portal's video streams (xdph-streaming-N) that an
//           app is linked to (assets/scripts/screenshare.sh, event-driven).
//           Hyprland's screencast event only hints at what is shared: it
//           also fires for the shell's own window previews, so on its own
//           it flickered on and off.
Singleton {
    id: root

    property var micApps: []
    property var micStreams: [] // [{ name, bin, pid }] (calls read these)
    property var cameraApps: []
    // [{ node, app, target, window, links: [link ids], started }]
    property var shares: []
    readonly property bool screenShared: shares.length > 0
    readonly property string screenApp: shares[0]?.app ?? ""
    readonly property string screenTarget: shares[0]?.target ?? ""
    readonly property real screenStarted: shares[0]?.started ?? 0
    // Where it shows (Settings → Menu Bar), never twice: the island's live
    // activity, the purple dot, or both. With the island off, the dot.
    readonly property string shareIndicator: Config.options.privacy?.screenShareIndicator ?? "island"
    readonly property bool shareInIsland: screenShared && Island.enabled && shareIndicator !== "dot"
    readonly property bool shareDot: screenShared && (!Island.enabled || shareIndicator !== "island")
    readonly property bool mic: micApps.length > 0
    readonly property bool camera: cameraApps.length > 0
    property var locationApps: []
    readonly property bool location: locationApps.length > 0
    readonly property bool any: mic || camera || location || shareDot


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
        command: [Tools.watcher, decodeURIComponent(Qt.resolvedUrl("../assets/scripts/camera.sh").toString().replace("file://", ""))]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: line => {
                const list = line.replace(/^\[|\]$/g, "").split(",").filter(s => s !== "");
                root.cameraApps = list.map(root.nice);
            }
        }
    }

    // ── Location ────────────────────────────────────────────────────────
    // GeoClue is D-Bus activated: only look while it's already running, so
    // asking never starts it. An app that can't be named shows as "An app".
    Process {
        running: Tools.has("busctl")
        command: [Tools.watcher, decodeURIComponent(Qt.resolvedUrl("../assets/scripts/location.sh").toString().replace("file://", ""))]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: line => {
                const list = line.replace(/^\[|\]$/g, "").split(",").filter(s => s !== "");
                root.locationApps = list.map(a => a === "?" ? "An app" : root.nice(a.replace(/\.desktop$/, "").split(".").pop()));
            }
        }
    }

    // ── Screen sharing ──────────────────────────────────────────────────
    Process {
        id: shareWatch
        running: Tools.ready
        command: [Tools.watcher, decodeURIComponent(Qt.resolvedUrl("../assets/scripts/screenshare.sh").toString().replace("file://", ""))]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: line => root.readGraph(line)
        }
        onExited: shareRetry.start() // PipeWire restarted
    }
    Timer {
        id: shareRetry
        interval: 5000
        onTriggered: {
            root.shares = [];
            shareWatch.running = true;
        }
    }
    function readGraph(line) {
        let graph;
        try {
            graph = JSON.parse(line);
        } catch (e) {
            return;
        }
        const byId = {};
        for (const o of graph)
            byId[o.id] = o;
        const props = o => o?.info?.props ?? {};
        const links = graph.filter(o => (o.type ?? "").endsWith(":Link") && o.info && o.info.state !== "error");
        const out = [];
        for (const n of graph) {
            if (!(n.type ?? "").endsWith(":Node") || !(props(n)["node.name"] ?? "").startsWith("xdph-streaming-"))
                continue;
            // (iRice's own screen recorder, through the portal, isn't a share)
            const mine = links.filter(l => {
                if (l.info["output-node-id"] !== n.id)
                    return false;
                const pp = props(byId[l.info["input-node-id"]]), cp = props(byId[pp["client.id"]]);
                return !/gpu-screen-recorder|gsr/i.test((pp["application.process.binary"] || "") + " " + (cp["application.process.binary"] || "") + " " + (pp["application.name"] || "") + " " + (cp["application.name"] || ""));
            });
            if (mine.length === 0)
                continue; // a stream nobody receives (yet): not shared
            // The receiving app: its node's name, else its client's
            const names = mine.map(l => {
                const peer = byId[l.info["input-node-id"]];
                const pp = props(peer), cp = props(byId[pp["client.id"]]);
                return pp["application.name"] || cp["application.name"] || pp["application.process.binary"] || cp["application.process.binary"] || pp["node.name"] || "";
            }).filter(x => x !== "");
            const was = shares.find(x => x.node === n.id);
            const hint = was ? { target: was.target, window: was.window } : root.castHint();
            out.push({ node: n.id, app: names.length ? root.nice(names[0]) : "An app", target: hint.target, window: hint.window, links: mine.map(l => l.id), started: was?.started ?? Date.now() });
        }
        if (JSON.stringify(out) !== JSON.stringify(shares))
            shares = out;
        if (out.length === 0)
            stopping = false;
    }

    // What is shared, from Hyprland's screencast event around the moment the
    // stream started (OWNER 0: a display, 1: a window; v2 adds its name). Only
    // a hint: the shell's own previews send the same event.
    property var lastCast: null // { owner, name, at }
    function castHint() {
        const c = lastCast;
        if (!c || Date.now() - c.at > 4000)
            return { target: "Your screen", window: false };
        if (c.owner === "1") {
            const t = Hyprland.toplevels.values.find(t => c.name !== "" && (t.address === c.name || "0x" + t.address === c.name));
            const o = t?.lastIpcObject ?? {};
            return { target: o.title || o.class || c.name || "A window", window: true };
        }
        const m = Hyprland.monitors.values.find(m => m.name === c.name);
        return { target: m ? (m.lastIpcObject?.description || m.name) : (c.name || "Your screen"), window: false };
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            // screencast>>STATE,OWNER  (newer: screencastv2>>STATE,OWNER,NAME)
            if (event.name !== "screencast" && event.name !== "screencastv2")
                return;
            const f = event.data.split(",");
            if (f[0] === "1")
                root.lastCast = { owner: f[1] ?? "0", name: f.slice(2).join(","), at: Date.now() };
        }
    }

    // Stop sharing: cut the stream's links, so the app gets no more frames
    // (closing the portal session isn't possible from outside: its session
    // objects look the same as other portals'). If the app holds on, the
    // menu says to stop it there.
    property bool stopping: false
    property bool stopFailed: false
    function stopSharing() {
        const ids = shares.flatMap(x => x.links);
        if (ids.length === 0)
            return;
        stopping = true;
        stopFailed = false;
        Quickshell.execDetached(["sh", "-c", "for l in \"$@\"; do pw-cli destroy \"$l\" >/dev/null 2>&1; done", "sh"].concat(ids.map(String)));
        stopCheck.restart();
    }
    Timer {
        id: stopCheck
        interval: 2500
        onTriggered: {
            root.stopFailed = root.screenShared;
            root.stopping = false;
        }
    }
    onScreenSharedChanged: if (!screenShared) stopFailed = false
}
