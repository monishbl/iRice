pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import "../common/LiveTextLogic.js" as Logic

// Live Text: the text in a picture, read on this computer by tesseract
// (nothing is uploaded). read() gives a picture's words with their boxes
// (LiveTextLayer: select them in the screenshot editor and Quick
// Preview). The screenshot index lets Search find a shot by its words: built
// in the background by one low-priority process, only for screenshots it
// hasn't read yet, and not on battery below 30% or in power saver
// (assets/scripts/shot-index.py; $XDG_STATE_HOME/irice, owner-only).
Singleton {
    id: root

    readonly property bool available: ScreenTools.hasTesseract

    // ── A picture's words ─────────────────────────────────────────────────
    // read(path, key) → done(key, { status: ready | none | missing | error,
    // width, height, words }) for whoever asked with that key (path#key: key
    // changes when the file does, as the editor saves). The last few are kept.
    signal done(string key, var result)
    property var recent: [] // [{ key, result }], newest first
    property var pending: ({}) // key → true while tesseract reads it
    function keyOf(path, key) {
        return path + "#" + (key ?? "");
    }
    function read(path, key) {
        const k = keyOf(path, key);
        const hit = recent.find(r => r.key === k);
        if (hit) {
            done(k, hit.result);
            return;
        }
        if (!available) {
            done(k, { status: "missing", width: 0, height: 0, words: [] });
            return;
        }
        if (pending[k])
            return;
        pending[k] = true;
        const p = readJob.createObject(root, { key: k, command: ["tesseract", path, "stdout", "-l", ScreenTools.ocrLangArg, "tsv"] });
        p.running = true;
    }
    function finished(k, code, out) {
        delete pending[k];
        if (code !== 0) {
            done(k, { status: "error", width: 0, height: 0, words: [] });
            return;
        }
        const r = Logic.parseTsv(out);
        r.status = r.words.length > 0 ? "ready" : "none";
        recent = [{ key: k, result: r }].concat(recent.filter(x => x.key !== k)).slice(0, 4);
        done(k, r);
    }
    Component {
        id: readJob
        Process {
            id: job
            property string key
            property string out: ""
            property bool streamed: false
            property int code: -1
            // (once both the exit and the end of its output are in)
            function finish() {
                if (!streamed || code < 0)
                    return;
                root.finished(key, code, out);
                destroy();
            }
            stdout: StdioCollector {
                onStreamFinished: {
                    job.out = text;
                    job.streamed = true;
                    job.finish();
                }
            }
            onExited: c => {
                job.code = c;
                job.finish();
            }
        }
    }

    // ── Quick actions ─────────────────────────────────────────────────────
    // A found number opens a phone app only when one handles tel: links;
    // otherwise it's copied
    property bool telHandler: false
    Process {
        running: true
        command: ["xdg-mime", "query", "default", "x-scheme-handler/tel"]
        stdout: StdioCollector { onStreamFinished: root.telHandler = text.trim() !== "" }
    }
    // a: { kind: link | email | phone | wifi, value, label, wifi }. Returns
    // what to say when it copied instead ("" when it opened something)
    function act(a) {
        if (a.kind === "link")
            Launch.openLink(a.value); // (web links only)
        else if (a.kind === "email")
            Launch.openLink("mailto:" + a.value);
        else if (a.kind === "phone" && telHandler)
            Launch.open("tel:" + a.value.replace(/[^+\d]/g, ""));
        else if (a.kind === "phone")
            return copy(a.value, "Number copied");
        else if (a.kind === "wifi" && WifiShare.joinable(a.wifi))
            WifiShare.join(a.wifi);
        else if (a.kind === "wifi")
            return copy(a.wifi.password, "Password copied");
        return "";
    }
    function copy(text, what) {
        Clipboard.copyText(text); // (through stdin)
        return what || "Copied";
    }

    // ── Finding screenshots by their text (Search) ─────────────────────────
    readonly property bool indexing: Config.options.screenshot.textSearch ?? true
    readonly property string indexFile: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/irice/screenshot-text.json"
    readonly property string script: Quickshell.shellDir + "/assets/scripts/shot-index.py"
    readonly property bool mayIndex: Logic.mayIndex({ onBattery: Power.onBattery, percent: Power.percent, saver: Power.mode === "power-saver" })
    property var shots: ({}) // the index, once Search first asks
    property bool wanted: false
    function search(query, limit) {
        if (!indexing)
            return [];
        wanted = true;
        return Logic.search(shots, query, limit);
    }
    FileView {
        id: indexView
        path: root.wanted && root.indexing ? root.indexFile : ""
        blockLoading: false
        printErrors: false
        onLoaded: {
            try {
                root.shots = JSON.parse(text()).shots ?? {};
            } catch (e) {
                root.shots = {};
            }
        }
    }

    // Opened from Search: Quick Preview shows it with what you searched for
    // selected (previewQuery, taken once)
    property string previewPath: ""
    property string previewQuery: ""
    function preview(path, query) {
        previewPath = path;
        previewQuery = query;
        Panels.quickLook(path);
    }
    function takeQuery(path) {
        if (path !== previewPath)
            return "";
        const q = previewQuery;
        previewPath = "";
        previewQuery = "";
        return q;
    }

    // The index, in the background: a while after a screenshot, and at start
    property bool waiting: false // asked while it may not run: runs once it may
    function indexSoon() {
        if (indexing)
            indexDelay.restart();
    }
    Timer {
        id: indexDelay
        interval: 20000 // (the screenshot's own Live Text goes first)
        onTriggered: root.indexNow()
    }
    function indexNow() {
        if (!indexing || !available || indexer.running)
            return;
        waiting = !mayIndex;
        if (!waiting)
            indexer.running = true;
    }
    onMayIndexChanged: {
        if (mayIndex && waiting)
            indexSoon();
        else if (!mayIndex && indexer.running) {
            indexer.running = false; // (what it read so far is kept)
            waiting = true;
        }
    }
    onIndexingChanged: {
        if (indexing)
            return indexSoon();
        // Off: the index goes too
        indexer.running = false;
        shots = {};
        Quickshell.execDetached(["rm", "-f", "--", indexFile, indexFile + ".tmp"]);
    }
    Process {
        id: indexer
        command: ["nice", "-n", "19", "ionice", "-c", "3", "env", "OMP_THREAD_LIMIT=1", "python3", "-I", root.script, "run", ScreenTools.shotsDir, root.indexFile, ScreenTools.ocrLangArg]
        onExited: if (root.wanted)
            indexView.reload()
    }
    Component.onCompleted: Startup.later(() => indexSoon())
}
