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

    // Installing hands over to assets/scripts/update-run.sh, started detached
    // from a copy (with the update screen's): it stops this shell, updates
    // behind a full-screen "Installing update" screen, rebuilds, and starts
    // the new shell once. Updating under a running shell made it reload
    // halfway (its files change), which killed the update with it.
    function apply() {
        if (!ready || !allowed)
            return;
        applying = true;
        error = "";
        result = null;
        Quickshell.execDetached(["sh", "-c",
            "t=$(mktemp -d \"${XDG_RUNTIME_DIR:-/tmp}/irice-update.XXXXXX\") || exit 1; " +
            "cp -r \"$1/shell/assets/updater\" \"$t/ui\" && cp \"$1/shell/assets/scripts/update-run.sh\" \"$t/run.sh\" && printf '%s' \"$2\" > \"$t/choices.json\" && " +
            "exec setsid -f bash \"$t/run.sh\" \"$t\" \"$1\" \"$3\" </dev/null >/dev/null 2>&1",
            "sh", repo, JSON.stringify(choices), Config.dir]);
        startTimeout.start();
    }
    // If the update screen never took over (the helper couldn't start)
    Timer {
        id: startTimeout
        interval: 20000
        onTriggered: if (root.applying) {
            root.applying = false;
            root.error = "The update didn't start. Try again, or run tools/irice-update.py in the iRice folder.";
        }
    }

    // After the restart: what the last install did, for Software Update
    FileView {
        id: lastResult
        path: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-update-result.json"
        blockLoading: false
        printErrors: false
        onLoaded: {
            try {
                const r = JSON.parse(text());
                if (r.error)
                    root.error = r.error;
                else if (r.applied)
                    root.result = r;
            } catch (e) {}
            Quickshell.execDetached(["rm", "-f", path]);
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
