pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Pipewire
import qs.common

// Screen recording. gpu-screen-recorder (hardware encoding, system audio and
// the microphone mixed into one track, pause) when it's installed, else
// wf-recorder (one audio source, no pause). Started from the recording
// toolbar (Super+Shift+5, Control Center → Record), Super+Shift+R (an area),
// Spotlight; the island shows it live with Stop, Pause, mic mute and Discard.
// ScreenTools.recording / recordingStarted / stopRecording() forward here.
Singleton {
    id: root

    readonly property var cfg: Config.options.recorder
    readonly property bool gsr: Tools.has("gpu-screen-recorder")
    readonly property bool available: gsr || Tools.has("wf-recorder")
    readonly property bool canPause: gsr
    readonly property string folder: (cfg.folder || "").trim() !== "" ? cfg.folder.replace(/^~(?=\/|$)/, Quickshell.env("HOME")) : Quickshell.env("HOME") + "/Videos/Screen Recordings"

    property bool recording: false
    property bool paused: false
    property string file: ""
    property real started: 0 // ms; moved forward by the time spent paused
    property real pausedAt: 0
    readonly property int elapsed: Math.max(0, Math.floor(((paused ? pausedAt : clock.date.getTime()) - started) / 1000))
    SystemClock {
        id: clock
        precision: SystemClock.Seconds
        enabled: root.recording
    }

    // ── The toolbar ─────────────────────────────────────────────────────
    // Super+Shift+5 opens every screen tool; Control Center's Record opens
    // the recording ones only (Entire screen · Window · Selected region)
    property bool toolbarRecord: false
    function openToolbar() {
        if (recording)
            return stop();
        toolbarRecord = true;
        Panels.show("screenTools", ScreenTools.focusedScreen());
    }
    Connections {
        target: Panels
        function onOpenChanged() {
            if (!Panels.isOpen("screenTools"))
                root.toolbarRecord = false;
        }
    }
    // Start what the toolbar has selected
    function recordArea(area) {
        cfg.area = area;
        if (area === "screen")
            ScreenTools.recordScreen();
        else
            ScreenTools.select(area === "window" ? "record-window" : "record");
    }

    // ── Microphones (for the options menu and Settings) ─────────────────
    property var mics: [] // [{ name, description }]
    function refreshMics() {
        if (Tools.has("pactl"))
            micList.running = true;
        else
            mics = pipewireMics();
    }
    // Without pactl (no pipewire-pulse): PipeWire's own audio sources
    function pipewireMics() {
        return Pipewire.nodes.values.filter(n => n.audio && !n.isSink && !n.isStream && !(n.name ?? "").endsWith(".monitor")).map(n => ({ name: n.name, description: n.description || n.nickname || n.name }));
    }
    Process {
        id: micList
        command: ["pactl", "-f", "json", "list", "sources"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.mics = JSON.parse(text).filter(s => !s.monitor_source && !(s.name ?? "").endsWith(".monitor")).map(s => ({ name: s.name, description: s.description || s.name }));
                } catch (e) {
                    root.mics = root.pipewireMics();
                }
            }
        }
    }
    readonly property string micLabel: cfg.mic === "" ? "Off" : cfg.mic === "default" ? "Default microphone" : (mics.find(m => m.name === cfg.mic)?.description ?? cfg.mic)

    // The microphone being recorded, for its level meter and mute
    readonly property var micNode: !recording || cfg.mic === "" ? null : cfg.mic === "default" ? Pipewire.defaultAudioSource : (Pipewire.nodes.values.find(n => n.name === cfg.mic) ?? Pipewire.defaultAudioSource)
    PwObjectTracker {
        objects: [root.micNode]
    }
    property bool micMuted: false
    property var micWasMuted: null // what it was before we touched it
    function toggleMic() {
        const a = micNode?.audio;
        if (!a)
            return;
        if (micWasMuted === null)
            micWasMuted = a.muted;
        a.muted = !a.muted;
        micMuted = a.muted;
    }
    function restoreMic() {
        if (micWasMuted !== null && micNode?.audio)
            micNode.audio.muted = micWasMuted;
        micWasMuted = null;
        micMuted = false;
    }
    // Level 0..1, only while something shows it (the island's open card)
    property int meterUsers: 0
    readonly property real micLevel: meter.peak
    PwNodePeakMonitor {
        id: meter
        node: root.micNode
        enabled: root.meterUsers > 0 && root.micNode !== null && !root.micMuted
    }

    // ── Countdown ───────────────────────────────────────────────────────
    property int countdown: 0 // seconds left; 0 = none showing
    property var pending: null // { region | output }
    Timer {
        id: tick
        interval: 1000
        repeat: true
        onTriggered: {
            root.countdown--;
            if (root.countdown <= 0) {
                stop();
                root.launch(root.pending);
                root.pending = null;
            }
        }
    }
    function cancelCountdown() {
        tick.stop();
        countdown = 0;
        pending = null;
    }

    // ── Start / stop ────────────────────────────────────────────────────
    // what: { output: "DP-1" } (a whole display) or { region: "x,y wxh" }
    // (global, as ScreenTools.geometry() gives it)
    function start(what) {
        if (recording || countdown > 0 || !available)
            return;
        Panels.close();
        const n = Math.max(0, cfg.countdown ?? 0);
        if (n > 0) {
            pending = what;
            countdown = n;
            tick.restart();
        } else
            Qt.callLater(() => launch(what));
    }

    // Without hardware encoding (Intel without intel-media-driver, NVIDIA
    // without libva-nvidia-driver, a VM) gpu-screen-recorder exits at once;
    // launch() then runs it again once with the CPU encoder (H.264 only)
    property bool cpuEncoding: false // this recording uses the CPU encoder
    property var lastWhat: null
    readonly property string cpuNote: "Hardware video encoding isn't available (install intel-media-driver for Intel or libva-nvidia-driver for NVIDIA); recorded with the CPU instead"
    // Is this stderr gpu-screen-recorder's "can't encode on the GPU"?
    function hwEncodeFailure(text) {
        return /vaapi|va-api|libva|failed to create|no h264|(h264|hevc|av1)[^\n]*(not supported|unavailable)|encod(er|ing)|nvenc|cuda|vulkan video/i.test(text || "");
    }

    function launch(what, cpu, keepFile) {
        if (!what)
            return;
        lastWhat = what;
        cpuEncoding = !!cpu;
        if (!keepFile) {
            const stamp = Qt.formatDateTime(new Date(), "yyyy-MM-dd 'at' hh.mm.ss");
            file = folder + "/Screen Recording " + stamp + ".mp4";
        }
        const audio = [];
        if (cfg.systemAudio)
            audio.push("default_output");
        if (cfg.mic !== "")
            audio.push(cfg.mic === "default" ? "default_input" : cfg.mic);
        let cmd;
        if (gsr) {
            cmd = ["gpu-screen-recorder"];
            if (what.region) {
                // "x,y wxh" → -region WxH+X+Y
                const m = what.region.match(/^(-?\d+),(-?\d+) (\d+)x(\d+)$/);
                cmd.push("-w", "region", "-region", m ? m[3] + "x" + m[4] + "+" + m[1] + "+" + m[2] : what.region);
            } else
                cmd.push("-w", what.output || "screen");
            // the CPU encoder only does H.264
            cmd.push("-f", String(cfg.fps || 60), "-k", cpu ? "h264" : (cfg.codec || "h264"), "-q", cfg.quality || "high", "-c", "mp4", "-cursor", cfg.cursor ? "yes" : "no");
            if (cpu)
                cmd.push("-encoder", "cpu");
            if (audio.length > 0)
                cmd.push("-a", audio.join("|")); // one track, mixed
            cmd.push("-o", file);
        } else {
            // wf-recorder: one audio source (the microphone if chosen, else
            // what's playing, through the output's monitor)
            cmd = ["wf-recorder", "-r", String(cfg.fps || 60), "-f", file];
            if (what.region)
                cmd.push("-g", what.region);
            else if (what.output)
                cmd.push("-o", what.output);
            if (cfg.mic !== "")
                cmd.push("--audio=" + (cfg.mic === "default" ? "default" : cfg.mic));
            else if (cfg.systemAudio)
                cmd.push("--audio=" + (Pipewire.defaultAudioSink?.name ? Pipewire.defaultAudioSink.name + ".monitor" : "default"));
        }
        proc.command = ["sh", "-c", "mkdir -p \"$1\" && shift && exec \"$@\"", "sh", folder].concat(cmd);
        discarding = false;
        paused = false;
        proc.running = true;
        recording = true;
        started = Date.now();
    }

    function stop() {
        if (countdown > 0)
            return cancelCountdown();
        if (!proc.running)
            return;
        if (paused && gsr)
            proc.signal(12); // SIGUSR2: resume first, so the end is written
        proc.signal(2); // SIGINT: stop and finish the file
    }
    function togglePause() {
        if (!proc.running || !gsr)
            return;
        proc.signal(12); // SIGUSR2: pause / resume
        if (paused)
            started += Date.now() - pausedAt;
        else
            pausedAt = Date.now();
        paused = !paused;
    }

    // Discard: stop, and throw the file away unless Undo is pressed within 8 s
    property bool discarding: false
    property string discarded: "" // the file waiting to go
    function discard() {
        if (!proc.running)
            return;
        discarding = true;
        stop();
    }
    function undoDiscard() {
        dropTimer.stop();
        const f = discarded;
        discarded = "";
        if (f !== "")
            root.saved(f);
    }
    Timer {
        id: dropTimer
        interval: 8000
        onTriggered: {
            if (root.discarded !== "")
                Quickshell.execDetached(["rm", "-f", "--", root.discarded]);
            root.discarded = "";
        }
    }

    signal saved(string file)
    signal dropped(string file) // discarded, Undo still possible
    signal failed(string why)
    signal notice(string title, string why, string brief) // saved, with something to know (brief: one line, for the island)

    Process {
        id: proc
        stderr: StdioCollector {
            id: errs
        }
        onExited: code => {
            root.restoreMic(); // first: micNode is gone once recording is false
            root.recording = false;
            root.paused = false;
            const f = root.file;
            if (root.discarding) {
                root.discarding = false;
                // a second Discard within the Undo time: the first one is final now
                if (root.discarded !== "" && root.discarded !== f)
                    Quickshell.execDetached(["rm", "-f", "--", root.discarded]);
                root.discarded = f;
                dropTimer.restart();
                root.dropped(f);
            } else if (code !== 0 && code !== 130 && code !== 255) {
                const all = errs.text || "";
                // no hardware encoder: once more on the CPU, same file
                if (root.gsr && !root.cpuEncoding && Date.now() - root.started < 10000 && root.hwEncodeFailure(all)) {
                    console.warn("Recorder: hardware encoding failed, retrying on the CPU:", all.trim().split("\n").pop());
                    Qt.callLater(() => root.launch(root.lastWhat, true, true));
                    return;
                }
                const e = all.trim().split("\n").filter(l => l.trim() !== "").pop() ?? "";
                if (root.cpuEncoding && root.hwEncodeFailure(all))
                    root.failed("Hardware video encoding isn't available and the CPU encoder failed too. Install intel-media-driver (Intel) or libva-nvidia-driver (NVIDIA), or install wf-recorder.");
                else
                    root.failed(e !== "" ? e : "The recorder stopped (code " + code + "). Try again, or check that the display is still connected.");
            } else {
                root.saved(f);
                if (root.cpuEncoding)
                    root.notice("Recorded with the CPU", root.cpuNote, "Install intel-media-driver or libva-nvidia-driver");
            }
        }
    }
    function open(f) {
        Quickshell.execDetached(["xdg-open", f]);
    }
    function showInFolder(f) {
        Quickshell.execDetached([Files.manager || "xdg-open", f.replace(/\/[^/]*$/, "")]);
    }

    // When nothing else announces it (the island is off or doesn't show
    // recordings): a notification with Open / Show in Folder
    readonly property bool islandSays: Config.options.island.enabled && (Config.options.island.events ?? []).includes("recording")
    onSaved: f => {
        if (!islandSays)
            Quickshell.execDetached(["sh", "-c", "a=$(notify-send -a 'Screen Recording' -i video-x-generic -A open=Open -A folder='Show in Folder' 'Recording saved' \"$(basename \"$1\")\"); [ \"$a\" = open ] && xdg-open \"$1\"; [ \"$a\" = folder ] && ${2:-xdg-open} \"$(dirname \"$1\")\"", "sh", f, Files.manager]);
    }
    onFailed: why => {
        if (!islandSays)
            Quickshell.execDetached(["notify-send", "-a", "Screen Recording", "-u", "critical", "Recording didn't work", why]);
    }
    onNotice: (title, why, brief) => {
        if (!islandSays)
            Quickshell.execDetached(["notify-send", "-a", "Screen Recording", title, why]);
    }
    onDropped: f => {
        if (!islandSays)
            Quickshell.execDetached(["sh", "-c", "a=$(notify-send -a 'Screen Recording' -t 8000 -A undo=Undo 'Recording discarded' \"$(basename \"$1\")\"); [ \"$a\" = undo ] && qs -p \"$2\" ipc call recorder undo", "sh", f, decodeURIComponent(Qt.resolvedUrl("..").toString().replace("file://", ""))]);
    }

    // qs ipc call recorder start|stop|pause|discard|undo|toolbar
    IpcHandler {
        target: "recorder"
        function toolbar(): void {
            root.openToolbar();
        }
        function stop(): void {
            root.stop();
        }
        function pause(): void {
            root.togglePause();
        }
        function discard(): void {
            root.discard();
        }
        function undo(): void {
            root.undoDiscard();
        }
        function screen(): void {
            ScreenTools.recordScreen();
        }
    }
}
