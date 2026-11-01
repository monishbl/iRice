pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common
import "ShelfLogic.js" as Logic

// The drop shelf (modules/shelf/ShelfWindow): a place to park files while
// you get to where they're going. Drop files (or text) on it, then drag them
// out again, one at a time or all together (as often as needed). It holds
// references, not copies, and keeps them across restarts until they're
// removed or cleared.
// It opens:
//   - shaking the pointer while dragging something (shelf.shake)
//   - holding a drag at the left or right edge of a screen (shelf.edge)
// and only for a real drag of files: a selection, a scrollbar, a slider, or
// a browser's image or link dragged about never opens it
// (modules/shelf/ShelfProbes). Text and links can still go onto it once open.
//   - Super+Alt+D (keymap.json "shelf"), or its menu bar icon (shelf.menubar)
// While the left button is held (keybinds.lua: pointerDown / pointerUp,
// non-consuming), the pointer is followed (assets/scripts/pointer-follow.py);
// never otherwise. Moving a window (the irice-glass plugin says so) doesn't count.
Singleton {
    id: root

    readonly property var cfg: Config.options.shelf
    readonly property bool watching: cfg.shake || cfg.edge

    // ── What's on it ────────────────────────────────────────────────────
    // [{ uri, path, name, kind: "image" | "folder" | "file" | "text", text? }]
    property var items: []
    readonly property int count: items.length
    readonly property var paths: items.filter(i => i.path).map(i => i.path)

    // Dropped: file URLs, else text (ShelfLogic.drop)
    function drop(urls, text) {
        const r = Logic.drop(items, urls, text);
        if (r.added === 0)
            return false;
        items = r.items;
        save();
        return true;
    }
    function removeAt(i) {
        const list = items.slice();
        list.splice(i, 1);
        items = list;
        save();
        if (list.length === 0 && opened)
            closeSoon.restart();
    }
    property var cleared: []
    function clear() {
        if (items.length === 0)
            return;
        cleared = items;
        items = [];
        save();
        Island.post({
            kind: "custom", icon: "tray", title: "Shelf cleared",
            subtitle: cleared.length + (cleared.length === 1 ? " item" : " items") + " removed (the files themselves are untouched)",
            color: Theme.color.secondary, action: "Undo", onAction: () => root.undoClear()
        }, 8000);
    }
    function undoClear() {
        items = cleared.concat(items);
        cleared = [];
        save();
        show("menubar", null);
    }
    // MIME data for dragging items out
    function mimeFor(list) {
        return Logic.mimeFor(list);
    }

    // Kept across restarts (references only)
    readonly property string file: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/irice/shelf.json"
    function save() {
        saver.restart();
    }
    Timer {
        id: saver
        interval: 300
        onTriggered: store.setText(JSON.stringify({ items: root.items }, null, 1))
    }
    FileView {
        id: store
        path: root.file
        atomicWrites: true
        printErrors: false // (no file until something's been on the shelf)
        onLoaded: {
            try {
                const j = JSON.parse(text());
                if (Array.isArray(j.items))
                    root.items = j.items.filter(i => i && (i.uri || i.text));
            } catch (e) {}
        }
    }

    // ── Showing it ──────────────────────────────────────────────────────
    property bool opened: false
    property var screen: null
    property string side: "right" // left | right (which edge it sits at)
    property real atY: -1 // the pointer's height on that screen (-1: near the top)
    property bool fromDrag: false // opened mid-drag: it goes again if nothing lands
    function show(how, scr) {
        screen = scr ?? Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
        if (how !== "shake" && how !== "edge") {
            side = "right";
            atY = -1;
        }
        fromDrag = how === "shake" || how === "edge";
        closeSoon.stop();
        opened = true;
    }
    function hide() {
        opened = false;
        fromDrag = false;
    }
    function toggle(scr) {
        opened ? hide() : show("shortcut", scr);
    }
    property bool hovered: false // (ShelfWindow: the pointer, or a drag, is over it)
    Timer {
        id: closeSoon
        interval: 1200
        onTriggered: if (!root.hovered && root.items.length === 0) root.hide()
    }

    // ── Following a drag ────────────────────────────────────────────────
    property bool down: false
    property real downAt: 0
    property var start: null // {x, y}
    property var last: ({ x: 0, y: 0 })
    property bool dragging: false // moved far enough to be a drag
    property bool windowMove: false // (it's a window being moved)
    property var samples: [] // [{t, x}] the last moment's x positions
    // (ShelfProbes: drop areas while the button is held, and one under the
    // pointer after a shake, to tell a real drag of something)
    property bool probing: false
    property var probe: null // { x, y, screen } just after a shake
    Timer {
        id: probeEnd
        interval: 900
        onTriggered: root.probe = null
    }
    property string edgeAt: ""
    property var edgeScreen: null
    Timer {
        id: edgeHold
        interval: 350
        onTriggered: {
            if (root.opened || !root.down)
                return;
            root.side = root.edgeAt;
            root.atY = root.last.y - (root.edgeScreen?.y ?? 0);
            root.show("edge", root.edgeScreen);
        }
    }
    function dragArrived(where, scr, y) {
        if (opened)
            return;
        if (where === "shake") {
            side = probe && scr && probe.x > scr.x + scr.width / 2 ? "right" : "left";
            atY = probe && scr ? probe.y - scr.y : -1;
            show("shake", scr);
            probe = null;
        } else {
            edgeAt = where;
            edgeScreen = scr;
            edgeHold.restart();
        }
    }
    function dragLeft(where) {
        if (where !== "shake")
            edgeHold.stop();
    }

    function pointerDown() {
        if (!watching)
            return;
        down = true;
        downAt = Date.now();
        start = null;
        dragging = false;
        windowMove = false;
        samples = [];
        probe = null;
        lastMove = Date.now();
        follow.restart();
    }
    function pointerUp() {
        if (!down)
            return;
        down = false;
        probing = false;
        probe = null;
        edgeHold.stop();
        follow.stop();
        if (follower.running)
            follower.signal(15);
        if (opened && fromDrag && items.length === 0)
            closeSoon.restart();
        else if (opened && fromDrag)
            fromDrag = false; // (something landed: it stays)
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "irice-drag" && root.down) {
                root.windowMove = true;
                root.probing = false;
                if (follower.running)
                    follower.signal(15);
            }
        }
    }
    // Most clicks are over before this: only a held button is followed
    // (assets/scripts/pointer-follow.py, until the button comes up)
    Timer {
        id: follow
        interval: 120
        onTriggered: {
            if (root.down && !root.windowMove) {
                root.probing = true;
                if (!follower.running)
                    follower.running = true; // (one at a time: a second would be queued)
            }
        }
    }
    // A release can go missing (a surface appearing under the pointer): a
    // pointer that hasn't moved for 4 s, or a follower that ended, counts as
    // the button coming up
    property real lastMove: 0
    Timer {
        interval: 1000
        repeat: true
        running: follower.running
        onTriggered: if (root.down && Date.now() - root.lastMove > 4000) root.pointerUp()
    }
    Process {
        id: follower
        command: ["python3", "-I", Quickshell.shellDir + "/assets/scripts/pointer-follow.py"]
        onExited: if (root.down) root.pointerUp()
        stdout: SplitParser {
            onRead: line => {
                const m = line.match(/^\s*(-?\d+(?:\.\d+)?),\s*(-?\d+(?:\.\d+)?)\s*$/);
                if (m)
                    root.moved(+m[1], +m[2]);
            }
        }
    }

    function moved(x, y) {
        const now = Date.now();
        if (!start || x !== last.x || y !== last.y)
            lastMove = now;
        last = { x: x, y: y };
        if (!start)
            start = { x: x, y: y };
        if (!dragging && Math.hypot(x - start.x, y - start.y) > 16)
            dragging = true;
        if (!dragging || windowMove)
            return;
        const scr = Quickshell.screens.find(s => x >= s.x && x < s.x + s.width && y >= s.y && y < s.y + s.height) ?? null;

        // Shaken: the pointer turned back and forth quickly. Whether it's
        // carrying something is for the drop area put under it to tell
        // (ShelfProbes: dragArrived)
        samples = samples.filter(s => now - s.t < 700).concat([{ t: now, x: x }]);
        if (cfg.shake && !opened && Logic.shaken(samples, 40)) {
            probe = { x: x, y: y, screen: scr };
            probeEnd.restart();
            samples = [];
        }
        // (held at an edge: the edge strips tell, ShelfProbes)
    }
    // qs ipc call shelf toggle | open | close | clear
    IpcHandler {
        target: "shelf"
        function toggle(): void {
            root.toggle(null);
        }
        function open(): void {
            root.show("shortcut", null);
        }
        function close(): void {
            root.hide();
        }
        function clear(): void {
            root.clear();
        }
    }
}
