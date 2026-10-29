pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import "../common/phosphor.js" as Phosphor

// iRice's own updates (not the system's: that's Updates), from the git
// repository this shell runs from, through tools/irice-update.py. Checked
// only when you ask (Software Update → Check for updates; SoftwareUpdate
// runs this, the system check and the package check together). The report
// lists what's new and every conflict (your code edits, settings whose
// defaults change, the material you use, shortcut clashes); nothing is
// applied until each one is decided.
Singleton {
    id: root

    // (IRICE_UPDATE_REPO: another checkout to update, for testing the updater)
    property string repo: Quickshell.env("IRICE_UPDATE_REPO") || ""
    property bool sourceReady: false
    property bool pendingCheck: false
    Process {
        running: true
        command: ["python3", "-I", Quickshell.shellDir + "/assets/scripts/update-source.py", Quickshell.shellDir]
        stdout: StdioCollector { onStreamFinished: {
            if (!root.repo) root.repo = text.trim();
            root.sourceReady = true;
            if (root.pendingCheck) { root.pendingCheck = false; root.check(); }
        } }
    }
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
        if (!sourceReady) { pendingCheck = true; return; }
        if (!repo) { error = "The installation source is missing. Reinstall iRice from its checkout."; return; }
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
                let r = null;
                try {
                    r = JSON.parse(text);
                } catch (e) {
                    root.checking = false;
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
                // The packages of the version that would be installed
                root.checkPackages(r.available ? (r.upstream ?? "") : "");
                root.checking = false; // (last: the package check is already under way)
            }
        }
    }

    // Installing hands over to assets/scripts/update-run.sh, started detached
    // from a copy (with the update screen's): it stops this shell, updates
    // behind a full-screen "iRice is updating" screen, rebuilds, and starts
    // the new shell once. Updating under a running shell made it reload
    // halfway (its files change), which killed the update with it.
    // rebuild: parts to rebuild anyway ("plugin" after a Hyprland update);
    // with no iRice update, only those are rebuilt (same screen, same restart).
    // Started by SoftwareUpdate, after the system part.
    function launch(rebuild) {
        if (!allowed || !sourceReady || !repo)
            return;
        applying = true;
        error = "";
        result = null;
        Quickshell.execDetached(["sh", "-c",
            "t=$(mktemp -d \"${XDG_RUNTIME_DIR:-/tmp}/irice-update.XXXXXX\") || exit 1; " +
            "cp -r \"$1/shell/assets/updater\" \"$t/ui\" && cp \"$1/shell/assets/scripts/update-run.sh\" \"$t/run.sh\" && printf '%s' \"$2\" > \"$t/choices.json\" && " +
            // the update screen shows your menu icon: its glyph and font, copied
            "printf '%s' \"$4\" > \"$t/look.json\" && printf '%s' \"$5\" > \"$t/rebuild\" && cp \"$1/shell/assets/fonts/Phosphor-Fill.ttf\" \"$t/ui/\" 2>/dev/null; " +
            "exec setsid -f bash \"$t/run.sh\" \"$t\" \"$1\" \"$3\" </dev/null >/dev/null 2>&1",
            "sh", repo, JSON.stringify(choices), Config.dir, JSON.stringify(look()), (rebuild ?? []).join(",")]);
        startTimeout.start();
    }
    // The menu icon (as the menu bar draws it: MenuBar.qml's system menu)
    function look() {
        const c = Config.options.appearance.menuIcon;
        const names = { irice: "drop", linux: "linux-logo", windows: "windows-logo", apple: "apple-logo", android: "android-logo", distro: "linux-logo", command: "command", terminal: "terminal-window", cpu: "cpu", atom: "atom", planet: "planet", rocket: "rocket-launch", star: "star-four", lightning: "lightning", flame: "flame", leaf: "leaf", snowflake: "snowflake", moon: "moon-stars", cat: "cat", ghost: "ghost", alien: "alien", steam: "steam-logo", github: "github-logo", circle: "circle-wavy" };
        const image = c === "distro" && SystemInfo.osLogo !== "" ? Quickshell.iconPath(SystemInfo.osLogo, true) : "";
        return { glyph: Phosphor.glyphs[names[c] ?? "drop"] ?? "", image: image.replace("file://", "") };
    }

    // ── iRice's packages ────────────────────────────────────────────────
    // tools/irice-packages.sh: what's missing from tools/packages.sh (of the
    // version about to be installed) and what iRice dropped that's still
    // installed. SoftwareUpdate installs / removes them in its root step.
    property var packages: ({ missing: [], unavailable: [], dropped: [] })
    property bool packagesChecking: pkgStatus.running
    function checkPackages(ref) {
        if (!repo || pkgStatus.running)
            return;
        pkgStatus.ref = ref ?? "";
        pkgStatus.running = true;
    }
    Process {
        id: pkgStatus
        property string ref: ""
        command: [root.repo + "/tools/irice-packages.sh", "status"].concat(ref !== "" ? [ref] : [])
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.packages = JSON.parse(text.trim());
                } catch (e) {}
            }
        }
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

    // Kept on Software Update ("What's new") until Done
    function dismissResult() {
        result = null;
    }
    // Updated: an alert once the island is up, its button opens the notes
    Timer {
        id: announce
        interval: 4000
        onTriggered: {
            const n = root.result?.changelog?.length ?? 0;
            const title = "iRice updated";
            const sub = n > 0 ? root.result.changelog[0].title + (n > 1 ? " and " + (n - 1) + " more" : "") : "Your settings and shortcuts are as you left them";
            if (Island.enabled)
                Island.post({ kind: "sysupdate", icon: "check-circle", title: title, subtitle: sub, color: Theme.accents.green, action: "What's new", onAction: () => Panels.openSettings("update") }, 10000);
            else
                Quickshell.execDetached(["notify-send", "-a", "iRice", "-i", "system-software-update", title, sub + ". Settings → Software Update shows what's new."]);
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
                else if (r.applied || r.rebuilt) {
                    root.result = r;
                    if (r.applied)
                        announce.start();
                }
            } catch (e) {}
            Quickshell.execDetached(["rm", "-f", path]);
        }
    }
}
