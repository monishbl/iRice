pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// System health (Settings → About → System health): whether the services
// iRice uses actually answer, through assets/scripts/health-check.py. Only
// checked when asked (the page opening, Check again, Spotlight), never in
// the background. A broken check says how to fix it; restarting a user
// service can be done from the page.
Singleton {
    id: root

    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/health-check.py").toString().replace("file://", ""))
    // Set by the password prompt (modules/polkit/Polkit.qml): whether its
    // agent registered with polkit, which only the shell itself knows
    property string polkit: System.sandboxed ? "off" : "no"

    property var checks: [] // [{ id, name, group, status, detail, fix, canFix }]
    property bool checking: false
    property real checkedAt: 0
    property string fixing: "" // the check whose fix is running
    property string error: ""
    readonly property var attention: checks.filter(c => c.status === "fail" || c.status === "warn")
    readonly property var groups: ["Sound", "Network & sharing", "Power", "Desktop", "iRice itself"]

    function args() {
        return ["--polkit", polkit].concat(Google.connected ? ["--google"] : []);
    }
    signal fixDone(string id) // (a fix ran; Backends looks again)
    function check() {
        if (checking)
            return;
        Tools.recheck(); // (something may have been installed since)
        checking = true;
        error = "";
        proc.command = ["python3", "-I", script].concat(args());
        proc.running = true;
    }
    Process {
        id: proc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.checks = JSON.parse(text);
                    root.checkedAt = Date.now();
                } catch (e) {
                    root.error = "The health check didn't answer properly. Check that python3 is installed.";
                }
            }
        }
        onExited: root.checking = false
    }

    function fix(id) {
        if (fixing !== "")
            return;
        fixing = id;
        fixProc.command = ["python3", "-I", script, "fix", id];
        fixProc.running = true;
    }
    Process {
        id: fixProc
        stdout: StdioCollector {
            onStreamFinished: {
                let r = {};
                try {
                    r = JSON.parse(text);
                } catch (e) {}
                if (!r.ok)
                    root.error = r.error || "That didn't work. Try the command shown instead.";
            }
        }
        onExited: {
            const id = root.fixing;
            root.fixing = "";
            root.checking = false;
            root.fixDone(id);
            root.check(); // see whether it worked
        }
    }

    // Plain text, scrubbed like a bug report, to the clipboard
    function copyReport() {
        copyProc.command = ["sh", "-c", "python3 -I \"$0\" text \"$@\" | wl-copy", script].concat(args());
        copyProc.running = true;
    }
    Process {
        id: copyProc
        onExited: code => Island.post({ kind: "custom", icon: "copy", title: code === 0 ? "Health report copied" : "Couldn't copy the report", subtitle: code === 0 ? "Paste it into an issue or a message" : "Check that wl-clipboard is installed", color: Theme.accent }, 3000)
    }

    // Spotlight: open the page (it checks as it opens)
    function open() {
        Panels.openSettings("about/health");
    }
}
