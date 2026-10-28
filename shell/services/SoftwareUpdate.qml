pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Software Update's one button: iRice and the system together, in a safe
// order. Nothing is checked on its own; Check for updates looks at:
//   iRice    git fetch (IriceUpdate: what's new, conflicts to decide), and the
//            new version's compat.json (tested Hyprland) and packages.sh
//   system   checkupdates, and yay/paru -Qua for the AUR (Updates)
//   packages iRice's missing ones, and ones it dropped (a checkbox)
// and makes one plan. Update runs it:
//   1. the system part (sysupdate.py; one password for everything as root):
//      snapshot (snapper / timeshift on btrfs) → pacman -Syu [holding
//      Hyprland back if you chose to] → iRice's missing packages → the
//      dropped ones; then the AUR (as you; a second password only if any)
//   2. iRice (IriceUpdate.launch → update-run.sh's screen): the new version,
//      rebuilt against what's now installed (the glass plugin too when
//      Hyprland changed), and the shell restarts
// System first, so iRice builds against the final versions; if it fails,
// iRice isn't touched. Only one part when only one has changes.
Singleton {
    id: root

    // ── Check ───────────────────────────────────────────────────────────
    property bool checked: false // a check finished (this session)
    readonly property bool checking: IriceUpdate.checking || Updates.checking || IriceUpdate.packagesChecking || compatPending || compatProc.running || hyprProc.running || snapProc.running
    onCheckingChanged: if (!checking && asked) {
        asked = false;
        checked = true;
    }
    property bool asked: false
    function check() {
        if (running || checking)
            return;
        asked = true;
        hold = -1;
        // This check's compat.json is read once the iRice check has said which
        // version would be installed; until then the plan can't be trusted
        // (an old compat could let an untested Hyprland through unasked)
        compat = null;
        compatPending = true;
        IriceUpdate.check(); // (fetches, then reads the new version's packages)
        Updates.refresh();
        hyprProc.running = true;
        snapProc.running = true;
    }
    // The tested Hyprland range of the version that would be installed
    property var compat: null
    property bool compatPending: false // a check is waiting for its compat.json
    function readCompat() {
        if (!compatPending || compatProc.running || IriceUpdate.checking)
            return;
        const r = IriceUpdate.report;
        compatProc.ref = r?.available ? (r.upstream ?? "HEAD") : "HEAD";
        compatProc.running = true;
    }
    Connections {
        target: IriceUpdate
        function onReportChanged() { root.readCompat(); }
        // (the report may not change, e.g. the same result or an error)
        function onCheckingChanged() { root.readCompat(); }
    }
    Process {
        id: compatProc
        property string ref: "HEAD"
        command: ["git", "-C", IriceUpdate.repo, "show", ref + ":compat.json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.compat = JSON.parse(text);
                } catch (e) {
                    root.compat = null;
                }
            }
        }
        onExited: root.compatPending = false
    }
    property string hyprInstalled: ""
    Process {
        id: hyprProc
        command: ["sh", "-c", "pacman -Q hyprland 2>/dev/null | awk '{print $2}'"]
        stdout: StdioCollector { onStreamFinished: root.hyprInstalled = text.trim() }
    }
    // A snapshot tool that's set up: snapper with a root config, or
    // timeshift on btrfs (an rsync snapshot would take far too long)
    property string snapshot: ""
    Process {
        id: snapProc
        command: ["sh", "-c", "if command -v snapper >/dev/null && [ -f /etc/snapper/configs/root ]; then echo snapper; elif command -v timeshift >/dev/null && [ \"$(findmnt -no FSTYPE /)\" = btrfs ]; then echo timeshift; fi"]
        stdout: StdioCollector { onStreamFinished: root.snapshot = root.fakeRun ? "snapper" : text.trim() }
    }

    // Versions: "1:0.56.2-4.1" → [0, 56, 2]; "0.56.x" matches any 0.56
    function parts(v) {
        return String(v ?? "").replace(/^\d+:/, "").replace(/-[^-]*$/, "").split(/[.+]/).map(x => x === "x" ? -1 : (parseInt(x) || 0));
    }
    function cmp(a, b) { // b may hold wildcards (-1)
        for (let i = 0; i < Math.max(a.length, b.length); i++) {
            const x = a[i] ?? 0, y = b[i] ?? 0;
            if (y === -1)
                return 0;
            if (x !== y)
                return x < y ? -1 : 1;
        }
        return 0;
    }
    readonly property var hyprUpdate: Updates.list.find(p => p.name === "hyprland") ?? null
    readonly property string hyprMax: compat?.hyprland?.max ?? ""
    // The repo's Hyprland is newer than this iRice is tested with
    readonly property bool hyprTooNew: !!hyprUpdate && hyprMax !== "" && cmp(parts(hyprUpdate.to), parts(hyprMax)) > 0
    readonly property string hyprShort: hyprUpdate ? parts(hyprUpdate.to).slice(0, 2).join(".") : ""
    readonly property string hyprNow: hyprInstalled !== "" ? parts(hyprInstalled).slice(0, 2).join(".") : ""
    // Your choice: -1 not yet, 1 keep the current Hyprland, 0 update anyway
    property int hold: -1
    // What holding back means: Hyprland and the hypr* / aquamarine libraries
    // it's built with (and anything named for it, like its portal)
    readonly property var held: hold === 1 ? Updates.list.concat(Updates.aurList).map(p => p.name).filter(n => /^(hypr|aquamarine)|hyprland/.test(n)) : []
    property bool removeDropped: true
    // AUR packages are built from scripts anyone can submit, and they're
    // built here without showing you what changed: only when you ask
    property bool includeAur: false

    // ── The plan ────────────────────────────────────────────────────────
    readonly property var plan: {
        const sysCount = Updates.list.filter(p => !held.includes(p.name)).length;
        const aurCount = Updates.aurList.filter(p => !held.includes(p.name)).length;
        const pk = IriceUpdate.packages;
        return {
            irice: IriceUpdate.available,
            system: sysCount > 0 || root.fakeRun, // (the sandbox's simulated run can always be tried)
            aur: includeAur && aurCount > 0,
            install: pk.missing ?? [],
            remove: removeDropped ? (pk.dropped ?? []) : [],
            // Hyprland changes: rebuild the glass plugin against it
            rebuild: !!hyprUpdate && hold !== 1 ? ["plugin"] : [],
            sysCount: sysCount,
            aurCount: aurCount
        };
    }
    readonly property bool anything: plan.irice || plan.system || plan.aur || plan.install.length > 0 || plan.remove.length > 0
    readonly property int undecided: (plan.irice ? IriceUpdate.undecided : 0) + (hyprTooNew && hold === -1 ? 1 : 0)
    readonly property bool canUpdate: checked && !checking && anything && undecided === 0 && !running && (IriceUpdate.allowed || fakeRun)
    // The steps, for the progress view: the system job's, then iRice's
    readonly property var steps: {
        const s = (Updates.job?.steps ?? []).slice();
        const then = Updates.job?.then;
        if (then?.irice || (then?.rebuild?.length ?? 0) > 0 || IriceUpdate.applying)
            s.push("irice", "restart");
        return s;
    }
    readonly property string stepNow: IriceUpdate.applying ? "irice" : Updates.running ? (Updates.job?.step || "") : ""
    function stepName(s) {
        return ({ snapshot: "Snapshot", system: "System", packages: "iRice packages", remove: "Unused packages", aur: "AUR", irice: "iRice", restart: "Restart" })[s] ?? s;
    }

    // ── Update ──────────────────────────────────────────────────────────
    readonly property bool fakeRun: Updates.fake
    readonly property bool running: Updates.running || IriceUpdate.applying
    function update() {
        if (!canUpdate)
            return;
        const p = plan;
        const args = [];
        if (p.system)
            args.push("--system");
        if (snapshot !== "" && (p.system || p.install.length > 0 || p.remove.length > 0))
            args.push("--snapshot", snapshot);
        if (held.length > 0)
            args.push("--ignore", held.join(","));
        if (p.install.length > 0)
            args.push("--install", p.install.join(","));
        if (p.remove.length > 0)
            args.push("--remove", p.remove.join(","));
        if (p.aur)
            args.push("--aur");
        const after = { irice: p.irice, rebuild: p.rebuild, at: Date.now() / 1000 };
        const rootPart = p.system || p.aur || p.install.length > 0 || p.remove.length > 0;
        // (what follows travels with the job: its status keeps it, so a shell
        // restart in between doesn't lose it)
        if (rootPart)
            Updates.update(args.concat(["--then", JSON.stringify(after)]));
        else
            IriceUpdate.launch(after.rebuild);
    }
    function cancel() {
        Updates.cancel();
    }

    // The system part finished: the iRice part (or not)
    Connections {
        target: Updates
        function onFinished(j) {
            const after = j.then;
            if (j.state !== "done" || !after || !(after.irice || (after.rebuild ?? []).length > 0))
                return;
            if (root.fakeRun) {
                Quickshell.execDetached(["notify-send", "-a", "iRice", "Sandbox", "The iRice part would run now (update-run.sh: the update screen, rebuild " + ((after.rebuild ?? []).join(", ") || "nothing") + ", restart)."]);
                return;
            }
            IriceUpdate.launch(after.rebuild ?? []);
        }
    }
}
