pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// tessera-events (native/tessera-events, built by tools/build-native.sh): one
// process that follows the system for the whole shell (logind, udev, the
// sound server, the network, badges, Cast devices, the camera, location,
// screen sharing, copies, the clipboard history) and holds logind
// inhibitors, in place of a monitor process and a watch.sh wrapper for each.
// It writes a JSON line per event; services ask for a source with an
// EventWatch and listen to the signals below.
//
// Without it (not built, or it keeps ending) `ready` is false and every
// service runs its own watcher as before. covers(src): tessera-events follows
// src; fallsBack(src): the service's own watcher should run. Neither is
// true for the moment before tessera-events says it's ready, so watchers
// aren't started only to be stopped.
Singleton {
    id: root

    readonly property string binary: Quickshell.env("TESSERA_EVENTS") || Quickshell.env("IRICE_EVENTS") || Paths.data + "/bin/tessera-events"
    // (or the one built while Tessera was called iRice, until the next
    // rebuild: tools/build-native.sh)
    readonly property string olderBinary: Paths.data + "/bin/irice-events"
    property string found: ""
    property bool canStart: false // (once the command has the binary found: set after it)
    Process {
        running: true
        command: ["sh", "-c", "for b; do [ -x \"$b\" ] && { echo \"$b\"; exit 0; }; done; exit 1", "sh", root.binary, root.olderBinary]
        stdout: StdioCollector {
            onStreamFinished: {
                root.found = text.trim();
                if (root.found)
                    Qt.callLater(() => root.canStart = true);
                else {
                    root.quickEnds = 3; // (not built)
                    root.decided = true;
                }
            }
        }
    }
    property bool ready: false // running, and said it's ready
    property bool decided: false // ready, or known not to be
    property var unsupported: ({}) // sources it can't follow here (their watchers run)
    property bool pulseUp: false // connected to the sound server
    function covers(src) {
        return ready && !unsupported[src];
    }
    function fallsBack(src) {
        return decided && !covers(src);
    }

    signal login1(string kind, var args) // logind's signal: "PrepareForSleep", [true]
    signal launcherEntry(string app, var props) // "application://x.desktop", { count, "count-visible" }
    signal networkChanged // (nm or net: something to look at)
    signal avahiFound(string key, var service) // { name, host, port, txt }
    signal avahiLost(string key)
    signal avahiDown // (Avahi went: everything it knew is gone)
    signal udev(var props) // KEY: value, as `udevadm monitor --property` prints
    signal pulse(string line) // as `pactl subscribe` prints it
    signal camera(var apps)
    signal location(var apps) // desktop ids; "?" for one GeoClue can't name
    signal screenshare(var graph)
    signal copies(var items)
    signal clipboardChanged
    signal inhibited(string what, bool on, string error)

    // ── What services want ──────────────────────────────────────────────
    // id -> { src, args }, for each EventWatch that's on. A source is on
    // while anyone wants it; its args merge (lists join, the rest: last).
    property var wishes: ({})
    property int lastId: 0
    property var sent: ({}) // src -> the command sent for it last
    function join() {
        return ++lastId;
    }
    function want(id, src, on, args) {
        const w = Object.assign({}, wishes);
        const was = w[id]?.src;
        if (on)
            w[id] = { src: src, args: args ?? {} };
        else
            delete w[id];
        wishes = w;
        sync(src);
        if (was && was !== src)
            sync(was); // (its source changed: the old one may go off)
    }
    function sync(src) {
        if (!ready || !src)
            return;
        let on = false;
        const args = {};
        for (const w of Object.values(wishes)) {
            if (w.src !== src)
                continue;
            on = true;
            for (const [k, v] of Object.entries(w.args))
                args[k] = Array.isArray(v) && Array.isArray(args[k]) ? args[k].concat(v.filter(x => !args[k].includes(x))) : v;
        }
        if (!on && sent[src] === undefined)
            return; // (never turned on: nothing to turn off)
        // "inhibit:<what>": a logind inhibitor, held while wanted
        const cmd = src.startsWith("inhibit:") ? { cmd: "inhibit", what: src.slice(8), why: String(args.why ?? ""), on: on } : { cmd: "watch", src: src, on: on, args: args };
        const text = JSON.stringify(cmd);
        if (sent[src] === text)
            return;
        sent[src] = text;
        proc.write(text + "\n");
    }

    // ── The process ─────────────────────────────────────────────────────
    // Started again after it ends (KeptProcess: 10 s, 20 s…); after three
    // quick ends, or when it can't start at all, this shell gives up on it.
    property int quickEnds: 0
    property bool didStart: false
    KeptProcess {
        id: proc
        wanted: root.canStart && root.quickEnds < 3
        command: [root.found]
        stdinEnabled: true // (it ends when this closes: the shell quit)
        onStarted: root.didStart = true
        onRunningChanged: if (!running) {
            if (!root.didStart)
                root.quickEnds = 3; // (missing, or not runnable)
            else if (Date.now() - proc.startedAt < 60000)
                root.quickEnds++;
            else
                root.quickEnds = 0;
            root.didStart = false;
            root.ready = false;
            root.decided = true;
            root.pulseUp = false;
        }
        stdout: SplitParser {
            onRead: line => root.handle(line)
        }
    }
    // One that never says it's ready: watchers stand in meanwhile
    Timer {
        running: !root.decided
        interval: 3000
        onTriggered: root.decided = true
    }

    function handle(line) {
        let e;
        try {
            e = JSON.parse(line);
        } catch (x) {
            return;
        }
        if (e.src === "events") {
            if (e.kind === "ready") {
                unsupported = {};
                sent = {};
                ready = true;
                decided = true;
                const srcs = new Set(Object.values(wishes).map(w => w.src));
                for (const s of srcs)
                    sync(s);
            } else if (e.kind === "error") {
                console.warn("tessera-events:", e.msg);
            }
            return;
        }
        if (e.kind === "unsupported") {
            console.info("tessera-events: no " + e.src + " here (" + (e.error ?? "") + "): using its watcher");
            unsupported = Object.assign({}, unsupported, { [e.src]: true });
            return;
        }
        switch (e.src) {
        case "login1":
            login1(e.kind, e.args ?? []);
            break;
        case "unity":
            if ((e.args ?? []).length >= 2)
                launcherEntry(String(e.args[0]), e.args[1] ?? {});
            break;
        case "nm":
        case "net":
            networkChanged();
            break;
        case "avahi":
            if (e.kind === "new")
                avahiFound(e.key, e);
            else if (e.kind === "remove")
                avahiLost(e.key);
            else if (e.kind === "down")
                avahiDown();
            break;
        case "udev":
            udev(e.props ?? {});
            break;
        case "pulse":
            if (e.kind === "up")
                pulseUp = true;
            else if (e.kind === "down")
                pulseUp = false;
            else
                pulse("Event '" + e.kind + "' on " + e.facility + " #" + e.index);
            break;
        case "camera":
            camera(e.apps ?? []);
            break;
        case "location":
            location(e.apps ?? []);
            break;
        case "screenshare":
            screenshare(e.graph ?? []);
            break;
        case "copies":
            copies(e.items ?? []);
            break;
        case "clipboard":
            clipboardChanged();
            break;
        case "inhibit":
            if (e.error)
                console.warn("tessera-events: couldn't inhibit " + e.what + ": " + e.error);
            inhibited(e.what, e.on === true, e.error ?? "");
            break;
        }
    }
}
