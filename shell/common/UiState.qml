pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Remembered UI state (not settings): overlay widget layout, pins, crosshair
// style, floating image, to-dos. Lives in $XDG_STATE_HOME/irice/state.json.
// Read and written as plain JSON (no JsonAdapter: Quickshell 0.2 can crash
// deserialising var properties). Assign whole values to trigger a save,
// e.g. UiState.data.todos = [...].
Singleton {
    id: root

    readonly property string dir: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/irice"
    readonly property QtObject data: store

    QtObject {
        id: store

        // id -> { open, pinned, clickthrough, x, y, w, h }
        property var overlay: ({
                notes: { open: false, pinned: false, clickthrough: false, x: 80, y: 120, w: 340, h: 300 },
                resources: { open: true, pinned: false, clickthrough: false, x: 440, y: 120, w: 320, h: 260 },
                recorder: { open: false, pinned: false, clickthrough: false, x: 780, y: 120, w: 300, h: 170 },
                mixer: { open: false, pinned: false, clickthrough: false, x: 1100, y: 120, w: 340, h: 300 },
                image: { open: false, pinned: false, clickthrough: false, x: 80, y: 460, w: 360, h: 280 },
                crosshair: { open: false, pinned: false, clickthrough: false, x: 460, y: 420, w: 300, h: 230 },
                fps: { open: false, pinned: false, clickthrough: false, x: 780, y: 330, w: 300, h: 170 }
            })
        property var crosshair: ({ enabled: false, color: "#30D158", size: 12, thickness: 2, gap: 4, dot: true })
        property string floatingImage: ""
        property var todos: [] // [{ text, done, due? (ms) }]
        property var alarms: [] // [{ id, at (ms), label, kind: "timer" | "alarm" }]
        property var clipPins: [] // [{ text } | { image, source }]
        property var recentApps: [] // desktop entry ids, most recent first
        property string desktopNote: "" // the first Note widget (no id)
        property var desktopNotes: ({}) // each other Note widget: id -> text
        property var screenTime: ({}) // "YYYY-MM-DD" -> { app id: seconds } (last 7 days)
        property var stacks: ({}) // Smart Stack widget id -> [widget kinds]
        property var shortcutButtons: ({}) // Shortcuts widget id -> [action titles]
        property bool appThemeOn: false // Match apps wrote colour files (so turning it off empties them)
        property var coolDown: null // what cool-down changed, to put back: { blur: { passes, size }, rates: { output: Hz } }
        property var spaces: ({}) // spaces per display (Spaces): { displays: { identity: { base, label, name } }, shown: { identity: id }, migrated }
        property var caffeine: null // on: { until (ms; 0 = until turned off), minutes }; survives shell restarts
        property string todayShown: "" // the day ("YYYY-MM-DD", days start at 4 AM) Today last opened by itself
        property string todayPinned: "" // the day Today was pinned to the desktop for
        property string todayEnsured: "" // the day the daily Today widget was last put back (Widget / Both)
        property var conflictsMuted: [] // Conflicts: ids of things also running that you said not to mention again

        onOverlayChanged: root.scheduleSave()
        onCrosshairChanged: root.scheduleSave()
        onFloatingImageChanged: root.scheduleSave()
        onTodosChanged: root.scheduleSave()
        onAlarmsChanged: root.scheduleSave()
        onClipPinsChanged: root.scheduleSave()
        onRecentAppsChanged: root.scheduleSave(true)
        onDesktopNoteChanged: root.scheduleSave()
        onDesktopNotesChanged: root.scheduleSave()
        onScreenTimeChanged: root.scheduleSave(true)
        onStacksChanged: root.scheduleSave()
        onShortcutButtonsChanged: root.scheduleSave()
        onAppThemeOnChanged: root.scheduleSave()
        onCoolDownChanged: root.scheduleSave()
        onCaffeineChanged: root.scheduleSave()
        onTodayShownChanged: root.scheduleSave()
        onTodayPinnedChanged: root.scheduleSave()
        onTodayEnsuredChanged: root.scheduleSave()
        onSpacesChanged: root.scheduleSave()
        onConflictsMutedChanged: root.scheduleSave()
    }

    property bool loaded: false
    property bool initialized: false
    readonly property var keys: ["overlay", "crosshair", "floatingImage", "todos", "alarms", "clipPins", "recentApps", "desktopNote", "desktopNotes", "screenTime", "stacks", "shortcutButtons", "appThemeOn", "coolDown", "caffeine", "todayShown", "todayPinned", "todayEnsured", "spaces", "conflictsMuted"]
    readonly property string journalId: Date.now().toString(36) + "-" + Math.random().toString(36).slice(2)
    readonly property string merger: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/state-merge.py").toString().replace("file://", ""))
    property int sequence: 0
    property int mergingSequence: 0
    property var observed: ({})
    property var dirty: ({})
    function snapshot() {
        const result = {};
        for (const key of keys)
            result[key] = JSON.parse(JSON.stringify(store[key]));
        return result;
    }
    function scheduleSave(lazy) {
        if (!loaded || !initialized)
            return;
        const current = snapshot(), changes = {}, next = Object.assign({}, dirty);
        const seq = ++sequence;
        for (const key of keys) {
            if (JSON.stringify(current[key]) !== JSON.stringify(observed[key])) {
                changes[key] = { revision: [Date.now(), journalId, seq], value: current[key] };
                next[key] = { sequence: seq, value: current[key] };
            }
        }
        if (!Object.keys(changes).length)
            return;
        // Write synchronously to a private immutable journal before debouncing
        // the shared state merge. A quit/crash cannot lose a pending debounce.
        journal.path = dir + "/pending/" + journalId + "-" + seq + ".json";
        journal.setText(JSON.stringify(changes));
        journal.waitForJob();
        observed = current;
        dirty = next;
        // The shared file: soon after a real change, but only every 30 s for
        // the frequent bookkeeping ones (recent apps on every focus change,
        // screen time every minute), each merge being a python3 run. They're
        // safe meanwhile: the journal above is already on disk.
        if (lazy) {
            if (!saveTimer.running && !lazyTimer.running)
                lazyTimer.start();
        } else {
            lazyTimer.stop();
            saveTimer.restart();
        }
    }
    // (only written: preload off, or each new path is first read and, not
    // existing yet, logged as a failed read)
    FileView { id: journal; preload: false; printErrors: true; blockWrites: true; atomicWrites: true }
    Timer { id: saveTimer; interval: 100; onTriggered: if (!writer.running) root.flush() }
    Timer { id: lazyTimer; interval: 30000; onTriggered: if (!writer.running) root.flush() }
    function flush() {
        if (writer.running)
            return;
        mergingSequence = sequence;
        writer.running = true;
    }
    Component.onDestruction: {
        if (initialized && Object.keys(dirty).length)
            Quickshell.execDetached(["python3", "-I", merger, dir]);
    }
    Process {
        id: writer
        command: ["python3", "-I", root.merger, root.dir]
        running: true
        onExited: code => {
            if (code !== 0) {
                // The journals stay for the next merge. The first time, carry
                // on with the file as it is, or nothing would ever load or
                // save (a missing python3, say)
                console.warn("UI state merge failed; recovery journals retained");
                if (!root.initialized) {
                    root.initialized = true;
                    file.reload();
                }
                return;
            }
            root.initialized = true;
            const next = {};
            for (const key of Object.keys(root.dirty))
                if (root.dirty[key].sequence > root.mergingSequence)
                    next[key] = root.dirty[key];
            root.dirty = next;
            file.reload();
            if (Object.keys(next).length)
                saveTimer.restart();
        }
    }

    // Two shells can share this file (the dev sandbox and the tty test
    // session); re-read it when it changes so neither writes back stale state
    FileView {
        id: file
        path: root.initialized ? root.dir + "/state.json" : ""
        watchChanges: true
        // (not while our own change is waiting to be saved: it would be lost)
        onFileChanged: reload()
        onLoaded: {
            root.loaded = false; // applying the file isn't a change to save
            try {
                const j = JSON.parse(text());
                for (const key of Object.keys(root.dirty))
                    j[key] = root.dirty[key].value;
                if (j.overlay)
                    store.overlay = Object.assign({}, store.overlay, j.overlay);
                if (j.crosshair)
                    store.crosshair = Object.assign({}, store.crosshair, j.crosshair);
                if (typeof j.floatingImage === "string")
                    store.floatingImage = j.floatingImage;
                if (Array.isArray(j.todos))
                    store.todos = j.todos;
                if (Array.isArray(j.alarms))
                    store.alarms = j.alarms;
                if (Array.isArray(j.clipPins))
                    store.clipPins = j.clipPins;
                if (Array.isArray(j.recentApps))
                    store.recentApps = j.recentApps;
                if (typeof j.desktopNote === "string")
                    store.desktopNote = j.desktopNote;
                if (j.desktopNotes && typeof j.desktopNotes === "object")
                    store.desktopNotes = j.desktopNotes;
                if (typeof j.appThemeOn === "boolean")
                    store.appThemeOn = j.appThemeOn;
                for (const k of ["screenTime", "stacks", "shortcutButtons"])
                    if (j[k] && typeof j[k] === "object")
                        store[k] = j[k];
                store.coolDown = j.coolDown && typeof j.coolDown === "object" ? j.coolDown : null;
                store.caffeine = j.caffeine && typeof j.caffeine === "object" ? j.caffeine : null;
                if (typeof j.todayShown === "string")
                    store.todayShown = j.todayShown;
                if (typeof j.todayPinned === "string")
                    store.todayPinned = j.todayPinned;
                if (typeof j.todayEnsured === "string")
                    store.todayEnsured = j.todayEnsured;
                store.spaces = j.spaces && typeof j.spaces === "object" ? j.spaces : {};
            } catch (e) {}
            root.observed = root.snapshot();
            root.loaded = true;
        }
        onLoadFailed: { root.observed = root.snapshot(); root.loaded = root.initialized; }
    }

    // Overlay widget state helpers
    function widget(id) {
        return store.overlay[id] ?? { open: false, pinned: false, clickthrough: false, x: 100, y: 100, w: 320, h: 240 };
    }
    function setWidget(id, patch) {
        const o = Object.assign({}, store.overlay);
        o[id] = Object.assign({}, widget(id), patch);
        store.overlay = o;
    }
}
