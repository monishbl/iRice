import QtQuick
import Quickshell
import qs.common
import qs.services

// Settings drawn in Rust: the "about" page group's side in this shell
// (RustBridge.qml). While Settings is open (`live`) it tells the pages
// what only this shell knows (`feed`, the pages' cx.feed["about"]) and does
// what they ask (`act`: cx.ask("about", what, args)).
//
// Software Update (SoftwareUpdate, TesseraUpdate, Updates, BootSnapshots:
// the root helper and pacman stay this shell's), System health (Health,
// Conflicts), Report a Bug (BugReport), the cloud drive folders
// (CloudDrives: Settings → Google and Microsoft 365) and shortcut and
// gesture import and export (Keymap: Settings → Keyboard and Gestures). The
// About page's system info, Back up and restore and Login Items are
// tessera-ui's own (src/surfaces/settings/pages/about).
Scope {
    id: root

    property bool live: false

    // The page on show: what opening it does in the QML pages
    // (Component.onCompleted), done when Settings in Rust shows it
    readonly property string page: live ? Panels.settingsPage : ""
    onPageChanged: {
        if (page === "update") {
            SoftwareUpdate.checkUndo();
            BootSnapshots.refresh();
        } else if (page === "about/health") {
            Health.check();
        } else if (page === "about/bugreport") {
            if (BugReport.log === "" && !BugReport.gathering)
                BugReport.gather();
        }
    }
    // The update's full output is read only while someone shows it
    readonly property bool logShown: page === "update" && Updates.showDetails && !!Updates.job
    onLogShownChanged: Updates.logUsers += logShown ? 1 : -1

    function home(p) {
        return String(p ?? "").replace(Quickshell.env("HOME"), "~");
    }
    function pkgs(l) {
        return (l ?? []).map(x => ({ name: x.name ?? "", from: x.from ?? "", to: x.to ?? "" }));
    }

    // ── Import and export (ImportExport.qml's state, one per section set) ─
    property var ie: ({}) // key -> { pathMode, pathSections, preview, error, status, busy, undoSnap }
    function ieGet(key) {
        return Object.assign({ pathMode: "", pathSections: [], preview: null, error: "", status: "", busy: false, undoSnap: "" }, ie[key] ?? {});
    }
    function ieSet(key, o) {
        const n = Object.assign({}, ie);
        n[key] = Object.assign(ieGet(key), o);
        ie = n;
    }
    function ieDefault(secs) {
        return "~/Documents/" + (secs.length === 1 && secs[0] === "gestures" ? "tessera-gestures.json" : "tessera-shortcuts.json");
    }
    function iePick(key, save, secs, cb) {
        const start = Keymap.expand(ieDefault(secs));
        let cmd = null;
        if (Keymap.hasZenity)
            cmd = save ? ["zenity", "--file-selection", "--save", "--confirm-overwrite", "--title=Export shortcuts", "--filename=" + start, "--file-filter=JSON files | *.json"] : ["zenity", "--file-selection", "--title=Import shortcuts", "--filename=" + Keymap.home + "/Documents/", "--file-filter=JSON files | *.json"];
        else if (Keymap.hasKdialog)
            cmd = save ? ["kdialog", "--title", "Export shortcuts", "--getsavefilename", start, "*.json"] : ["kdialog", "--title", "Import shortcuts", "--getopenfilename", Keymap.home + "/Documents", "*.json"];
        if (!cmd) {
            ieSet(key, { pathSections: secs, pathMode: save ? "export" : "import", pathText: ieDefault(secs) });
            return;
        }
        ieSet(key, { busy: true });
        Keymap.run(Launch.wrap(cmd), (code, out) => {
            ieSet(key, { busy: false });
            const p = out.trim().split("\n")[0] ?? "";
            if (code === 0 && p !== "")
                cb(p);
            else if (code !== 1)
                ieSet(key, { pathSections: secs, pathMode: save ? "export" : "import", pathText: ieDefault(secs) });
        });
    }
    function ieExport(key, path, secs) {
        ieSet(key, { busy: true });
        Keymap.exportTo(path, secs, (ok, err) => {
            if (ok)
                ieSet(key, { busy: false, pathMode: "", status: "Exported to " + path.replace(Keymap.home, "~") });
            else
                ieSet(key, { busy: false, pathMode: "", error: "Couldn't write " + path + (err ? " (" + err + ")" : "") + ". Choose a folder you can write to, such as ~/Documents." });
        });
    }
    function ieImport(key, path, secs) {
        ieSet(key, { busy: true });
        Keymap.readFile(path, (ok, text, err) => {
            ieSet(key, { busy: false, pathMode: "" });
            if (!ok) {
                ieSet(key, { error: "Couldn't read " + path + (err ? " (" + err.replace(/^cat: /, "") + ")" : "") + ". Check the path and that the file exists." });
                return;
            }
            const p = Keymap.previewFile(text, secs);
            ieSet(key, p.error ? { error: p.error } : { preview: p });
        });
    }
    property var ieTimers: ({})
    Component {
        id: undoTimer
        Timer {
            property string key
            interval: 8000
            onTriggered: {
                root.ieSet(key, { undoSnap: "", status: "" });
                destroy();
            }
        }
    }
    function ieUndoLater(key) {
        undoTimer.createObject(root, { key: key }).start();
    }
    function ieAct(what, a) {
        const key = String(a[0] ?? "");
        const secs = (a[1] ?? []).map(String);
        const clear = { error: "", status: "", preview: null, pathMode: "" };
        switch (what) {
        case "export":
            ieSet(key, clear);
            iePick(key, true, secs, p => ieExport(key, p, secs));
            break;
        case "import":
            ieSet(key, clear);
            iePick(key, false, secs, p => ieImport(key, p, secs));
            break;
        case "end4": {
            ieSet(key, Object.assign({ busy: true }, clear));
            Keymap.previewEnd4(secs.includes("keybinds") || secs.includes("shortcuts"), secs.includes("gestures"), p => ieSet(key, p.error ? { busy: false, error: p.error } : { busy: false, preview: p }));
            break;
        }
        case "path": { // the typed path, when there's no file dialog
            const s = ieGet(key);
            const p = String(a[2] ?? "").trim();
            if (p === "")
                break;
            if (s.pathMode === "export")
                ieExport(key, p, s.pathSections);
            else
                ieImport(key, p, s.pathSections);
            break;
        }
        case "pathCancel":
            ieSet(key, { pathMode: "" });
            break;
        case "apply": {
            const snap = Keymap.snapshot();
            Keymap.applyPreview(ieGet(key).preview);
            ieSet(key, { preview: null, undoSnap: snap, status: "Imported." });
            ieUndoLater(key);
            break;
        }
        case "close":
            ieSet(key, { preview: null });
            break;
        case "dismiss":
            ieSet(key, { error: "" });
            break;
        case "undo":
            Keymap.restore(ieGet(key).undoSnap);
            ieSet(key, { undoSnap: "", status: "Import undone." });
            ieUndoLater(key);
            break;
        }
    }

    readonly property var feed: live ? ({
            su: {
                running: SoftwareUpdate.running,
                checking: SoftwareUpdate.checking,
                checked: SoftwareUpdate.checked,
                anything: SoftwareUpdate.anything,
                restartPending: SoftwareUpdate.restartPending,
                hyprNow: SoftwareUpdate.hyprNow,
                hyprTooNew: SoftwareUpdate.hyprTooNew,
                hyprShort: SoftwareUpdate.hyprShort,
                hyprMax: SoftwareUpdate.compat?.hyprland?.max ?? "",
                hold: SoftwareUpdate.hold,
                heldHypr: SoftwareUpdate.heldHypr,
                held: SoftwareUpdate.held,
                snapshot: SoftwareUpdate.snapshot,
                includeAur: SoftwareUpdate.includeAur,
                removeDropped: SoftwareUpdate.removeDropped,
                undecided: SoftwareUpdate.undecided,
                canUpdate: SoftwareUpdate.canUpdate,
                plan: {
                    system: !!SoftwareUpdate.plan.system,
                    aur: !!SoftwareUpdate.plan.aur,
                    sysCount: SoftwareUpdate.plan.sysCount ?? 0,
                    aurCount: SoftwareUpdate.plan.aurCount ?? 0,
                    install: (SoftwareUpdate.plan.install ?? []).length,
                    remove: (SoftwareUpdate.plan.remove ?? []).length,
                    setup: !!SoftwareUpdate.plan.setup,
                    tessera: !!SoftwareUpdate.plan.tessera,
                    rebuild: (SoftwareUpdate.plan.rebuild ?? []).length
                },
                steps: SoftwareUpdate.steps.map(s => ({ name: SoftwareUpdate.stepName(s), now: s === SoftwareUpdate.stepNow })),
                undoCount: SoftwareUpdate.undoCount,
                undoMissing: SoftwareUpdate.undo.missing ?? [],
                undoPackages: (SoftwareUpdate.undo.packages ?? []).map(p => p.name),
                undoLong: SoftwareUpdate.undo.at ? Qt.formatDateTime(new Date(SoftwareUpdate.undo.at * 1000), "d MMMM, h:mm ap") : "",
                undoDay: SoftwareUpdate.undo.at ? Qt.formatDateTime(new Date(SoftwareUpdate.undo.at * 1000), "dddd h:mm ap") : "",
                canGoBack: SoftwareUpdate.canGoBack,
                skippedAll: SoftwareUpdate.skippedAll.map(p => ({ name: p.name, to: p.to })),
                includeSkipped: SoftwareUpdate.includeSkipped
            },
            tu: {
                allowed: TesseraUpdate.allowed,
                available: TesseraUpdate.available,
                error: TesseraUpdate.error,
                applying: TesseraUpdate.applying,
                report: TesseraUpdate.report ? { current: TesseraUpdate.report.current ?? "", latest: TesseraUpdate.report.latest ?? "", changelog: TesseraUpdate.report.changelog ?? [], notes: TesseraUpdate.report.notes ?? [] } : null,
                result: TesseraUpdate.result ? { backup: root.home(TesseraUpdate.result.backup ?? ""), now: TesseraUpdate.result.now ?? "", changelog: TesseraUpdate.result.changelog ?? [], notes: TesseraUpdate.result.notes ?? [] } : null,
                conflicts: TesseraUpdate.conflicts.map(c => ({ id: c.id, kind: c.kind, title: c.title ?? "", detail: c.detail ?? "", mine: c.mine ?? "", theirs: c.theirs ?? "", choices: c.choices ?? [], labels: c.labels ?? {}, change: c.change ?? "" })),
                choices: TesseraUpdate.choices,
                missing: TesseraUpdate.packages.missing ?? [],
                dropped: TesseraUpdate.packages.dropped ?? [],
                reasons: TesseraUpdate.packages.reasons ?? {}
            },
            up: {
                job: Updates.job ? { state: Updates.job.state ?? "", kind: Updates.job.kind ?? "", text: Updates.job.text ?? "", phase: Updates.job.phase ?? "", package: Updates.job.package ?? "", error: Updates.job.error ?? "", restart: Updates.job.restart === true, upgraded: (Updates.job.upgraded ?? []).length, aur: (Updates.job.steps ?? []).includes("aur") } : null,
                list: root.pkgs(Updates.list),
                aurList: root.pkgs(Updates.aurList),
                fake: Updates.fake,
                interrupted: Updates.interrupted,
                cancellable: Updates.cancellable,
                running: Updates.running,
                progress: Updates.progress,
                showDetails: Updates.showDetails,
                log: root.logShown ? Updates.log.replace(/^@@step .*\n/gm, "") : ""
            },
            bs: {
                booted: BootSnapshots.booted,
                assistant: BootSnapshots.assistant,
                canMenu: BootSnapshots.canMenu,
                grubBtrfs: BootSnapshots.grubBtrfs,
                menuOn: BootSnapshots.menuOn,
                error: BootSnapshots.error,
                busy: BootSnapshots.busy
            },
            needsInstalling: Needs.installing,
            health: {
                checks: Health.checks.map(c => ({ id: c.id, name: c.name ?? "", group: c.group ?? "", status: c.status ?? "", detail: c.detail ?? "", fix: c.fix ?? "", canFix: !!c.canFix, fixRoot: !!c.fixRoot, fixPassword: !!c.fixPassword, fixCommand: c.fixCommand ?? "" })),
                groups: Health.groups,
                checking: Health.checking,
                fixing: Health.fixing,
                error: Health.error,
                checkedAt: Health.checkedAt > 0 ? Qt.formatTime(new Date(Health.checkedAt), TimeFormat.hm) : ""
            },
            conflicts: Conflicts.found.map(c => ({ id: c.id, name: c.name ?? "", what: c.what ?? "", fix: c.fix ?? "", muted: Conflicts.muted.includes(c.id) })),
            bug: {
                title: BugReport.title,
                happened: BugReport.happened,
                steps: BugReport.steps,
                expected: BugReport.expected,
                hasShot: BugReport.hasShot,
                shot: BugReport.shot,
                shotVersion: BugReport.shotVersion,
                includeShot: BugReport.includeShot,
                includeLog: BugReport.includeLog,
                includeInfo: BugReport.includeInfo,
                gathering: BugReport.gathering,
                log: BugReport.log,
                info: Object.keys(BugReport.sysinfo).map(k => k + ": " + BugReport.sysinfo[k]).join("\n"),
                state: BugReport.state_,
                error: BugReport.error,
                waitSeconds: BugReport.waitSeconds,
                issueUrl: BugReport.issueUrl
            },
            welcomeName: Today.accountName,
            healthChecks: Health.checks.length,
            healthAttention: Health.attention.length,
            drives: {
                hasRclone: CloudDrives.hasRclone,
                installing: CloudDrives.installing,
                installError: CloudDrives.installError,
                busy: CloudDrives.busy,
                errors: CloudDrives.errors,
                names: CloudDrives.names,
                gdrive: CloudDrives.drive("gdrive"),
                onedrive: CloudDrives.drive("onedrive"),
                updatesRunning: Updates.running
            },
            keymap: {
                ready: Keymap.ready,
                dialog: Keymap.hasZenity || Keymap.hasKdialog
            },
            ie: root.ie
        }) : null
    onFeedChanged: if (live) RustUi.tell("settings", { feed: { about: feed } })

    function act(what, args) {
        const a = args ?? [];
        if (what.startsWith("ie.")) {
            root.ieAct(what.slice(3), a);
            return;
        }
        switch (what) {
        // Software Update
        case "su.check":
            SoftwareUpdate.check();
            break;
        case "su.update":
            SoftwareUpdate.update();
            break;
        case "su.cancel":
            SoftwareUpdate.cancel();
            break;
        case "su.goBack":
            SoftwareUpdate.goBack();
            break;
        case "su.hold":
            SoftwareUpdate.hold = Number(a[0]) === 0 ? 1 : 0;
            break;
        case "su.includeAur":
            SoftwareUpdate.includeAur = !!a[0];
            break;
        case "su.removeDropped":
            SoftwareUpdate.removeDropped = !!a[0];
            break;
        case "su.includeSkip":
            SoftwareUpdate.includeSkip(String(a[0] ?? ""), !!a[1]);
            break;
        case "tu.choose":
            TesseraUpdate.choose(String(a[0] ?? ""), String(a[1] ?? ""));
            break;
        case "tu.dismissResult":
            TesseraUpdate.dismissResult();
            break;
        case "up.dismiss":
            Updates.dismiss();
            break;
        case "up.copyLog":
            Updates.copyLog();
            break;
        case "up.showDetails":
            Updates.showDetails = !!a[0];
            break;
        case "reboot":
            Quickshell.execDetached(["systemctl", "reboot"]);
            break;
        case "bs.openAssistant":
            BootSnapshots.openAssistant();
            break;
        case "bs.setMenu":
            BootSnapshots.setMenu(!!a[0]);
            break;
        case "needs.install": // (only the packages the pages name)
            if (["btrfs-assistant", "grub-btrfs"].includes(String(a[0])))
                Needs.install([String(a[0])]);
            break;
        // System health
        case "health.check":
            Health.check();
            break;
        case "health.copy":
            Health.copyReport();
            break;
        case "health.fix":
            Health.fix(String(a[0] ?? ""));
            break;
        case "conflicts.mute":
            Conflicts.mute([String(a[0] ?? "")]);
            break;
        case "conflicts.unmute":
            Conflicts.unmute(String(a[0] ?? ""));
            break;
        // Report a Bug
        case "bug.set":
            if (["title", "happened", "steps", "expected"].includes(a[0]))
                BugReport[a[0]] = String(a[1] ?? "");
            break;
        case "bug.include":
            if (["includeShot", "includeLog", "includeInfo"].includes(a[0]))
                BugReport[a[0]] = !!a[1];
            break;
        case "bug.editShot":
            BugReport.editShot();
            break;
        case "bug.copy":
            BugReport.copy();
            break;
        case "bug.openGithub":
            BugReport.openGithub();
            break;
        case "bug.send":
            BugReport.send();
            break;
        case "bug.openIssue":
            if (/^https:\/\//.test(BugReport.issueUrl))
                Launch.open(BugReport.issueUrl);
            break;
        // About
        case "welcome":
            Panels.welcomeRequested();
            break;
        case "focusReload": // (a restore wrote the Focus modes)
            Focus.reload();
            break;
        case "openFile": // (a folder or file of yours, in Files)
            if (String(a[0] ?? "").startsWith("/"))
                Launch.open("file://" + a[0]);
            break;
        // Cloud drive folders
        case "drives.install":
            CloudDrives.install();
            break;
        case "drives.open":
            CloudDrives.open(String(a[0] ?? ""));
            break;
        case "drives.act":
            if (["mount", "disconnect", "connect"].includes(a[0]))
                CloudDrives.act(String(a[0]), String(a[1] ?? ""));
            break;
        case "drives.cancel":
            CloudDrives.cancel();
            break;
        default:
            console.warn("settings (about): no act " + what);
        }
    }
}
