pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import qs.common

// Live captions (Settings → Accessibility): subtitles for the sound this
// computer plays, made here (assets/scripts/captions.py, whisper.cpp's Tiny
// model; never the microphone, nothing saved). Shown in a floating bar, the
// island, or a strip across the bottom (modules/captions, Island).
Singleton {
    id: root

    readonly property var cfg: Config.options.captions
    readonly property bool enabled: Config.ready && cfg.enabled
    readonly property string placement: cfg.placement || "bar"
    readonly property string script: Quickshell.shellDir + "/assets/scripts/captions.py"

    // What it needs: whisper.cpp and the Tiny model (Dictation's)
    readonly property bool hasBinary: Dictation.hasBinary
    readonly property bool hasModel: Dictation.info.models?.tiny ?? false
    readonly property bool ready: hasBinary && hasModel
    readonly property bool running: helper.running

    function toggle() {
        cfg.enabled = !cfg.enabled;
    }
    onEnabledChanged: {
        if (enabled && !ready)
            Dictation.refresh(); // (installed since it was last looked at?)
        lines = [];
        partial = "";
        error = "";
    }

    // The words: finished sentences (the last two), and the one being heard
    property var lines: []
    property string partial: ""
    property bool quiet: true // nothing said for a while: the bar fades
    property string error: ""
    readonly property string latest: partial !== "" ? partial : (lines[lines.length - 1] ?? "")
    readonly property string previous: partial !== "" ? (lines[lines.length - 1] ?? "") : (lines[lines.length - 2] ?? "")

    KeptProcess {
        id: helper
        wanted: root.enabled && root.ready
        command: [Tools.watcher, "python3", "-I", root.script, "run", root.cfg.language || "auto"]
        stdinEnabled: true // (Tools.watcher: it, and the recorder, end with the shell)
        stdout: SplitParser {
            onRead: line => {
                let m;
                try {
                    m = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (m.partial !== undefined) {
                    root.partial = m.partial;
                    root.quiet = false;
                } else if (m.final !== undefined) {
                    root.partial = "";
                    if (m.final !== "")
                        root.lines = root.lines.concat([m.final]).slice(-2);
                    root.quiet = false;
                } else if (m.quiet) {
                    root.quiet = true;
                } else if (m.error) {
                    root.error = m.error;
                }
            }
        }
        onRunningChanged: if (running) root.error = ""
    }
    // The language changed: started again with it
    Connections {
        target: root.cfg
        function onLanguageChanged() {
            helper.restart();
        }
    }

    // qs ipc call captions toggle | on | off
    IpcHandler {
        target: "captions"
        function toggle(): void { root.toggle(); }
        function on(): void { root.cfg.enabled = true; }
        function off(): void { root.cfg.enabled = false; }
    }
    GlobalShortcut {
        appid: "irice"
        name: "captions"
        description: "Live captions on or off"
        onPressed: root.toggle()
    }

    // Setting it up: whisper.cpp (Dictation's install) and Tiny (75 MB)
    function install() {
        Dictation.install();
    }
    function download() {
        Dictation.download("tiny", false);
    }
    readonly property bool downloading: Dictation.downloading === "tiny"
}
