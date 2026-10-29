pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import qs.common

// Reporting a bug (Settings → Report a Bug, the "reportBug" shortcut or
// gesture, Spotlight). The screenshot is taken first, before anything of the
// report shows, so it has the screen as it was. The log and system info are
// gathered and scrubbed by assets/scripts/bug-report.py, and shown in full
// before anything is sent; each can be left out. Sent to iRice's report
// server, which files a GitHub issue; if that fails, the report can be
// copied or opened on GitHub instead.
Singleton {
    id: root

    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/bug-report.py").toString().replace("file://", ""))
    readonly property string dir: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-bug-report"
    readonly property string shot: dir + "/screen.png"

    // The form (kept while Settings is closed and reopened)
    property string title: ""
    property string happened: ""
    property string steps: ""
    property string expected: ""
    property bool includeShot: true
    property bool includeLog: true
    property bool includeInfo: true

    property bool hasShot: false
    property int shotVersion: 0 // bumped when the screenshot changes (the preview reloads)
    property string log: ""
    property var sysinfo: ({})
    property bool gathering: false

    // idle | sending | sent | error
    property string state_: "idle"
    property string error: ""
    property string issueUrl: ""
    property real lastSent: 0
    readonly property int waitSeconds: Math.max(0, Math.ceil((lastSent + 60000 - now) / 1000))
    property real now: Date.now()
    Timer {
        running: root.waitSeconds > 0
        interval: 1000
        repeat: true
        onTriggered: root.now = Date.now()
    }

    // Start a report: take the screenshot of the focused display, then open
    // the page (and read the log and system info meanwhile)
    function start() {
        state_ = "idle";
        error = "";
        issueUrl = "";
        const mon = Hyprland.focusedMonitor?.name ?? "";
        capture.command = ["sh", "-c", "mkdir -p \"$1\" && chmod 700 \"$1\" && grim ${3:+-o \"$3\"} \"$2\"", "sh", dir, shot, mon];
        capture.running = true;
    }
    Process {
        id: capture
        onExited: code => {
            root.hasShot = code === 0;
            root.includeShot = root.hasShot;
            root.shotVersion++;
            Panels.openSettings("bugreport");
            root.gather();
        }
    }
    function gather() {
        if (gatherProc.running)
            return;
        gathering = true;
        gatherProc.running = true;
    }
    Process {
        id: gatherProc
        command: ["python3", "-I", root.script, "gather", Quickshell.shellDir]
        stdout: StdioCollector {
            onStreamFinished: {
                root.gathering = false;
                try {
                    const r = JSON.parse(text);
                    root.log = r.log ?? "";
                    root.sysinfo = r.sysinfo ?? {};
                } catch (e) {
                    root.log = "";
                    root.sysinfo = {};
                }
            }
        }
    }

    // Mark up the screenshot (blur or crop something) in the screenshot editor
    function editShot() {
        if (hasShot)
            ScreenTools.editRequested(shot);
    }

    function report() {
        return { title: title, happened: happened, steps: steps, expected: expected,
            includeShot: includeShot && hasShot, includeLog: includeLog, includeInfo: includeInfo,
            shot: shot, log: log, sysinfo: sysinfo };
    }

    function send() {
        if (state_ === "sending" || waitSeconds > 0)
            return;
        if (title.trim() === "" || happened.trim() === "") {
            state_ = "error";
            error = title.trim() === "" ? "Add a title, then send again." : "Say what happened, then send again.";
            return;
        }
        state_ = "sending";
        error = "";
        sendProc.stdinEnabled = true;
        sendProc.running = true;
    }
    Process {
        id: sendProc
        command: ["python3", "-I", root.script, "send"]
        stdinEnabled: true
        onStarted: {
            write(JSON.stringify(root.report()) + "\n");
            stdinEnabled = false; // (end of input)
        }
        stdout: StdioCollector {
            onStreamFinished: {
                let r = null;
                try {
                    r = JSON.parse(text);
                } catch (e) {}
                if (r?.ok) {
                    root.state_ = "sent";
                    root.issueUrl = r.issue_url ?? "";
                    root.lastSent = Date.now();
                    root.now = Date.now();
                    root.title = root.happened = root.steps = root.expected = "";
                } else {
                    root.state_ = "error";
                    root.error = r?.error ?? "Couldn't send the report. Try again, or copy it or open it on GitHub instead.";
                }
            }
        }
    }

    // Fallbacks: the report as text on the clipboard, or a prefilled GitHub
    // issue (without the screenshot and log, which don't fit in a link)
    function copy() {
        textProc.stdinEnabled = true;
        textProc.running = true;
    }
    Process {
        id: textProc
        command: ["sh", "-c", "python3 -I \"$1\" text | wl-copy", "sh", root.script]
        stdinEnabled: true
        onStarted: {
            write(JSON.stringify(root.report()) + "\n");
            stdinEnabled = false;
        }
        onExited: code => Island.post({ kind: "custom", icon: "copy", title: code === 0 ? "Report copied" : "Couldn't copy the report", subtitle: code === 0 ? "Paste it into an issue or a message" : "Check that wl-clipboard is installed", color: Theme.accent }, 3000)
    }
    function openGithub() {
        urlProc.stdinEnabled = true;
        urlProc.running = true;
    }
    Process {
        id: urlProc
        command: ["python3", "-I", root.script, "github-url"]
        stdinEnabled: true
        onStarted: {
            write(JSON.stringify(root.report()) + "\n");
            stdinEnabled = false;
        }
        stdout: StdioCollector {
            onStreamFinished: if (text.trim().startsWith("https://"))
                Qt.openUrlExternally(text.trim())
        }
    }
}
