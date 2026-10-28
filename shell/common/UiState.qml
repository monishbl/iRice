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
        property var workspaceHomes: ({}) // space id -> the display it belongs on (WorkspaceHomes)
        property var caffeine: null // on: { until (ms; 0 = until turned off), minutes }; survives shell restarts
        property string todayShown: "" // the day ("YYYY-MM-DD", days start at 4 AM) Today last opened by itself
        property string todayPinned: "" // the day Today was pinned to the desktop for

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
        onWorkspaceHomesChanged: root.scheduleSave()
    }

    property bool loaded: false
    // `lazy`: changes that may come often and matter little if lost (the
    // recent apps order on every focus change) are saved within 30 s
    function scheduleSave(lazy) {
        if (!loaded)
            return;
        if (lazy !== true)
            saveTimer.restart();
        else if (!saveTimer.running && !lazySave.running)
            lazySave.start();
    }
    Timer {
        id: lazySave
        interval: 30000
        onTriggered: saveTimer.restart()
    }
    Component.onDestruction: if (lazySave.running) saveTimer.triggered()

    Process {
        running: true
        command: ["mkdir", "-p", root.dir]
    }

    Timer {
        id: saveTimer
        interval: 300
        onTriggered: file.setText(JSON.stringify({
            overlay: store.overlay,
            crosshair: store.crosshair,
            floatingImage: store.floatingImage,
            todos: store.todos,
            alarms: store.alarms,
            clipPins: store.clipPins,
            recentApps: store.recentApps,
            desktopNote: store.desktopNote,
            desktopNotes: store.desktopNotes,
            screenTime: store.screenTime,
            stacks: store.stacks,
            shortcutButtons: store.shortcutButtons,
            appThemeOn: store.appThemeOn,
            coolDown: store.coolDown,
            caffeine: store.caffeine,
            todayShown: store.todayShown,
            todayPinned: store.todayPinned,
            workspaceHomes: store.workspaceHomes
        }, null, 2))
    }

    // Two shells can share this file (the dev sandbox and the tty test
    // session); re-read it when it changes so neither writes back stale state
    FileView {
        id: file
        path: root.dir + "/state.json"
        watchChanges: true
        // (not while our own change is waiting to be saved: it would be lost)
        onFileChanged: if (!saveTimer.running) reload()
        onLoaded: {
            root.loaded = false; // applying the file isn't a change to save
            try {
                const j = JSON.parse(text());
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
                store.workspaceHomes = j.workspaceHomes && typeof j.workspaceHomes === "object" ? j.workspaceHomes : {};
            } catch (e) {}
            root.loaded = true;
        }
        onLoadFailed: root.loaded = true
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
