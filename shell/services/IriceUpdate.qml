pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// iRice's own updates (not the system's: that's Updates), from the git
// repository this shell runs from, through tools/irice-update.py. Checked a
// while after start and every six hours; when one is ready, a notification
// points to Settings → Software Update, which shows what's new and every
// conflict (your code edits, settings whose defaults change, the material
// you use, shortcut clashes). Nothing is applied until each one is decided.
Singleton {
    id: root

    // (IRICE_UPDATE_REPO: another checkout to update, for testing the updater)
    readonly property string repo: Quickshell.env("IRICE_UPDATE_REPO") || Quickshell.shellDir + "/.."
    readonly property string helper: repo + "/tools/irice-update.py"
    // Never from the dev sandbox on its own copy: it would update the
    // developer's working tree
    readonly property bool allowed: Quickshell.env("IRICE_SANDBOX") !== "1" || Quickshell.env("IRICE_UPDATE_REPO") !== ""
    readonly property bool tty: Quickshell.env("IRICE_TTY") === "1"

    property var report: null // the helper's check result
    property bool checking: false
    property bool applying: false
    property string error: ""
    property var result: null // the last apply's result
    readonly property bool available: !!report && report.available === true && !report.error
    readonly property var conflicts: report?.conflicts ?? []
    property var choices: ({}) // conflict id -> choice
    readonly property int undecided: conflicts.filter(c => !c.choices.includes(choices[c.id])).length
    readonly property bool ready: available && undecided === 0 && !applying

    function choose(id, choice) {
        const c = Object.assign({}, choices);
        c[id] = choice;
        choices = c;
    }

    function check() {
        if (checking || applying)
            return;
        checking = true;
        error = "";
        checkProc.running = true;
    }
    Process {
        id: checkProc
        command: ["python3", "-I", root.helper, "check", "--config", Config.dir]
        stdout: StdioCollector {
            onStreamFinished: {
                root.checking = false;
                let r = null;
                try {
                    r = JSON.parse(text);
                } catch (e) {
                    root.error = "The update check didn't answer properly. Try again later.";
                    return;
                }
                root.error = r.error ?? "";
                // Keep decisions already made for conflicts that are still there
                const kept = {};
                for (const c of r.conflicts ?? [])
                    if (root.choices[c.id] !== undefined)
                        kept[c.id] = root.choices[c.id];
                root.choices = kept;
                root.report = r;
                if (root.available && r.latest !== root.notified) {
                    root.notified = r.latest;
                    const n = (r.conflicts ?? []).length;
                    Quickshell.execDetached(["notify-send", "-a", "iRice", "-i", "system-software-update", "iRice update ready",
                        (r.notes?.length ?? 0) + " change" + (r.notes?.length === 1 ? "" : "s") + (n > 0 ? ", " + n + " thing" + (n === 1 ? "" : "s") + " for you to decide" : "") + ". Open Settings → Software Update to review and install it."]);
                }
            }
        }
    }
    property string notified: "" // the version last announced

    function apply() {
        if (!ready || !allowed)
            return;
        applying = true;
        error = "";
        result = null;
        applyProc.command = ["sh", "-c", "f=$(mktemp) && printf '%s' \"$1\" > \"$f\" && python3 -I \"$2\" apply --config \"$3\" --choices \"$f\"; s=$?; rm -f \"$f\"; exit $s", "sh", JSON.stringify(choices), helper, Config.dir];
        applyProc.running = true;
    }
    Process {
        id: applyProc
        stdout: StdioCollector {
            onStreamFinished: {
                root.applying = false;
                let r = null;
                try {
                    r = JSON.parse(text);
                } catch (e) {
                    root.error = "Installing the update failed partway. Your settings were backed up first; run tools/irice-update.py check in the iRice folder to see where it stands.";
                    return;
                }
                if (r.error) {
                    root.error = r.error;
                    return;
                }
                root.result = r;
                root.report = null;
                root.choices = {};
                restartTimer.start();
            }
        }
    }
    // The new version: Hyprland's config (keybinds, gestures, look) and the shell
    Timer {
        id: restartTimer
        interval: 1500 // (long enough to read "Installed")
        onTriggered: {
            Quickshell.execDetached(["hyprctl", "reload", "config-only"]);
            Quickshell.execDetached(root.tty ? [root.repo + "/sandbox/tty-shell.sh", "restart"] : [Quickshell.shellDir + "/assets/scripts/repair-shell.sh"]);
        }
    }

    Timer {
        running: root.allowed && Config.ready
        interval: 6 * 60 * 60 * 1000
        repeat: true
        onTriggered: root.check()
    }
    Timer { // the first check, once the session has settled
        running: root.allowed && Config.ready
        interval: 2 * 60 * 1000
        onTriggered: root.check()
    }
}
