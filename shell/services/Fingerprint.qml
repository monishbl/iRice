pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Fingerprints (assets/scripts/fingerprint.py): the reader and what it
// still needs, the fingers enrolled, and where a finger is accepted.
//   - iRice's lock screen: its own PAM file (assets/pam/irice-fingerprint),
//     next to the password, no root: lock.fingerprint
//   - password prompts, sudo, the login screen: a line in that PAM file,
//     through iRice's root helper (`irice-root fingerprint on|off …`, the
//     password once per change)
// Installing what the reader needs goes through Software Update's job (one
// password; AUR packages through yay or paru).
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/fingerprint.py"
    property var info: ({}) // fingerprint.py status
    readonly property var reader: info.reader ?? null
    readonly property bool hasReader: reader !== null
    readonly property var missing: (info.packages ?? []).concat(info.aur ?? []) // to install first
    readonly property bool ready: hasReader && (info.device ?? false) // fprintd has the reader
    readonly property var enrolled: info.enrolled ?? []
    readonly property var pam: info.pam ?? ({})
    readonly property bool lockScreen: ready && enrolled.length > 0 && (Config.options.lock.fingerprint ?? true)

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
    Component.onCompleted: refresh()

    // ── Installing what the reader needs ────────────────────────────────
    readonly property bool installing: Updates.running && installAsked
    property bool installAsked: false
    function install() {
        if (Updates.running || missing.length === 0)
            return;
        installAsked = true;
        const args = [];
        if ((info.packages ?? []).length > 0)
            args.push("--install", info.packages.join(","));
        if ((info.aur ?? []).length > 0)
            args.push("--aur-install", info.aur.join(","));
        Updates.update(args);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (!root.installAsked)
                return;
            root.installAsked = false;
            root.installError = j.state === "done" ? "" : (j.error || j.text || "The install didn't finish.");
            root.refresh();
        }
    }
    property string installError: ""

    // ── Adding a finger ─────────────────────────────────────────────────
    // stage: "" | waiting (claimed: touch it) | done | error
    property string enrollStage: ""
    property string enrollFinger: ""
    property int enrollStages: 0
    property int enrollCount: 0
    property string enrollHint: ""
    property string enrollError: ""
    readonly property bool enrolling: enrollProc.running
    function enroll(finger) {
        if (enrollProc.running)
            return;
        enrollFinger = finger;
        enrollStage = "starting";
        enrollStages = 0;
        enrollCount = 0;
        enrollHint = "";
        enrollError = "";
        enrollProc.command = ["python3", "-I", script, "enroll", finger];
        enrollProc.running = true;
    }
    function cancelEnroll() {
        if (enrollProc.running)
            enrollProc.signal(15);
        enrollStage = "";
    }
    // fprintd's results, said plainly
    readonly property var hints: ({
            "enroll-stage-passed": "Lift your finger, then touch the reader again",
            "enroll-retry-scan": "Didn't read that one. Touch the reader again",
            "enroll-swipe-too-short": "Too short. Try again",
            "enroll-finger-not-centered": "Put your finger in the middle of the reader",
            "enroll-remove-and-retry": "Lift your finger and try again"
        })
    Process {
        id: enrollProc
        stdout: SplitParser {
            onRead: line => {
                let m;
                try {
                    m = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (m.stage === "claimed") {
                    root.enrollStage = "waiting";
                    root.enrollStages = m.stages ?? 0;
                    root.enrollHint = "Touch the fingerprint reader";
                } else if (m.result !== undefined) {
                    root.enrollCount = m.count ?? root.enrollCount;
                    root.enrollHint = root.hints[m.result] ?? root.enrollHint;
                } else if (m.ok !== undefined) {
                    root.enrollStage = m.ok ? "done" : "error";
                    root.enrollError = m.ok ? "" : (m.error || "That didn't work. Try again.");
                    root.refresh();
                }
            }
        }
        onExited: if (root.enrollStage === "starting" || root.enrollStage === "waiting") {
            root.enrollStage = "error";
            root.enrollError = root.enrollError || "The reader stopped. Try again.";
        }
    }

    function remove(finger) {
        Quickshell.execDetached(["python3", "-I", script, "delete", finger]);
        refreshLater.restart();
    }
    Timer { id: refreshLater; interval: 800; onTriggered: root.refresh() }

    // ── Where else a finger is accepted (root helper, the password) ─────
    property string pamBusy: "" // polkit | sudo | login while it changes
    property string pamError: ""
    readonly property string rootHelper: "/usr/lib/irice/irice-root"
    function setPam(where, on) {
        if (pamProc.running)
            return;
        pamBusy = where;
        pamError = "";
        pamProc.command = ["pkexec", rootHelper, "fingerprint", on ? "on" : "off", where];
        pamProc.running = true;
    }
    Process {
        id: pamProc
        stderr: StdioCollector { id: pamErr }
        onExited: code => {
            if (code !== 0 && code !== 126)
                root.pamError = code === 127 ? "Not allowed (an administrator's password is needed)."
                    : (pamErr.text.replace(/^irice-root: /, "").trim() || "That didn't change. Update iRice, then try again.");
            root.pamBusy = "";
            root.refresh();
        }
    }

    // Finger names, said plainly
    readonly property var fingerNames: ({
            "right-index-finger": "Right index finger", "left-index-finger": "Left index finger",
            "right-thumb": "Right thumb", "left-thumb": "Left thumb",
            "right-middle-finger": "Right middle finger", "left-middle-finger": "Left middle finger",
            "right-ring-finger": "Right ring finger", "left-ring-finger": "Left ring finger",
            "right-little-finger": "Right little finger", "left-little-finger": "Left little finger"
        })
}
