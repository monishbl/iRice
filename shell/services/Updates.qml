pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// System updates: what's available (checkupdates, which never touches the
// real package database, and yay/paru -Qua for the AUR; only when asked:
// Software Update → Check for updates), and the system part of the update,
// without a terminal (SoftwareUpdate plans it): assets/scripts/sysupdate.py runs
// detached (it keeps going with Settings closed, and across shell restarts)
// and writes its progress to $XDG_RUNTIME_DIR/irice-sysupdate/, read here.
// The password is asked once, at the start, by iRice's own dialog (pkexec →
// the polkit agent); the AUR step's pacman commands go through the same root
// part, so it never asks again in the middle.
// In the dev sandbox the update is simulated (nothing is installed).
Singleton {
    id: root

    property int count: 0 // repo updates
    property var list: [] // [{ name, from, to }]
    property int aurCount: 0
    property var aurList: []
    property bool available: false // checkupdates installed
    property bool checking: false
    readonly property int total: count + aurCount

    readonly property bool fake: Quickshell.env("IRICE_SANDBOX") === "1" || Quickshell.env("IRICE_FAKE_SYSUPDATE") === "1"
    readonly property string dir: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-sysupdate"
    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/sysupdate.py").toString().replace("file://", ""))

    // ── The update job ──────────────────────────────────────────────────
    // { state: running|done|error|cancelled, phase, text, done, total,
    //   percent, package, upgraded, restart, aur, error, started, finished }
    property var job: null
    property real requested: 0 // when Update was pressed (an older status is the last run's)
    property bool interrupted: false // "running", but nothing holds its lock
    readonly property bool running: job?.state === "running" && !interrupted
    readonly property bool cancellable: running && (job.phase === "starting" || job.phase === "auth")
    readonly property real progress: running ? (job.percent ?? 0) / 100 : -1
    // The full output, only read while something shows it (Show details)
    property int logUsers: 0
    property bool showDetails: false // Software Update's output, open (the island's Details)
    property string log: ""

    // args: sysupdate.py's plan (--system, --snapshot, --ignore, --install,
    // --remove, --aur)
    function update(args) {
        if (running)
            return;
        interrupted = false;
        log = "";
        logSize = 0;
        // setsid, in this login session (so pkexec finds iRice's password
        // dialog through polkit); not tied to the shell
        Quickshell.execDetached(["setsid", "-f", "python3", "-I", script, "run"].concat(args ?? []).concat(fake ? ["--fake"] : []));
        requested = Date.now() / 1000;
        job = { state: "running", phase: "starting", text: "Starting", percent: 0, done: 0, total: 0, started: requested };
        poll.restart();
    }
    function cancel() {
        if (cancellable)
            Quickshell.execDetached(["python3", "-I", script, "cancel"]);
    }
    // Forget a finished run (the Settings card and the island stop showing it)
    function dismiss() {
        if (!running)
            job = null;
    }
    function copyLog() {
        Quickshell.execDetached(["sh", "-c", "wl-copy < \"$1\"", "sh", dir + "/log"]);
    }
    // The menu bar's and Spotlight's "Install updates": Software Update
    function install() {
        Panels.openSettings("update");
        SoftwareUpdate.check();
    }

    FileView {
        id: statusFile
        path: root.dir + "/status.json"
        blockLoading: false
        printErrors: false
        onLoaded: {
            let j = null;
            try {
                j = JSON.parse(text());
            } catch (e) {
                return;
            }
            // The last run's file, read before the new run wrote its own
            if (root.requested > 0 && (j.started ?? 0) < root.requested - 2)
                return;
            // A run that finished long ago (before this shell) isn't news
            if (!root.job && j.state !== "running" && Date.now() / 1000 - (j.finished ?? 0) > 3600)
                return;
            const was = root.job?.state;
            root.job = j;
            if (j.state !== "running" && was === "running")
                root.finished(j);
        }
    }
    // While it runs: the status every half second (a small file, replaced
    // rather than appended, so it's read again rather than watched)
    Timer {
        id: poll
        interval: 500
        repeat: true
        running: root.running
        onTriggered: statusFile.reload()
    }
    // The output, while Show details is open: every 1.5 s, only what was
    // added since (AUR builds can write megabytes; reading it all twice a
    // second, again and again, kept a core busy)
    Timer {
        interval: 1500
        repeat: true
        running: root.running && root.logUsers > 0
        onTriggered: root.readLog()
    }
    // Is the job still alive (does it hold its lock)? Once 5 s in, then
    // every 10 s
    Timer {
        id: lockTimer
        interval: 5000
        repeat: true
        running: root.job?.state === "running"
        onRunningChanged: interval = 5000
        onTriggered: {
            interval = 10000;
            if (!lockCheck.running)
                lockCheck.running = true;
        }
    }
    // A "running" status nobody holds the lock for: the job died (machine
    // went down, killed); say so instead of showing progress forever
    Process {
        id: lockCheck
        command: ["flock", "-n", root.dir + "/lock", "true"]
        onExited: code => {
            if (code === 0 && root.job?.state === "running") {
                // (just started: the job may not have taken the lock yet)
                if (Date.now() / 1000 - (root.job.started ?? 0) > 5)
                    root.interrupted = true;
            }
        }
    }
    // Reading the output: from logSize on, up to the size the file has now
    // (a smaller file is a new run: start over), keeping the last 2000 lines
    property real logSize: 0
    function readLog() {
        if (!logProc.running)
            logProc.running = true;
    }
    Process {
        id: logProc
        command: ["sh", "-c", "f=$1; o=$2; s=$(stat -c %s \"$f\" 2>/dev/null || echo 0); [ \"$s\" -lt \"$o\" ] && o=0 && echo '@@reset'; [ \"$s\" -gt \"$o\" ] && tail -c +$((o + 1)) \"$f\" | head -c $((s - o)); printf '\\n@@size %s\\n' \"$s\"", "sh", root.dir + "/log", String(Math.round(root.logSize))]
        stdout: StdioCollector {
            onStreamFinished: {
                const m = text.match(/\n@@size (\d+)\n?$/);
                if (!m)
                    return;
                let body = text.slice(0, m.index);
                let base = root.log;
                if (body.startsWith("@@reset\n")) {
                    body = body.slice(8);
                    base = "";
                }
                const all = (base + body).split("\n");
                root.log = (all.length > 2000 ? all.slice(all.length - 2000) : all).join("\n");
                root.logSize = parseInt(m[1]);
            }
        }
    }
    onLogUsersChanged: if (logUsers > 0) readLog()

    // Done: an alert in the island (a notification without it), and a fresh check
    signal finished(var j)
    onFinished: j => {
        if (logUsers > 0)
            readLog();
        check.running = true;
        if (j.state === "cancelled")
            return;
        const ok = j.state === "done";
        // iRice's part follows (its update screen): no alert in between
        if (ok && (j.then?.irice || (j.then?.rebuild?.length ?? 0) > 0))
            return;
        const n = (j.upgraded ?? []).length;
        const title = ok ? (n > 0 ? n + " update" + (n === 1 ? "" : "s") + " installed" : j.text) : j.text;
        const sub = ok ? (j.restart ? "Restart to finish: the kernel or system libraries changed" : "Your system is up to date") : (j.error || "See Software Update for details").split("\n").slice(-1)[0];
        if (Island.enabled && Island.wants("sysupdate"))
            Island.post({ kind: "sysupdate", icon: ok ? (j.restart ? "arrow-clockwise" : "check-circle") : "warning-circle", title: title, subtitle: sub, color: ok ? Theme.accents.green : Theme.color.danger, action: ok && j.restart ? "Restart" : "Details", onAction: () => ok && j.restart ? Quickshell.execDetached(["systemctl", "reboot"]) : Panels.openSettings("update") }, 8000);
        else
            Quickshell.execDetached(["notify-send", "-a", "iRice", "-i", ok ? "system-software-update" : "dialog-error", title, sub]);
    }

    // ── What's available ────────────────────────────────────────────────
    function refresh() {
        if (!check.running)
            check.running = true;
    }
    // (no periodic check: only Check for updates looks; the status file is
    // read once at start, to re-attach to a run that outlived the last shell)
    Process {
        id: check
        onRunningChanged: if (running) root.checking = true
        command: ["sh", "-c", "command -v checkupdates >/dev/null || exit 3; checkupdates 2>/dev/null; echo '--aur--'; h=$(command -v yay || command -v paru) && \"$h\" -Qua 2>/dev/null; exit 0"]
        stdout: StdioCollector {
            onStreamFinished: {
                const [repo, aur] = (text + "\n").split("--aur--\n");
                const parse = s => s.split("\n").map(l => l.trim().split(/\s+/)).filter(f => f.length >= 4 && f[2] === "->").map(f => ({ name: f[0], from: f[1], to: f[3] }));
                root.list = parse(repo ?? "");
                root.count = root.list.length;
                root.aurList = parse(aur ?? "");
                root.aurCount = root.aurList.length;
            }
        }
        onExited: code => {
            root.available = code !== 3;
            root.checking = false;
        }
    }
}
