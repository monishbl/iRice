pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Dictation: speak, and the words are typed into the app you're in. All on
// this computer: the microphone is recorded (pw-record, 16 kHz mono), whisper.cpp
// turns it into text (assets/scripts/dictate.py) and wtype types it.
//   Super+H (shell.qml: dictate / dictateRelease): a tap starts listening and
//   the next tap stops; held down, it listens until you let go.
// The island shows it while it listens and writes (Island.activities).
Singleton {
    id: root

    readonly property var cfg: Config.options.dictation
    readonly property string script: Quickshell.shellDir + "/assets/scripts/dictate.py"

    // ── What's installed (dictate.py status) ────────────────────────────
    property var info: ({})
    readonly property bool hasBinary: !!info.binary
    readonly property string model: cfg.model in (info.models ?? {}) ? cfg.model : "base"
    readonly property bool hasModel: info.models?.[model] ?? false
    readonly property bool ready: hasBinary && hasModel
    function refresh() {
        statusProc.running = true;
    }
    Process {
        id: statusProc
        command: ["python3", "-I", root.script, "status"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.info = JSON.parse(text);
                } catch (e) {}
            }
        }
    }
    Component.onCompleted: Startup.later(() => refresh()) // (after the desktop is up)

    // The language whisper listens for: the setting, or the system's (a
    // "C" locale has none: whisper works it out)
    readonly property string systemLanguage: /^[a-z]{2,3}$/.test(Qt.locale().name.split("_")[0]) ? Qt.locale().name.split("_")[0] : "auto"
    readonly property string language: cfg.language !== "" ? cfg.language : systemLanguage

    // ── Listening ───────────────────────────────────────────────────────
    // stage: "" | listening | writing
    property string stage: ""
    readonly property bool active: stage !== ""
    property real startedAt: 0
    property real pressedAt: 0 // the shortcut is down (0: it isn't)
    property string error: ""
    readonly property int holdMs: 400 // held longer than this: it stops on release
    readonly property int maxSeconds: 120
    property string wav: ""

    // The shortcut went down / came up
    function pressed() {
        if (stage === "listening") {
            stop();
            return;
        }
        if (stage === "writing")
            return;
        pressedAt = Date.now();
        start();
    }
    function released() {
        if (pressedAt === 0)
            return; // (a plain key up, or one the press already handled)
        const held = Date.now() - pressedAt;
        pressedAt = 0;
        if (held >= holdMs && stage === "listening")
            stop();
    }

    function start() {
        if (active || recorder.running || transcriber.running)
            return;
        if (!ready) {
            refresh();
            Island.post({
                kind: "custom", icon: "microphone-slash", title: "Dictation isn't set up",
                subtitle: !hasBinary ? "Install speech recognition in Settings" : "Download a speech model in Settings",
                color: Theme.color.secondary, action: "Open Settings", onAction: () => Panels.openSettings("dictation")
            }, 6000);
            pressedAt = 0;
            return;
        }
        if (Audio.micMuted) {
            Island.post({ kind: "custom", icon: "microphone-slash", title: "Your microphone is muted", subtitle: "Unmute it to dictate", color: Theme.color.secondary, action: "Unmute", onAction: () => Audio.toggleMic() }, 5000);
            pressedAt = 0;
            return;
        }
        error = "";
        wav = (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice/dictation-" + Date.now() + ".wav";
        // (PipeWire's recorder, else PulseAudio's, else ALSA's: each stops cleanly on SIGINT)
        recorder.command = ["sh", "-c", "umask 077; mkdir -p \"$(dirname \"$1\")\" || exit 1; "
            + "if command -v pw-record >/dev/null; then exec pw-record --rate 16000 --channels 1 --format s16 \"$1\"; "
            + "elif command -v parecord >/dev/null; then exec parecord --rate=16000 --channels=1 --format=s16le --file-format=wav \"$1\"; "
            + "else exec arecord -q -f S16_LE -r 16000 -c 1 -t wav \"$1\"; fi", "sh", wav];
        recorder.file = wav;
        recorder.running = true;
        startedAt = Date.now();
        stage = "listening";
        tick();
        limit.restart();
    }
    // Done speaking: write it out
    function stop() {
        if (stage !== "listening")
            return;
        limit.stop();
        stage = "writing";
        tick();
        if (Date.now() - startedAt < 500 || !recorder.running)
            stage = ""; // (too short to hold any words: dropped quietly)
        if (recorder.running)
            recorder.signal(2); // (SIGINT: pw-record finishes the file)
    }
    // Stop without typing anything
    function cancel() {
        if (!active)
            return;
        limit.stop();
        // (cancelled: for the process that is told to end, to drop its result)
        if (recorder.running) {
            cancelled = true;
            recorder.signal(2);
        } else if (transcriber.running) {
            cancelled = true;
            transcriber.signal(15);
            Quickshell.execDetached(["rm", "-f", transcriber.file]);
        }
        stage = "";
        pressedAt = 0;
    }
    property bool cancelled: false

    function tick() {
        if (cfg.sounds)
            Sounds.playFile(Sounds.files[stage === "listening" ? "recording" : "recordingOff"]);
    }

    Timer {
        id: limit
        interval: root.maxSeconds * 1000
        onTriggered: root.stop()
    }

    Process {
        id: recorder
        property string file // (this recording's: the next may have begun)
        stderr: StdioCollector { id: recErr }
        onExited: {
            const f = file;
            if (root.cancelled || root.stage !== "writing") {
                root.cancelled = false;
                Quickshell.execDetached(["rm", "-f", f]);
                if (root.stage === "listening") {
                    // (it ended by itself: no microphone, or no sound server)
                    limit.stop();
                    root.pressedAt = 0;
                    root.fail("The microphone couldn't be recorded. Check the input in Settings → Sound, then try again.");
                }
                return;
            }
            transcriber.file = f;
            transcriber.command = ["python3", "-I", root.script, "transcribe", f, root.model, root.language];
            transcriber.running = true;
        }
    }
    Process {
        id: transcriber
        property string file
        stdout: StdioCollector {
            onStreamFinished: {
                if (root.cancelled) {
                    root.cancelled = false;
                    return;
                }
                let r = {};
                try {
                    r = JSON.parse(text);
                } catch (e) {}
                if (r.ok)
                    root.finish(r.text ?? "");
                else
                    root.fail(r.error || "Dictation didn't work. Try again.");
            }
        }
    }

    // Type it where the cursor is
    function finish(text) {
        stage = "";
        if (text === "") {
            Island.post({ kind: "custom", icon: "microphone", title: "Didn't catch that", subtitle: "Speak a little closer to the microphone, then try again", color: Theme.color.secondary }, 3000);
            return;
        }
        if (!Tools.has("wtype")) {
            copyInstead(text);
            return;
        }
        // (through its input: a command line can be read by any program)
        typer.text = text;
        typer.stdinEnabled = true;
        typer.running = true;
    }
    Process {
        id: typer
        property string text
        command: ["wtype", "-"]
        stdinEnabled: true
        onStarted: {
            write(text);
            stdinEnabled = false;
        }
        onExited: code => {
            if (code !== 0)
                root.copyInstead(typer.text); // (no virtual keyboard here)
            typer.text = "";
        }
    }
    function copyInstead(text) {
        Clipboard.copyText(text);
        Island.post({ kind: "custom", icon: "clipboard-text", title: "Copied what you said", subtitle: "Typing into this app didn't work: paste it with Ctrl+V", color: Theme.accent }, 5000);
    }
    function fail(why) {
        stage = "";
        error = why;
        Island.post({ kind: "custom", icon: "microphone-slash", title: "Dictation didn't work", subtitle: why, color: Theme.color.danger }, 6000);
    }

    // qs ipc call dictation toggle | start | stop | cancel (for a key or
    // button of your own)
    IpcHandler {
        target: "dictation"
        function toggle(): void {
            root.stage === "listening" ? root.stop() : root.start();
        }
        function start(): void {
            root.start();
        }
        function stop(): void {
            root.stop();
        }
        function cancel(): void {
            root.cancel();
        }
    }

    // ── Setting it up (Settings → Dictation) ────────────────────────────
    readonly property bool installing: Updates.running && installAsked
    property bool installAsked: false
    property string installError: ""
    function install() {
        if (Updates.running || (hasBinary && Tools.has("wtype")))
            return;
        installAsked = true;
        installError = "";
        Updates.update(["--install", ["whisper-cpp"].concat(Tools.has("wtype") ? [] : ["wtype"]).join(",")]);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (!root.installAsked)
                return;
            root.installAsked = false;
            root.installError = j.state === "done" ? "" : (j.error || j.text || "The install didn't finish.");
            Tools.recheck();
            root.refresh();
        }
    }

    // A model download: which, how far (0..1), and what went wrong
    property string downloading: ""
    property real downloadProgress: 0
    property string downloadError: ""
    property bool downloadOk: false
    property string lastDownload: "" // (which one an error is about)
    // (the model in use changes once the new one is here: until then the
    // one you have keeps working)
    function download(name) {
        if (downloader.running)
            return;
        downloading = name;
        lastDownload = name;
        downloadProgress = 0;
        downloadError = "";
        downloadOk = false;
        downloader.command = ["python3", "-I", script, "download", name];
        downloader.running = true;
    }
    function cancelDownload() {
        if (downloader.running)
            downloader.signal(15);
    }
    Process {
        id: downloader
        stdout: SplitParser {
            onRead: line => {
                let m;
                try {
                    m = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (m.total > 0)
                    root.downloadProgress = m.done / m.total;
                if (m.ok === false)
                    root.downloadError = m.error ?? "";
                if (m.ok === true)
                    root.downloadOk = true;
            }
        }
        onExited: {
            if (root.downloadOk)
                Config.options.dictation.model = root.downloading;
            root.downloading = "";
            root.refresh();
        }
    }
    function removeModel(name) {
        Quickshell.execDetached(["python3", "-I", script, "remove", name]);
        refreshLater.restart();
    }
    Timer { id: refreshLater; interval: 500; onTriggered: root.refresh() }

    // name: label, size in MB (dictate.py MODELS)
    readonly property var models: [
        { value: "tiny", label: "Tiny", mb: 75, detail: "Fastest, makes more mistakes" },
        { value: "base", label: "Base", mb: 142, detail: "Quick, good for most dictation" },
        { value: "small", label: "Small", mb: 466, detail: "More accurate, a little slower" },
        { value: "medium", label: "Medium", mb: 1500, detail: "Accurate; slow without a fast processor" },
        { value: "large-v3-turbo", label: "Large (turbo)", mb: 1620, detail: "Most accurate; best with a fast processor" }
    ]
}
