import QtQuick
import Quickshell
import qs.common
import qs.services

// Settings drawn in Rust: the "input" page group's side in this shell
// (RustBridge.qml): Mouse & Trackpad, Gestures, Keyboard and Dictation
// (native/tessera-ui/src/surfaces/settings/pages/input). While Settings is
// open (`live`) it tells the pages what only this shell knows (`feed`, the
// pages' cx.feed["input"]): the mice and trackpads (Mouse), the shortcuts
// and gestures (Keymap: keybinds.json), Dictation's and Live Text's state,
// the tools there are, and the import and export's state; and does what
// they ask (`act`: cx.ask("input", what, args)): Keymap's changes (with
// one undo), installs and downloads (through Software Update's job), the
// import and export (file dialogs, previews).
Scope {
    id: root

    property bool live: false

    function plain(x) {
        return x === undefined ? null : JSON.parse(JSON.stringify(x));
    }

    readonly property var feed: live ? ({
            mice: Mouse.mice,
            touchpads: Mouse.touchpads,
            boundaryAvailable: Mouse.boundaryAvailable,
            keymap: {
                ready: Keymap.ready,
                defaultsError: Keymap.defaultsError,
                userError: Keymap.userError,
                saveError: Keymap.saveError,
                modKey: Keymap.modKey,
                hyprDir: Keymap.hyprDir,
                threeFingerDrag: Keymap.threeFingerDrag,
                binds: Keymap.binds.map(b => ({ id: b.id, keys: b.keys, defaultKeys: b.defaultKeys, group: b.group, label: b.label, action: root.plain(b.action), opts: root.plain(b.opts), custom: b.custom, disabled: b.disabled, changed: b.changed, keysChanged: b.keysChanged, keyList: b.keyList, actionText: b.actionText })),
                gestures: Keymap.gestures.map(g => ({ id: g.id, fingers: g.fingers, direction: g.direction, action: root.plain(g.action), label: g.label, custom: g.custom, disabled: g.disabled, changed: g.changed, text: g.text, actionText: g.actionText })),
                gestureDefaults: Keymap.gestureDefaults.map(d => ({ id: d.id, fingers: d.fingers, direction: d.direction, action: root.plain(d.action), label: d.label ?? d.id }))
            },
            dictation: {
                hasBinary: Dictation.hasBinary,
                ready: Dictation.ready,
                installing: Dictation.installing,
                installError: Dictation.installError,
                models: root.plain(Dictation.info.models ?? {}),
                downloading: Dictation.downloading,
                downloadProgress: Dictation.downloadProgress,
                downloadError: Dictation.downloadError,
                lastDownload: Dictation.lastDownload
            },
            tools: { ready: Tools.ready, swappy: Tools.has("swappy"), wtype: Tools.has("wtype") },
            ocr: { hasTesseract: ScreenTools.hasTesseract, installing: ScreenTools.ocrInstalling, error: ScreenTools.ocrInstallError, languages: root.plain(ScreenTools.ocrLanguages) },
            updatesRunning: Updates.running,
            ie: { keys: ieKeys.state, gestures: ieGestures.state }
        }) : null
    onFeedChanged: if (live) RustUi.tell("settings", { feed: { input: feed } })

    // One undo: the keymap as it was before the last change asked from here
    property string snap: ""
    function snapshot() {
        root.snap = Keymap.snapshot();
    }

    function act(what, args) {
        switch (what) {
        case "threeFingerDrag":
            Keymap.setThreeFingerDrag(!!args[0]);
            break;
        // ── Shortcuts ──
        case "kbDisable":
            snapshot();
            Keymap.disable(String(args[0]), !!args[1]);
            break;
        case "kbReset":
            snapshot();
            Keymap.reset(String(args[0]));
            break;
        case "kbRemove":
            snapshot();
            Keymap.removeShortcut(String(args[0]));
            break;
        case "kbSave": { // { editId, keys, takeOver, custom, entry }
            const o = args[0] ?? {};
            snapshot();
            for (const id of o.takeOver ?? [])
                Keymap.disable(String(id), true);
            if (!o.custom) {
                Keymap.setKeys(String(o.editId), String(o.keys));
                break;
            }
            const e = o.entry ?? {};
            let a = e.action ?? null;
            // (an app: its launch, from its desktop entry)
            if (a && a.desktop !== undefined && a.exec === undefined) {
                const app = Apps.byId(a.desktop);
                a = app ? Keymap.appAction(app) : (Keymap.bind(o.editId)?.action ?? null);
            }
            const entry = { keys: String(o.keys), label: String(e.label ?? "").trim() || Keymap.describe(a), action: a, opts: e.opts ?? {} };
            if (o.editId === "")
                Keymap.addShortcut(entry);
            else
                Keymap.updateShortcut(String(o.editId), entry);
            break;
        }
        // ── Gestures ──
        case "gSet":
            Keymap.setGesture(String(args[0]), args[1] ?? {});
            break;
        case "gDisable":
            snapshot();
            Keymap.disableGesture(String(args[0]), !!args[1]);
            break;
        case "gReset":
            snapshot();
            Keymap.resetGesture(String(args[0]));
            break;
        case "gRemove":
            snapshot();
            Keymap.removeGesture(String(args[0]));
            break;
        case "gAdd":
            snapshot();
            Keymap.addGesture(args[0] ?? {});
            break;
        case "undo":
            if (root.snap !== "") {
                Keymap.restore(root.snap);
                root.snap = "";
            }
            break;
        // ── Import and export: [scope, op, …] ──
        case "ie": {
            const s = args[0] === "gestures" ? ieGestures : ieKeys;
            s.run(String(args[1] ?? ""), args.slice(2));
            break;
        }
        // ── Dictation and Live Text ──
        case "dictRefresh":
            Dictation.refresh();
            break;
        case "dictInstall":
            Dictation.install();
            break;
        case "dictDownload":
            Dictation.download(String(args[0]));
            break;
        case "dictCancel":
            Dictation.cancelDownload();
            break;
        case "dictRemove":
            Dictation.removeModel(String(args[0]));
            break;
        case "installOcr":
            ScreenTools.installOcr(String(args[0] ?? ""));
            break;
        default:
            console.warn("settings (input): no act " + what);
        }
    }

    // Import and export (ImportExport.qml's logic), one per page: its
    // state goes in the feed (ie.keys, ie.gestures)
    component IeState: QtObject {
        id: ie
        property bool gesturesOnly: false // (the Gestures page's: its end4 import reads gestures only)
        property string pathMode: "" // "" | export | import (no file dialog installed)
        property var pathSections: []
        property string pathText: ""
        property int pathSeq: 0
        property var preview: null
        property int previewSeq: 0
        property string error: ""
        property string status: ""
        property bool busy: false
        property string undoSnap: ""
        readonly property var state: ({
                busy: busy,
                pathMode: pathMode,
                pathText: pathText,
                pathSeq: pathSeq,
                preview: preview === null ? null : { title: preview.title ?? "", note: preview.note ?? "", summary: preview.summary ?? "", total: preview.total ?? 0, lines: (preview.lines ?? []).map(l => ({ kind: l.kind ?? "", text: l.text ?? "" })), skipped: (preview.skipped ?? []).map(l => ({ text: l.text ?? "" })) },
                previewSeq: previewSeq,
                error: error,
                status: status,
                canUndo: undoSnap !== "",
                dialog: Keymap.hasZenity || Keymap.hasKdialog
            })
        property Timer undoTimer: Timer {
            interval: 8000
            onTriggered: {
                ie.undoSnap = "";
                ie.status = "";
            }
        }
        function defaultPath(secs) {
            return "~/Documents/" + (secs.length === 1 && secs[0] === "gestures" ? "tessera-gestures.json" : "tessera-shortcuts.json");
        }
        function clear() {
            error = "";
            status = "";
            preview = null;
            pathMode = "";
        }
        function askPath(save, secs) {
            pathSections = secs;
            pathMode = save ? "export" : "import";
            pathText = defaultPath(secs);
            pathSeq++;
        }
        // A file dialog; cb(path) on a choice, nothing on Cancel, the path field without one
        function pick(save, secs, cb) {
            const start = Keymap.expand(defaultPath(secs));
            let cmd = null;
            if (Keymap.hasZenity)
                cmd = save ? ["zenity", "--file-selection", "--save", "--confirm-overwrite", "--title=Export shortcuts", "--filename=" + start, "--file-filter=JSON files | *.json"] : ["zenity", "--file-selection", "--title=Import shortcuts", "--filename=" + Keymap.home + "/Documents/", "--file-filter=JSON files | *.json"];
            else if (Keymap.hasKdialog)
                cmd = save ? ["kdialog", "--title", "Export shortcuts", "--getsavefilename", start, "*.json"] : ["kdialog", "--title", "Import shortcuts", "--getopenfilename", Keymap.home + "/Documents", "*.json"];
            if (!cmd) {
                askPath(save, secs);
                return;
            }
            busy = true;
            Keymap.run(Launch.wrap(cmd), (code, out) => {
                ie.busy = false;
                const p = out.trim().split("\n")[0] ?? "";
                if (code === 0 && p !== "")
                    cb(p);
                else if (code !== 1)
                    ie.askPath(save, secs); // (the dialog didn't start: type a path)
            });
        }
        function doExport(path, secs) {
            busy = true;
            Keymap.exportTo(path, secs, (ok, err) => {
                ie.busy = false;
                ie.pathMode = "";
                if (ok)
                    ie.status = "Exported to " + path.replace(Keymap.home, "~");
                else
                    ie.error = "Couldn't write " + path + (err ? " (" + err + ")" : "") + ". Choose a folder you can write to, such as ~/Documents.";
            });
        }
        function doImport(path, secs) {
            busy = true;
            Keymap.readFile(path, (ok, text, err) => {
                ie.busy = false;
                ie.pathMode = "";
                if (!ok) {
                    ie.error = "Couldn't read " + path + (err ? " (" + err.replace(/^cat: /, "") + ")" : "") + ". Check the path and that the file exists.";
                    return;
                }
                const p = Keymap.previewFile(text, secs);
                if (p.error)
                    ie.error = p.error;
                else {
                    ie.preview = p;
                    ie.previewSeq++;
                }
            });
        }
        function run(op, a) {
            switch (op) {
            case "import": {
                const secs = a[0] ?? [];
                clear();
                pick(false, secs, p => doImport(p, secs));
                break;
            }
            case "export": {
                const secs = a[0] ?? [];
                clear();
                pick(true, secs, p => doExport(p, secs));
                break;
            }
            case "end4": {
                clear();
                busy = true;
                const secs = ie.gesturesOnly ? ["gestures"] : ["keybinds", "shortcuts", "gestures"];
                Keymap.previewEnd4(secs.includes("keybinds") || secs.includes("shortcuts"), secs.includes("gestures"), p => {
                    ie.busy = false;
                    if (p.error)
                        ie.error = p.error;
                    else {
                        ie.preview = p;
                        ie.previewSeq++;
                    }
                });
                break;
            }
            case "go": {
                const path = String(a[0] ?? "").trim();
                if (path === "" || busy)
                    break;
                if (pathMode === "export")
                    doExport(path, pathSections);
                else
                    doImport(path, pathSections);
                break;
            }
            case "cancelPath":
                pathMode = "";
                break;
            case "apply": {
                const s = Keymap.snapshot();
                Keymap.applyPreview(preview);
                preview = null;
                undoSnap = s;
                status = "Imported.";
                undoTimer.restart();
                break;
            }
            case "cancelPreview":
                preview = null;
                break;
            case "undo":
                if (undoSnap !== "") {
                    Keymap.restore(undoSnap);
                    undoSnap = "";
                    status = "Import undone.";
                    undoTimer.restart();
                }
                break;
            case "dismiss":
                error = "";
                break;
            }
        }
    }
    IeState {
        id: ieKeys
    }
    IeState {
        id: ieGestures
        gesturesOnly: true
    }
}
