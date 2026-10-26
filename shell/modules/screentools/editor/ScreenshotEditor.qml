import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.services

// macOS-style screenshot markup. After a screenshot, a thumbnail floats in at
// the bottom right for 5 s (hover pauses it, drag it out to drop the file,
// swipe right to dismiss); clicking it opens the editor. Used when
// Config.options.screenshot.editor is "irice" (the default); "swappy" keeps
// the old flow. This object owns the editing session, so edits survive the
// editor window being unloaded (Delete has Undo).
//
// Nothing renders while idle: both windows sit behind LazyLoaders.
Scope {
    id: root

    readonly property bool enabled: (Config.options.screenshot.editor ?? "irice") !== "swappy"
    readonly property bool thumbnails: Config.options.screenshot.thumbnail ?? true

    // ---- Session ----------------------------------------------------------
    property string path: "" // the screenshot being edited
    property var screen: null
    property int serial: 0 // bumped when a file changes on disk (images reload)

    // Thumbnail: "" hidden | "shot" a screenshot | "deleted" the Undo toast
    property string thumb: ""
    property string thumbFile: "" // what the thumbnail shows (may differ from `path` while editing)
    property bool editorOpen: false

    // Annotations, in the image's native pixels. Arrays are replaced, never
    // mutated, so history can keep references. crop 0x0 = the whole image.
    property var items: []
    property rect crop: Qt.rect(0, 0, 0, 0)
    property var undoStack: []
    property var redoStack: []
    readonly property bool dirty: undoStack.length > 0

    // Tool settings, kept across screenshots for this run
    property string tool: "pen" // crop | pen | highlighter | arrow | rect | ellipse | text | pixelate
    property int colorIndex: 1
    property int widthIndex: 1 // 0 thin, 1 medium, 2 thick
    // Chart series from the design rules. One fixed set, not per theme: the
    // colours are painted into the image, which shouldn't depend on whether
    // the shell happened to be light or dark.
    readonly property var palette: ["#6D7CDF", "#D05F43", "#409D48", "#BB5FAA", "#BE7100", "#1D9999", "#9B6BCE", "#CE597A"]
    readonly property var paletteNames: ["Indigo", "Vermilion", "Green", "Orchid", "Amber", "Teal", "Violet", "Rose"]

    property string pendingDelete: "" // deleted when the Undo toast times out
    property string pendingOpen: "" // opens once the unsaved-changes prompt is answered

    function resetEdits() {
        items = [];
        crop = Qt.rect(0, 0, 0, 0);
        undoStack = [];
        redoStack = [];
    }

    // ---- History ----------------------------------------------------------
    function snapshot() {
        return { items: items, crop: crop };
    }
    function commit(nextItems, nextCrop) {
        undoStack = undoStack.concat([snapshot()]);
        redoStack = [];
        items = nextItems;
        if (nextCrop !== undefined)
            crop = nextCrop;
    }
    function add(item) {
        commit(items.concat([item]));
    }
    function setCrop(r) {
        commit(items, r);
    }
    function undo() {
        if (undoStack.length === 0)
            return;
        const s = undoStack[undoStack.length - 1];
        redoStack = redoStack.concat([snapshot()]);
        undoStack = undoStack.slice(0, -1);
        items = s.items;
        crop = s.crop;
    }
    function redo() {
        if (redoStack.length === 0)
            return;
        const s = redoStack[redoStack.length - 1];
        undoStack = undoStack.concat([snapshot()]);
        redoStack = redoStack.slice(0, -1);
        items = s.items;
        crop = s.crop;
    }

    // ---- Flow -------------------------------------------------------------
    // ScreenTools saved (and copied) a new screenshot
    function shot(file) {
        finishDelete();
        if (!thumbnails) {
            notify(file);
            return;
        }
        if (!editorOpen) {
            path = file;
            resetEdits();
        }
        screen = ScreenTools.focusedScreen();
        thumbFile = file;
        serial++;
        thumb = "shot";
    }

    function openEditor(file) {
        const f = file || thumbFile || path;
        if (!f)
            return;
        finishDelete();
        thumb = "";
        if (editorOpen && f !== path && dirty) {
            pendingOpen = f; // the editor asks about the current changes first
            return;
        }
        if (f !== path) {
            path = f;
            resetEdits();
        }
        screen = screen ?? ScreenTools.focusedScreen();
        editorOpen = true;
    }

    // Close, discarding changes (the editor asks first when there are any)
    function closeEditor() {
        editorOpen = false;
        resetEdits();
        openPending();
    }
    function openPending() {
        const next = pendingOpen;
        pendingOpen = "";
        if (!next)
            return false;
        path = next;
        resetEdits();
        Qt.callLater(() => editorOpen = true);
        return true;
    }

    // Saved: the result floats in as a thumbnail (drag it out, reopen it)
    function saved(file) {
        editorOpen = false;
        resetEdits();
        path = file;
        serial++;
        if (openPending())
            return;
        thumbFile = file;
        thumb = thumbnails ? "shot" : "";
    }

    // Delete acts at once; the file goes when the Undo toast times out
    function deleteShot() {
        finishDelete();
        pendingDelete = path;
        thumbFile = path;
        editorOpen = false;
        pendingOpen = "";
        thumb = "deleted";
    }
    function undoDelete() {
        pendingDelete = "";
        thumb = "";
        editorOpen = true; // the session, edits included, is still here
    }
    function finishDelete() {
        if (!pendingDelete)
            return;
        const f = pendingDelete;
        pendingDelete = "";
        // To the Trash where possible, so it can still be recovered
        Quickshell.execDetached(["sh", "-c", "if command -v gio >/dev/null 2>&1; then gio trash -- \"$1\" || rm -f -- \"$1\"; else rm -f -- \"$1\"; fi", "sh", f]);
        if (path === f) {
            path = "";
            resetEdits();
        }
        if (thumb === "deleted")
            thumb = "";
    }

    // The thumbnail finished leaving (timed out, swiped, dropped somewhere)
    function thumbGone() {
        if (thumb === "deleted")
            finishDelete();
        thumb = "";
    }

    // Thumbnails turned off: the notification the shell used before, plus Edit
    // (the action comes back on stdout and is handled here)
    function notify(file) {
        const p = notifyJob.createObject(root, { file: file });
        p.running = true;
    }
    Component {
        id: notifyJob
        Process {
            id: n
            property string file
            command: ["notify-send", "-a", "Screenshot", "-i", file, "-A", "edit=Edit", "-A", "folder=Show in Folder", "Screenshot saved", "Copied to the clipboard"]
            stdout: StdioCollector {
                onStreamFinished: {
                    const a = text.trim();
                    if (a === "edit") {
                        root.screen = ScreenTools.focusedScreen();
                        root.openEditor(n.file);
                    } else if (a === "folder") {
                        Files.reveal(n.file);
                    }
                }
            }
            onExited: Qt.callLater(() => n.destroy())
        }
    }

    Component.onCompleted: ScreenTools.hasEditor = Qt.binding(() => root.enabled)
    Component.onDestruction: {
        ScreenTools.hasEditor = false;
        finishDelete();
    }
    Connections {
        target: ScreenTools
        function onShot(file) { root.shot(file); }
    }

    // qs ipc call shotEditor edit /path/to/image.png
    IpcHandler {
        target: "shotEditor"
        function edit(file: string): void {
            root.screen = ScreenTools.focusedScreen();
            root.openEditor(file);
        }
        function thumbnail(file: string): void {
            root.shot(file);
        }
        function close(): void {
            root.thumbGone();
            root.closeEditor();
        }
    }

    LazyLoader {
        active: root.thumb !== ""
        ShotThumbnail { session: root }
    }
    LazyLoader {
        active: root.editorOpen
        ShotEditorWindow { session: root }
    }
}
