pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Chromecast and Google TV (assets/scripts/cast.py: Google Cast spoken
// directly). What's playing, or a file, goes to a TV or speaker on the same
// network and is controlled from here: the island while it plays, Control
// Center's Cast page to pick a device, the share sheet for files, and
// "Cast to <TV>" in Spotlight.
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/cast.py"

    // ── Devices nearby (Avahi, _googlecast._tcp, followed while running) ─
    // [{ id, name, model, host, port }]
    property var devices: []
    property var found: ({}) // avahi name -> device
    Process {
        id: browser
        running: Tools.has("avahi-browse")
        command: ["avahi-browse", "-p", "-r", "-k", "_googlecast._tcp"]
        stdout: SplitParser {
            onRead: line => root.avahiLine(line)
        }
        // (avahi-daemon restarted: look again)
        onExited: restartBrowser.restart()
    }
    Timer { id: restartBrowser; interval: 10000; onTriggered: browser.running = Tools.has("avahi-browse") }
    function avahiLine(line) {
        const f = line.split(";");
        if (f.length < 4)
            return;
        const key = f[3];
        const next = Object.assign({}, found);
        if (f[0] === "-") {
            delete next[key];
        } else if (f[0] === "=" && f.length >= 10) {
            const txt = {};
            const re = /"([^=]+)=([^"]*)"/g; // (no matchAll in QML)
            let m;
            while ((m = re.exec(f[9])) !== null)
                txt[m[1]] = m[2];
            const dev = { id: txt.id || key, name: txt.fn || key, model: txt.md || "", host: f[7], port: Number(f[8]) || 8009 };
            // IPv4 over IPv6
            if (next[key] && !next[key].host.includes(":") && dev.host.includes(":"))
                return;
            next[key] = dev;
        } else {
            return;
        }
        found = next;
        devices = Object.values(next).sort((a, b) => a.name.localeCompare(b.name));
    }
    function iconFor(dev) {
        return /nest|home|speaker|audio/i.test(dev?.model ?? "") ? "speaker-hifi" : "television-simple";
    }

    // ── What can be cast right now ──────────────────────────────────────
    // What's playing, when its app says where it comes from: a file, or a
    // web page (a video site: through yt-dlp)
    readonly property var player: Media.player
    readonly property string playingUrl: String(player?.metadata?.["xesam:url"] ?? "")
    readonly property bool canCastPlaying: /^(file|https?):\/\//.test(playingUrl)
    readonly property bool hasYtDlp: Tools.has("yt-dlp")

    // ── The cast (one at a time) ────────────────────────────────────────
    // state: "" | connecting | loading | buffering | playing | paused | idle | ended | error
    property string state_: ""
    readonly property bool active: state_ !== "" && state_ !== "ended" && state_ !== "error"
    property var device: null
    property string title: ""
    property real time: 0
    property real duration: 0
    property real volume: 1
    property string error: ""

    function castPlaying(dev) {
        if (!canCastPlaying) {
            fail(dev, "Can't cast what's playing", "The app playing it doesn't say what it is. Cast a file from Files instead.");
            return;
        }
        const p = player;
        const cmd = { cmd: "cast", host: dev.host, port: dev.port, title: p?.trackTitle || "", start: Math.max(0, Math.floor(p?.position ?? 0)) };
        if (playingUrl.startsWith("file://"))
            cmd.file = decodeURIComponent(playingUrl.slice(7));
        else
            cmd.url = playingUrl;
        begin(dev, cmd);
        if (p?.isPlaying && p.canPause)
            p.pause(); // (it plays on the TV now)
    }
    function castFile(dev, path) {
        begin(dev, { cmd: "cast", host: dev.host, port: dev.port, file: path, title: path.split("/").pop().replace(/\.[^.]+$/, "") });
    }
    function begin(dev, cmd) {
        if (session.running) {
            send({ cmd: "stop" });
            pendingCast = { dev: dev, cmd: cmd };
            return;
        }
        device = dev;
        title = cmd.title;
        time = cmd.start ?? 0;
        duration = 0;
        error = "";
        state_ = "connecting";
        session.running = true;
        send(cmd);
    }
    property var pendingCast: null

    function send(obj) {
        if (session.running)
            session.write(JSON.stringify(obj) + "\n");
    }
    function togglePlay() {
        send({ cmd: state_ === "playing" || state_ === "buffering" ? "pause" : "play" });
    }
    function seek(t) {
        send({ cmd: "seek", t: Math.max(0, t) });
    }
    function setVolume(v) {
        send({ cmd: "volume", v: v });
    }
    function stop() {
        send({ cmd: "stop" });
    }

    Process {
        id: session
        command: ["python3", "-I", root.script, "session"]
        stdinEnabled: true
        stdout: SplitParser {
            onRead: line => {
                let s;
                try {
                    s = JSON.parse(line);
                } catch (e) {
                    return;
                }
                const was = root.state_;
                root.state_ = s.state ?? root.state_;
                root.time = s.time ?? root.time;
                root.duration = s.duration ?? root.duration;
                root.volume = s.volume ?? root.volume;
                if (s.state === "error")
                    root.fail(root.device, (was === "playing" || was === "paused" || was === "buffering" ? "Lost the connection to " : "Couldn't cast to ") + (root.device?.name ?? "the TV"), s.error || "Try again.");
            }
        }
        onExited: {
            if (root.state_ !== "error")
                root.state_ = "";
            if (root.pendingCast) {
                const p = root.pendingCast;
                root.pendingCast = null;
                root.begin(p.dev, p.cmd);
            }
        }
    }
    function fail(dev, title_, why) {
        error = why;
        Island.post({ kind: "custom", icon: "screencast", title: title_, subtitle: why, color: Theme.color.danger }, 7000);
    }

    // qs ipc call cast stop | toggle
    IpcHandler {
        target: "cast"
        function stop(): void {
            root.stop();
        }
        function toggle(): void {
            root.togglePlay();
        }
    }
}
