import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services
import qs.modules.spotlight
import Quickshell.Widgets

// Overview (tap Super). Two styles (Config overview.style):
//   grid   the spaces as miniatures of the screen (below)
//   macos  like macOS: a strip of spaces on a glass bar at the top (the
//          windows of each space, on the wallpaper if overview.wallpapers;
//          drag one to reorder, + for a new space) and only a search in the
//          middle
// Grid: spaces as miniatures of the screen,
// each window a preview of its content at its real place and size (floating
// windows above tiled ones, the most recently focused on top; fullscreen
// fills the miniature), plus a search field. Click a space to go there, a
// window to focus it, drag a window onto another space to move it. Typing
// searches apps and actions like Spotlight; Enter opens the first result.
// Arrows pick a space.
// With several displays (Config overview.displays):
//   rows   the screen it opens on lists every display's spaces, a labelled
//          row per display in arrangement order, each display's current
//          space marked; windows drag between rows
//   each   it opens on every screen, each listing only its own spaces
// Every display numbers its own spaces from 1 (Spaces); spaces of a display
// that's unplugged follow the display they're staying on, marked with theirs.
// Previews (Config overview.previews): "snapshot" captures each window once
// per open, "live" keeps them updating, "hybrid" is live for the current
// space only. Nothing is captured while the Overview is closed.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "overview"
        screen: modelData
        everyScreen: Config.options.overview?.displays === "each"

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "overview"

            readonly property string screenName: screen?.name ?? ""
            // Displays with spaces (connected, not the sandbox's own window)
            readonly property var displays: Hyprland.monitors.values.filter(m => Spaces.bases[m.name] !== undefined).sort((a, b) => a.x - b.x || a.y - b.y).map(m => m.name)
            readonly property bool multi: displays.length > 1
            readonly property bool each: multi && Config.options.overview?.displays === "each"
            readonly property bool rowsMode: multi && !each
            // Each screen shows its own: open on all of them, the keyboard on
            // the one it was opened on
            readonly property bool shown: each ? Panels.open === "overview" : Panels.isOpen("overview", screen)
            readonly property bool keyScreen: !each || Panels.screen === screen
            readonly property bool mac: Config.options.overview?.style === "macos"
            readonly property int activeWs: Spaces.active[screenName] ?? 1
            // The rows: every display (rows), or just this one
            readonly property var rowNames: rowsMode ? displays : [screenName]
            readonly property var rowIds: {
                const m = {};
                for (const n of rowNames)
                    m[n] = Spaces.idsFor(n, false);
                return m;
            }
            readonly property var ids: {
                const out = [];
                for (const n of rowNames)
                    for (const id of rowIds[n] ?? [])
                        out.push(id);
                return out;
            }
            // Where each space lives (its windows are laid out there)
            readonly property var hostOf: {
                const m = {};
                for (const w of Spaces.workspaces)
                    m[w.id] = w.monitor;
                return m;
            }
            // A display's place and logical size: { x, y, w, h }
            function geoOf(name) {
                const m = Hyprland.monitors.values.find(x => x.name === name);
                if (!m)
                    return { x: 0, y: 0, w: screen?.width ?? 1920, h: screen?.height ?? 1080 };
                const s = m.scale || 1;
                const w = m.width / s, h = m.height / s;
                return (m.lastIpcObject?.transform ?? 0) % 2 ? { x: m.x, y: m.y, w: h, h: w } : { x: m.x, y: m.y, w: w, h: h };
            }
            // A window's place within a space on display `geo`: { x, y, w, h }
            // from that display's corner. A window sent to a space another
            // display isn't showing keeps its old place until that space is
            // shown (Hyprland), so it's mapped across from where it is.
            function placeIn(o, geo) {
                const at = o.at ?? [0, 0], sz = o.size ?? [0, 0];
                const cx = at[0] + sz[0] / 2, cy = at[1] + sz[1] / 2;
                const inside = g => cx >= g.x && cx < g.x + g.w && cy >= g.y && cy < g.y + g.h;
                let src = geo;
                if (!inside(geo))
                    for (const m of Hyprland.monitors.values) {
                        const g = geoOf(m.name);
                        if (inside(g)) {
                            src = g;
                            break;
                        }
                    }
                const sx = geo.w / Math.max(1, src.w), sy = geo.h / Math.max(1, src.h);
                return { x: (at[0] - src.x) * sx, y: (at[1] - src.y) * sy, w: sz[0] * sx, h: sz[1] * sy };
            }
            function aspectOf(name) {
                const g = geoOf(name);
                return g.h / Math.max(1, g.w);
            }
            // Grid: spaces per line, and one cell width for every row that
            // fits them all on screen (each row keeps its display's shape)
            readonly property int columns: Math.max(1, Math.min(rowsMode ? 6 : 5, Math.max(...rowNames.map(n => (rowIds[n] ?? []).length), 1)))
            // Each its own: every other display is a drop zone on the edge
            // that faces it (drop a window there, or drag a space there)
            // (a snapshot replaced only when what it says changes: the zones'
            // Repeater keeps its delegates through every refresh of Hyprland's
            // lists, instead of making them again each time)
            readonly property var others: {
                const out = [];
                for (const n of displays)
                    if (n !== screenName) {
                        const g = geoOf(n);
                        out.push({ name: n, x: g.x, y: g.y, w: g.w, h: g.h });
                    }
                return out;
            }
            readonly property string zonesKey: each ? JSON.stringify(Spaces.edgeZones(geoOf(screenName), others)) : "[]"
            readonly property var zones: JSON.parse(zonesKey)
            // A drag pushed against an edge that faces another display (the
            // pointer crossing over to it): that display, as if dropped on its
            // zone. The pointer stays with the drag past the edge; the dragged
            // item stops at it.
            function edgeAt(gx, gy) {
                return multi ? Spaces.edgeTarget(geoOf(screenName), others, gx, gy) : "";
            }
            property string edgeZone: "" // the display a dragged window is pushed toward
            // (room for the zones on the left and right edges)
            readonly property real sideReserve: zones.some(z => z.side === "left" || z.side === "right") ? 160 : 0
            property var zoneItems: []
            function zoneAt(gx, gy) { // the display whose zone is under a window point
                for (const z of zoneItems)
                    if (z.hit(gx, gy))
                        return z.target;
                return "";
            }
            property string spaceZone: "" // the zone (or edge) a dragged space is over
            property bool macDrag: false // a space is being dragged in the macOS strip
            readonly property bool anyDrag: dragging || dragSpace > 0 || macDrag
            property int spaceGone: 0 // a space sent to another display, hidden until it's gone
            Timer {
                running: win.spaceGone > 0
                interval: 1500
                onTriggered: win.spaceGone = 0
            }
            readonly property real cellW: {
                const byWidth = (width - 160 - sideReserve - (columns - 1) * 24) / columns;
                if (!rowsMode)
                    return Math.min(300, byWidth);
                // rows below the search: label 28, title 26 + gap 24 a line, 32 between rows
                const avail = height - 64 - 52 - 40 - 48;
                let fixed = 0, perW = 0;
                for (const n of rowNames) {
                    const lines = Math.ceil(Math.max(1, (rowIds[n] ?? []).length) / columns);
                    fixed += 28 + lines * 50 + 32;
                    perW += lines * aspectOf(n);
                }
                return Math.max(80, Math.min(300, byWidth, (avail - fixed) / Math.max(0.1, perW)));
            }
            property int picked: activeWs
            property bool dragging: false // a window is being dragged to another space
            property int dragFrom: 0 // the space it's dragged from (drawn above the others)
            // Dragging a space (grid: by its empty area or title) to reorder
            // it within its display's row
            property int dragSpace: 0
            property point dragD: Qt.point(0, 0)
            property int dropOn: 0
            property var cells: [] // grid cells, for finding the one under a point
            function spaceAt(row, gx, gy) {
                let best = 0, dist = 1e9;
                for (const c of cells) {
                    if (c.row !== row || c.wsId === dragSpace)
                        continue;
                    const p = c.mapToItem(null, c.width / 2, c.height / 2);
                    const d = Math.hypot(p.x - gx, p.y - gy);
                    if (d < dist) {
                        dist = d;
                        best = c.wsId;
                    }
                }
                return best;
            }
            property var results: []
            property int selected: 0

            // Window previews: snapshot | live | hybrid (live on the current space)
            readonly property string previewMode: {
                const m = Config.options.overview?.previews;
                return m === "live" || m === "hybrid" ? m : "snapshot";
            }
            function liveFor(wsId) {
                return shown && (previewMode === "live" || (previewMode === "hybrid" && Object.keys(Spaces.active).some(n => Spaces.active[n] === wsId)));
            }
            // True from open until the close animation ends: previews hold
            // their source (and last frame) while it fades out, and drop it after
            property bool capturing: false
            signal recapture() // reopened mid-close: snapshots take a new picture
            onVisibleChanged: if (!visible) capturing = false

            property real progress: 0
            Component.onCompleted: if (shown) open()
            onShownChanged: {
                if (shown) {
                    open();
                } else {
                    // Stop the opening spring first: closed while it was still
                    // running (a quick double tap of Super), the two fought,
                    // and the spring left it drawn on screen but taking no input
                    openAnim.stop();
                    closeAnim.start();
                    menu = null;
                }
            }
            function open() {
                Hyprland.refreshToplevels();
                Hyprland.refreshMonitors(); // (places, for the edge zones and layouts)
                if (capturing)
                    recapture();
                capturing = true;
                picked = activeWs;
                field.text = "";
                results = [];
                if (keyScreen)
                    field.forceActiveFocus();
                closeAnim.stop();
                openAnim.start();
            }
            SpringAnimation { id: openAnim; target: win; property: "progress"; to: 1; spring: Theme.motion.spring + 0.6; damping: Theme.motion.damping + 0.08; epsilon: 0.004 }
            NumberAnimation { id: closeAnim; target: win; property: "progress"; to: 0; duration: Theme.motion.popover * 0.7; easing.type: Easing.InCubic }

            visible: progress > 0 && closeGuard.open
            CloseGuard { id: closeGuard; shown: win.shown }
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown && keyScreen ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            // Move the picked space by `n` places in the grid
            function step(n) {
                const i = Math.max(0, ids.indexOf(picked));
                picked = ids[Math.max(0, Math.min(ids.length - 1, i + n))] ?? picked;
            }
            // Search answers that arrive later (files, conversions)
            Connections {
                target: Search
                function onRevisionChanged() {
                    if (!win.shown || field.text === "")
                        return;
                    // Keep the row you moved to (rows shift as answers arrive)
                    const keep = win.selected > 0 ? win.results[win.selected]?.title : undefined;
                    win.results = Search.query(field.text).filter(r => !r.ai);
                    const i = win.results.findIndex(r => r.title === keep);
                    win.selected = i >= 0 ? i : 0;
                }
            }
            function go(id) {
                Panels.close();
                Spaces.go(id);
            }
            function focusWindow(t) {
                Panels.close();
                const id = t.workspace?.id ?? 0;
                if (id > 0 && Spaces.active[hostOf[id] ?? ""] !== id)
                    Hyprland.dispatch("hl.dsp.focus({ workspace = " + id + " })");
                Qt.callLater(() => t.wayland?.activate());
            }
            function moveWindow(t, id) {
                Spaces.sendWindow(t.address, id);
                afterMove();
            }
            // To another display's current space
            function moveWindowToDisplay(t, name) {
                Spaces.sendWindowToDisplay(t.address, name);
                afterMove();
            }
            // A whole space to another display (its next number there)
            function moveSpace(id, name) {
                if (!Spaces.exists(id))
                    return;
                spaceGone = id;
                Spaces.moveSpaceTo(id, name);
                afterMove();
            }
            // Windows' places come with a refresh: one, once Hyprland says the
            // move happened (its events for one move come together), so every
            // screen's Overview shows it where it landed. (It used to be three
            // on a timer, each re-placing the miniatures.)
            property bool awaitingMove: false
            function afterMove() {
                awaitingMove = true;
                moveFallback.restart();
            }
            Connections {
                target: Hyprland
                enabled: win.awaitingMove
                function onRawEvent(event) {
                    if (["movewindowv2", "moveworkspacev2", "renameworkspace"].includes(event.name))
                        moveSettle.restart();
                }
            }
            Timer {
                id: moveSettle
                interval: 50
                onTriggered: {
                    win.awaitingMove = false;
                    moveFallback.stop();
                    Hyprland.refreshToplevels();
                    Hyprland.refreshWorkspaces();
                }
            }
            Timer { // no event came (the move didn't happen): look anyway
                id: moveFallback
                interval: 500
                onTriggered: moveSettle.restart()
            }

            // Right-click menus: a window or a space to another display
            property var menu: null // { items, x, y }
            function openMenu(items, item, mx, my) {
                const p = item.mapToItem(null, mx, my);
                menu = { items: items, x: p.x, y: p.y };
            }
            function windowMenu(t) {
                const here = hostOf[t.workspace?.id ?? 0] ?? screenName;
                const items = [];
                for (const n of displays) {
                    if (n === here)
                        continue;
                    if (items.length > 0)
                        items.push({ separator: true });
                    const label = Spaces.labelForName(n);
                    items.push({ text: "Move to " + label, action: () => win.moveWindowToDisplay(t, n) });
                    // and its other spaces
                    for (const id of Spaces.idsFor(n, false).filter(i => Spaces.isOwn(i, n) && i !== Spaces.active[n]).slice(0, 8))
                        items.push({ text: "Move to " + Spaces.title(id, n) + " on " + label, action: () => win.moveWindow(t, id) });
                }
                if (items.length === 0)
                    items.push({ text: "Only one display is connected", enabled: false });
                return items;
            }
            function spaceMenu(id) {
                const here = hostOf[id] ?? (Spaces.ownerName(id) || screenName);
                const items = displays.filter(n => n !== here).map(n => ({
                    text: "Move space to " + Spaces.labelForName(n),
                    enabled: Spaces.exists(id),
                    action: () => win.moveSpace(id, n)
                }));
                if (items.length === 0)
                    items.push({ text: "Only one display is connected", enabled: false });
                return items;
            }
            Loader {
                active: win.menu !== null && win.shown
                sourceComponent: ContextMenu {
                    screen: win.screen
                    items: win.menu?.items ?? []
                    readonly property bool up: (win.menu?.y ?? 0) > win.height * 0.6
                    pointX: (win.menu?.x ?? 0) + menuWidth / 2 // opens at the pointer, like macOS
                    pointY: win.menu?.y ?? 0
                    upward: up
                    onDismissed: {
                        win.menu = null;
                        if (win.shown && win.keyScreen)
                            field.forceActiveFocus();
                    }
                }
            }
            // "Space 3", with the display it belongs to when it's another's
            function titleOf(id, row) {
                return Spaces.title(id, row);
            }

            // Dim the screen behind
            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0, 0, 0, (win.mac ? 0.18 : 0.35) * Math.min(1, win.progress))
                MouseArea {
                    anchors.fill: parent
                    onClicked: Panels.close()
                }
            }

            Item {
                id: content
                anchors.fill: parent
                opacity: Math.min(1, win.progress)
                scale: 0.97 + 0.03 * win.progress

                // Search
                Item {
                    id: searchBox
                    width: 560 + 2 * fieldGlass.shadowMargin
                    height: 52 + 2 * fieldGlass.shadowMargin
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: win.mac ? Math.round(parent.height * 0.36) : 64
                    Glass {
                        id: fieldGlass
                        anchors.fill: parent
                        radius: 26
                        shadowMargin: 20
                    }
                    Icon {
                        x: fieldGlass.shadowMargin + 18
                        anchors.verticalCenter: parent.verticalCenter
                        name: "magnifying-glass"
                        bold: true
                        size: 20
                        color: Theme.color.secondary
                    }
                    TextInput {
                        id: field
                        x: fieldGlass.shadowMargin + 50
                        width: 560 - 70
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.color.label
                        selectionColor: Theme.accentAlpha(0.5)
                        clip: true
                        font { family: Theme.font.family; pixelSize: 20 }
                        onTextChanged: {
                            win.results = Search.query(text).filter(r => !r.ai);
                            win.selected = 0;
                        }
                        Keys.onEscapePressed: text !== "" ? text = "" : Panels.close()
                        Keys.onReturnPressed: win.results.length > 0 ? (Panels.close(), win.results[win.selected].run()) : win.go(win.picked)
                        Keys.onEnterPressed: win.results.length > 0 ? (Panels.close(), win.results[win.selected].run()) : win.go(win.picked)
                        Keys.onLeftPressed: e => { if (text === "") win.step(-1); else e.accepted = false; }
                        Keys.onRightPressed: e => { if (text === "") win.step(1); else e.accepted = false; }
                        Keys.onUpPressed: text !== "" ? win.selected = Math.max(0, win.selected - 1) : win.mac ? undefined : win.step(-win.columns)
                        Keys.onDownPressed: text !== "" ? win.selected = Math.min(win.results.length - 1, win.selected + 1) : win.mac ? undefined : win.step(win.columns)
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: field.text === ""
                            text: win.mac ? "Search" : "Search apps, or pick a space"
                            color: Theme.color.tertiary
                            font: field.font
                        }
                    }
                }

                // Results while typing
                Item {
                    visible: win.results.length > 0
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: searchBox.y + searchBox.height - 30
                    width: 560 + 40
                    height: Math.min(resultList.contentHeight, 420) + 16 + 40
                    Glass {
                        anchors.fill: parent
                        radius: 22
                        shadowMargin: 20
                    }
                    MouseArea { anchors.fill: parent; anchors.margins: 20 }
                    ListView {
                        id: resultList
                        x: 28
                        y: 28
                        width: 560 - 16
                        height: parent.height - 56
                        clip: true
                        model: win.results
                        currentIndex: win.selected
                        boundsBehavior: Flickable.StopAtBounds
                        delegate: ResultRow {
                            required property var modelData
                            required property int index
                            width: resultList.width
                            result: modelData
                            selected: index === win.selected
                            onActivated: {
                                Panels.close();
                                modelData.run();
                            }
                        }
                    }
                }

                // macOS style: the spaces in a strip on a glass bar at the top
                // (the compositor blurs what's behind it), a strip per display
                // with several. Drag a space sideways to put it elsewhere
                // (Spaces.move: it takes the place of the one it's dropped on);
                // click to go there; + adds a new empty space after the last
                Item {
                    id: macBar
                    visible: win.mac
                    width: parent.width
                    height: macRows.y + macRows.height + 20
                    y: -(1 - Math.min(1, win.progress)) * 40
                    Rectangle {
                        anchors.fill: parent
                        color: Theme.dark ? Qt.rgba(0.12, 0.12, 0.14, 0.6) : Qt.rgba(1, 1, 1, 0.5)
                        Rectangle {
                            anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                            height: 1
                            color: Theme.color.separator
                        }
                    }
                    MouseArea { anchors.fill: parent } // (a click on the bar doesn't close)

                    Column {
                        id: macRows
                        y: win.rowsMode ? 40 : 20 // (row labels below the menu bar)
                        width: parent.width
                        spacing: 16

                        Repeater {
                            model: ScriptModel { values: win.mac ? win.rowNames : [] }
                            Item {
                                id: macRow
                                required property string modelData
                                readonly property string row: modelData
                                readonly property var rowIdList: win.rowIds[row] ?? []
                                readonly property real aspect: win.aspectOf(row)
                                // Short strips with several displays, so the bar stays a bar
                                readonly property real stripH: win.rowsMode ? Math.max(48, Math.min(96, (win.height * 0.42) / win.rowNames.length - 40)) : 120
                                width: macRows.width
                                height: (win.rowsMode ? 20 : 0) + stripH + 26

                                Text {
                                    visible: win.rowsMode
                                    x: 28
                                    text: Spaces.labelForName(macRow.row)
                                    color: Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                                }

                                Row {
                                    id: strip
                                    readonly property int n: Math.max(1, macRow.rowIdList.length)
                                    readonly property real tileW: Math.min(macRow.stripH / macRow.aspect, 170, (macRow.width - 220 - (n - 1) * 24) / n)
                                    property int dragSpace: 0
                                    property real dragDx: 0
                                    property real dragDy: 0 // (only with drop zones: a space can leave the bar)
                                    property int dropOn: 0
                                    onDragSpaceChanged: win.macDrag = dragSpace > 0
                                    function spaceAt(px) {
                                        let best = 0, dist = 1e9;
                                        for (const c of strip.children) {
                                            if (c.wsId === undefined)
                                                continue;
                                            const d = Math.abs(c.x + c.width / 2 - px);
                                            if (d < dist) {
                                                dist = d;
                                                best = c.wsId;
                                            }
                                        }
                                        return best;
                                    }
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: win.rowsMode ? 20 : 6
                                    spacing: 24

                                    Repeater {
                                        model: ScriptModel { values: macRow.rowIdList }
                                        Column {
                                            id: tile
                                            required property int modelData
                                            readonly property int wsId: modelData
                                            readonly property var geo: win.geoOf(win.hostOf[wsId] ?? macRow.row)
                                            readonly property real s: strip.tileW / Math.max(1, geo.w)
                                            readonly property bool current: Spaces.active[win.hostOf[wsId] ?? ""] === wsId
                                            readonly property bool dragged: strip.dragSpace === wsId
                                            readonly property bool target: strip.dragSpace > 0 && strip.dropOn === wsId && !dragged
                                            spacing: 6
                                            z: dragged ? 2 : 0
                                            opacity: dragged ? 0.85 : win.spaceGone === wsId ? 0 : 1
                                            transform: Translate { x: tile.dragged ? strip.dragDx : 0; y: tile.dragged ? strip.dragDy : 0 }

                                            Item {
                                                width: strip.tileW
                                                height: Math.round(strip.tileW * macRow.aspect)
                                                scale: thumbMouse.pressed && !tile.dragged ? 0.97 : thumbMouse.containsMouse ? 1.03 : 1
                                                Behavior on scale { NumberAnimation { duration: Theme.motion.hover } }
                                                ClippingRectangle {
                                                    anchors.fill: parent
                                                    radius: 10
                                                    color: Theme.dark ? "#1A1A1D" : "#F4F4F5"
                                                    border.width: 1
                                                    border.color: Theme.dark ? "#36363C" : "#D4D4D8"
                                                    Image {
                                                        anchors.fill: parent
                                                        visible: Config.options.overview.wallpapers
                                                        source: visible ? Wallpaper.source : ""
                                                        fillMode: Image.PreserveAspectCrop
                                                        sourceSize: Qt.size(340, 220)
                                                        asynchronous: true
                                                    }
                                                    // Its windows, at their places
                                                    Repeater {
                                                        model: ScriptModel { values: Windows.on(tile.wsId).filter(t => t.lastIpcObject?.at && t.lastIpcObject?.size && !t.lastIpcObject.hidden) }
                                                        Rectangle {
                                                            required property var modelData
                                                            readonly property var ipc: modelData.lastIpcObject
                                                            readonly property var r: win.placeIn(ipc, tile.geo)
                                                            x: r.x * tile.s
                                                            y: r.y * tile.s
                                                            width: Math.max(4, r.w * tile.s)
                                                            height: Math.max(4, r.h * tile.s)
                                                            radius: 3
                                                            color: Theme.dark ? "#2C2C31" : "#FFFFFF"
                                                            border.width: 1
                                                            border.color: Theme.dark ? "#36363C" : "#E4E4E7"
                                                            IconImage {
                                                                anchors.centerIn: parent
                                                                implicitSize: Math.min(18, parent.height * 0.6)
                                                                source: Apps.iconFor(parent.ipc.class)
                                                            }
                                                        }
                                                    }
                                                }
                                                // The current space, or where a dragged one would land
                                                Squircle {
                                                    anchors { fill: parent; margins: -3 }
                                                    radius: 13
                                                    color: "transparent"
                                                    border.width: 3
                                                    border.color: Theme.accent
                                                    visible: tile.current || tile.target
                                                }
                                                MouseArea {
                                                    id: thumbMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                                    cursorShape: strip.dragSpace > 0 ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                                                    property real pressX: 0
                                                    property real pressY: 0
                                                    onPressed: m => {
                                                        const p = thumbMouse.mapToItem(null, m.x, m.y); // (in the window: the tile moves with the drag)
                                                        pressX = p.x;
                                                        pressY = p.y;
                                                        if (m.button === Qt.RightButton)
                                                            win.openMenu(win.spaceMenu(tile.wsId), thumbMouse, m.x, m.y);
                                                    }
                                                    onPositionChanged: m => {
                                                        if (!(pressedButtons & Qt.LeftButton))
                                                            return;
                                                        const p = thumbMouse.mapToItem(null, m.x, m.y);
                                                        // (the tile stops at the screen's edge; the pointer can go on)
                                                        const dx = Math.max(0, Math.min(win.width - 1, p.x)) - pressX;
                                                        const dy = win.zones.length > 0 ? Math.max(0, Math.min(win.height - 1, p.y)) - pressY : 0;
                                                        if (strip.dragSpace === 0 && Math.hypot(dx, dy) > 8)
                                                            strip.dragSpace = tile.wsId;
                                                        if (strip.dragSpace === tile.wsId) {
                                                            strip.dragDx = dx;
                                                            strip.dragDy = dy;
                                                            win.spaceZone = win.zoneAt(p.x, p.y) || win.edgeAt(p.x, p.y);
                                                            strip.dropOn = win.spaceZone !== "" ? 0 : strip.spaceAt(tile.x + tile.width / 2 + dx);
                                                        }
                                                    }
                                                    onReleased: m => {
                                                        if (m.button !== Qt.LeftButton)
                                                            return;
                                                        if (strip.dragSpace === tile.wsId) {
                                                            if (win.spaceZone !== "")
                                                                win.moveSpace(tile.wsId, win.spaceZone);
                                                            else if (strip.dropOn > 0 && strip.dropOn !== tile.wsId)
                                                                Spaces.move(macRow.rowIdList, tile.wsId, strip.dropOn);
                                                            strip.dragSpace = 0;
                                                            strip.dragDx = 0;
                                                            strip.dragDy = 0;
                                                            strip.dropOn = 0;
                                                            win.spaceZone = "";
                                                        } else {
                                                            win.go(tile.wsId);
                                                        }
                                                    }
                                                }
                                            }
                                            Text {
                                                anchors.horizontalCenter: parent.horizontalCenter
                                                text: Spaces.title(tile.wsId, macRow.row, "Desktop")
                                                color: Theme.color.label
                                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: tile.current ? Font.DemiBold : Font.Normal }
                                            }
                                        }
                                    }
                                }

                                // + a new empty space on this display, after the last one in use
                                Rectangle {
                                    anchors { right: parent.right; rightMargin: 28 }
                                    y: strip.y + Math.round(strip.tileW * macRow.aspect) / 2 - height / 2
                                    width: 36
                                    height: 36
                                    radius: 18
                                    color: plusMouse.pressed ? Theme.color.fillHover : plusMouse.containsMouse ? Theme.color.fill : Qt.rgba(0, 0, 0, 0)
                                    border.width: 1
                                    border.color: Theme.color.separator
                                    Icon {
                                        anchors.centerIn: parent
                                        name: "plus"
                                        bold: true
                                        size: 18
                                        color: Theme.color.label
                                    }
                                    MouseArea {
                                        id: plusMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            Panels.close();
                                            Spaces.newSpace(macRow.row);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Spaces (grid): a row per display with several (labelled), or
                // this display's
                Column {
                    id: rows
                    visible: !win.mac && win.results.length === 0
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: Math.max(searchBox.y + searchBox.height + 40, (parent.height - height) / 2)
                    spacing: 32

                    Repeater {
                        model: ScriptModel { values: win.mac ? [] : win.rowNames }
                        Column {
                            id: gridRow
                            required property string modelData
                            readonly property string row: modelData
                            readonly property var rowIdList: win.rowIds[row] ?? []
                            readonly property real cellH: win.cellW * win.aspectOf(row)
                            anchors.horizontalCenter: parent?.horizontalCenter
                            spacing: 8
                            // a window dragged from here passes over the other rows
                            z: win.dragFrom > 0 && rowIdList.includes(win.dragFrom) || rowIdList.includes(win.dragSpace) ? 1 : 0

                            Text {
                                visible: win.rowsMode
                                text: Spaces.labelForName(gridRow.row)
                                color: Qt.rgba(1, 1, 1, 0.85)
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                            }

                            Grid {
                                id: spaces
                                columns: win.columns
                                spacing: 24

                                Repeater {
                                    // ScriptModel keeps delegates (and their captures) when the list is rebuilt
                                    model: ScriptModel { values: gridRow.rowIdList }
                                    Column {
                                        id: space
                                        required property int modelData
                                        readonly property int wsId: modelData
                                        readonly property string row: gridRow.row
                                        // where it lives (its windows are laid out there)
                                        readonly property var geo: win.geoOf(win.hostOf[wsId] ?? gridRow.row)
                                        readonly property real k: win.cellW / Math.max(1, geo.w)
                                        readonly property bool current: Spaces.active[win.hostOf[wsId] ?? ""] === wsId
                                        // Grouped windows behind their tab are hidden
                                        readonly property var wins: Windows.on(space.wsId).filter(t => t.lastIpcObject?.at && t.lastIpcObject?.size && !t.lastIpcObject.hidden)
                                        readonly property bool live: win.liveFor(space.wsId)
                                        readonly property bool dragged: win.dragSpace === space.wsId
                                        Component.onCompleted: win.cells = win.cells.concat([space])
                                        Component.onDestruction: win.cells = win.cells.filter(c => c !== space)
                                        // Dragging the space (its empty area, or its title), within its row.
                                        // Measured in the window (the handle moves with the space, so
                                        // its own coordinates would follow it and lag the pointer)
                                        function dragMove(item, mx, my, from) {
                                            const p = item.mapToItem(null, mx, my);
                                            // (the space stops at the screen's edge; the pointer can go on)
                                            const dx = Math.max(0, Math.min(win.width - 1, p.x)) - from.x;
                                            const dy = Math.max(0, Math.min(win.height - 1, p.y)) - from.y;
                                            if (win.dragSpace === 0 && Math.hypot(dx, dy) > 10)
                                                win.dragSpace = space.wsId;
                                            if (win.dragSpace === space.wsId) {
                                                win.dragD = Qt.point(dx, dy);
                                                // over another display's zone, or pushed against the
                                                // edge that faces it: it goes there instead
                                                win.spaceZone = win.zoneAt(p.x, p.y) || win.edgeAt(p.x, p.y);
                                                win.dropOn = win.spaceZone !== "" ? 0 : win.spaceAt(space.row, p.x, p.y);
                                            }
                                        }
                                        function dragEnd() { // true if it was a drag, not a click
                                            if (win.dragSpace !== space.wsId)
                                                return false;
                                            if (win.spaceZone !== "")
                                                win.moveSpace(space.wsId, win.spaceZone);
                                            else if (win.dropOn > 0 && win.dropOn !== space.wsId)
                                                Spaces.move(gridRow.rowIdList, space.wsId, win.dropOn);
                                            win.spaceZone = "";
                                            win.dragSpace = 0;
                                            win.dragD = Qt.point(0, 0);
                                            win.dropOn = 0;
                                            return true;
                                        }
                                        z: win.dragFrom === space.wsId || dragged ? 1 : 0 // a dragged window (or space) passes over the others
                                        spacing: 8
                                        opacity: dragged ? 0.85 : win.spaceGone === wsId ? 0 : 1
                                        transform: Translate { x: space.dragged ? win.dragD.x : 0; y: space.dragged ? win.dragD.y : 0 }

                                        // The space: a plain neutral container
                                        Rectangle {
                                            id: screenRect
                                            width: win.cellW
                                            height: gridRow.cellH
                                            radius: 14
                                            clip: !win.dragging // let a dragged tile leave its space
                                            z: win.dragging ? 1 : 0
                                            color: Theme.dark ? "#1A1A1D" : "#F4F4F5"
                                            readonly property bool target: win.dragSpace > 0 && win.dropOn === space.wsId && !space.dragged
                                            // a drag pushed toward this display's edge lands on its current space
                                            readonly property bool edgeHot: space.current && (win.edgeZone || win.spaceZone) !== "" && (win.edgeZone || win.spaceZone) === win.hostOf[space.wsId]
                                            // picked (pointer, arrows): accent; each display's current space: a softer accent
                                            border.width: space.wsId === win.picked || dropArea.containsDrag || target || edgeHot || (space.current && win.rowsMode) ? 2 : 1
                                            border.color: dropArea.containsDrag || target || edgeHot || space.wsId === win.picked ? Theme.accent : space.current ? (win.rowsMode ? Theme.accentAlpha(0.55) : (Theme.dark ? "#36363C" : "#D4D4D8")) : (Theme.dark ? "#26262A" : "#E4E4E7")
                                            scale: spaceMouse.pressed ? 0.97 : spaceMouse.containsMouse ? 1.02 : 1
                                            Behavior on scale { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.04; epsilon: 0.002 } }

                                            // The wallpaper behind the windows (overview.wallpapers)
                                            ClippingRectangle {
                                                anchors { fill: parent; margins: 1 }
                                                visible: Config.options.overview.wallpapers
                                                radius: 13
                                                color: "transparent"
                                                Image {
                                                    anchors.fill: parent
                                                    source: parent.visible ? Wallpaper.source : ""
                                                    fillMode: Image.PreserveAspectCrop
                                                    sourceSize: Qt.size(400, 250)
                                                    asynchronous: true
                                                }
                                            }

                                            Text {
                                                anchors.centerIn: parent
                                                visible: space.wins.length === 0
                                                text: "No windows"
                                                color: Theme.dark ? "#94949D" : "#6B6B74"
                                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                                            }
                                            MouseArea {
                                                id: spaceMouse
                                                anchors.fill: parent
                                                hoverEnabled: true
                                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                                cursorShape: win.dragSpace > 0 ? Qt.ClosedHandCursor : Qt.ArrowCursor
                                                property point pressAt: Qt.point(0, 0)
                                                onEntered: win.picked = space.wsId
                                                onPressed: m => {
                                                    pressAt = spaceMouse.mapToItem(null, m.x, m.y);
                                                    if (m.button === Qt.RightButton)
                                                        win.openMenu(win.spaceMenu(space.wsId), spaceMouse, m.x, m.y);
                                                }
                                                onPositionChanged: m => {
                                                    if (pressedButtons & Qt.LeftButton)
                                                        space.dragMove(spaceMouse, m.x, m.y, pressAt);
                                                }
                                                onReleased: m => {
                                                    if (m.button === Qt.LeftButton && !space.dragEnd())
                                                        win.go(space.wsId);
                                                }
                                            }
                                            DropArea {
                                                id: dropArea
                                                anchors.fill: parent
                                                onDropped: drop => {
                                                    const t = drop.source?.toplevel;
                                                    if (t && (t.workspace?.id ?? 0) !== space.wsId) {
                                                        drop.accept(Qt.MoveAction);
                                                        win.moveWindow(t, space.wsId);
                                                    } else {
                                                        drop.accepted = false; // its own space: it springs back
                                                    }
                                                }
                                            }

                                            // A preview per window, at its real place and size
                                            Repeater {
                                                model: ScriptModel { values: space.wins }
                                                Item {
                                                    id: thumb
                                                    required property var modelData
                                                    readonly property var toplevel: modelData
                                                    readonly property var o: modelData.lastIpcObject ?? ({})
                                                    // fullscreen: 0 none, 1 maximized, 2 fullscreen (bit flags; a bool on old Hyprland)
                                                    readonly property int fsMode: o.fullscreen === true ? 2 : (o.fullscreen | 0)
                                                    readonly property bool full: (fsMode & 2) !== 0
                                                    // Stacking like Hyprland: tiled, then floating by focus (recent on
                                                    // top), then fullscreen and maximized
                                                    readonly property int stack: fsMode > 0 ? 3000 : o.floating ? 2000 - Math.max(0, Math.min(999, o.focusHistoryID ?? 999)) : 1
                                                    // Fullscreen fills the miniature, inside its 1px border
                                                    readonly property var r: win.placeIn(o, space.geo)
                                                    readonly property real homeX: full ? 1 : r.x * space.k
                                                    readonly property real homeY: full ? 1 : r.y * space.k
                                                    property bool gone: false // dropped on another space, until the move lands
                                                    x: homeX
                                                    y: homeY
                                                    width: full ? screenRect.width - 2 : Math.max(16, r.w * space.k)
                                                    height: full ? screenRect.height - 2 : Math.max(12, r.h * space.k)
                                                    opacity: gone ? 0 : 1
                                                    Drag.active: thumbDrag.active
                                                    Drag.source: thumb
                                                    // the pointer is what's dropped (a big miniature over an
                                                    // edge zone would otherwise miss it)
                                                    Drag.hotSpot.x: thumbDrag.active ? thumbDrag.centroid.pressPosition.x : width / 2
                                                    Drag.hotSpot.y: thumbDrag.active ? thumbDrag.centroid.pressPosition.y : height / 2
                                                    z: thumbDrag.active ? 5000 : stack
                                                    // A new place (a refresh after a move, the space's other
                                                    // windows re-tiling) glides there instead of jumping
                                                    Behavior on x { enabled: !thumbDrag.active && win.shown; NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                                                    Behavior on y { enabled: !thumbDrag.active && win.shown; NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                                                    Behavior on width { enabled: win.shown; NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                                                    Behavior on height { enabled: win.shown; NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                                                    scale: thumbDrag.active ? 1.04 : 1
                                                    Behavior on scale { ScaleAnimator { duration: 150; easing.type: Easing.OutCubic } }

                                                    WindowPreview {
                                                        id: preview
                                                        anchors.fill: parent
                                                        toplevel: thumb.modelData
                                                        radius: thumb.full ? 13 : 4 // the space's 14 minus its border
                                                        capture: win.capturing
                                                        live: space.live
                                                        hovered: tileMouse.containsMouse || thumbDrag.active
                                                    }
                                                    Connections {
                                                        target: win
                                                        function onRecapture() {
                                                            preview.recapture();
                                                        }
                                                    }
                                                    Timer {
                                                        running: thumb.gone
                                                        interval: 1500 // the move didn't happen: show it again
                                                        onTriggered: thumb.gone = false
                                                    }
                                                    MouseArea {
                                                        id: tileMouse
                                                        anchors.fill: parent
                                                        hoverEnabled: true
                                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                                        onEntered: win.picked = space.wsId
                                                        onClicked: m => {
                                                            if (m.button === Qt.RightButton)
                                                                win.openMenu(win.windowMenu(thumb.modelData), tileMouse, m.x, m.y);
                                                            else
                                                                win.focusWindow(thumb.modelData);
                                                        }
                                                    }
                                                    DragHandler {
                                                        id: thumbDrag
                                                        // Pushed against an edge that faces another display
                                                        onCentroidChanged: {
                                                            if (active) {
                                                                const p = centroid.scenePosition;
                                                                win.edgeZone = win.edgeAt(p.x, p.y);
                                                            }
                                                        }
                                                        onActiveChanged: {
                                                            win.dragFrom = active ? space.wsId : 0;
                                                            win.dragging = active;
                                                            if (active) {
                                                                // The miniature stays whole on screen, stopping at
                                                                // its edge (the pointer holds the drag beyond it,
                                                                // onto the other display)
                                                                const o = screenRect.mapToItem(null, 0, 0);
                                                                xAxis.minimum = -o.x;
                                                                xAxis.maximum = win.width - o.x - thumb.width;
                                                                yAxis.minimum = -o.y;
                                                                yAxis.maximum = win.height - o.y - thumb.height;
                                                            } else {
                                                                const to = win.edgeZone;
                                                                win.edgeZone = "";
                                                                const over = thumb.Drag.target;
                                                                const already = to !== "" && Spaces.active[to] === (thumb.modelData.workspace?.id ?? 0);
                                                                if (to !== "" && !already && !(over && over.objectName === "chip")) {
                                                                    // to that display's current space
                                                                    thumb.Drag.cancel();
                                                                    win.moveWindowToDisplay(thumb.modelData, to);
                                                                    thumb.gone = true;
                                                                } else {
                                                                    thumb.gone = thumb.Drag.drop() === Qt.MoveAction;
                                                                }
                                                                // Back home (the drag broke the bindings); if it moved,
                                                                // it stays hidden until it shows up on the new space
                                                                thumb.x = Qt.binding(() => thumb.homeX);
                                                                thumb.y = Qt.binding(() => thumb.homeY);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Text {
                                            anchors.horizontalCenter: parent.horizontalCenter
                                            text: Spaces.title(space.wsId, gridRow.row) + (space.wins.length > 0 ? " · " + space.wins.length + (space.wins.length === 1 ? " window" : " windows") : "")
                                            color: Qt.rgba(1, 1, 1, space.wsId === win.picked || space.current ? 0.95 : 0.7)
                                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: space.current ? Font.DemiBold : Font.Medium }
                                            // The title is a handle too (a space full of windows has no empty area)
                                            MouseArea {
                                                id: titleMouse
                                                anchors { fill: parent; margins: -6 }
                                                hoverEnabled: true
                                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                                cursorShape: win.dragSpace > 0 ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                                                property point pressAt: Qt.point(0, 0)
                                                onEntered: win.picked = space.wsId
                                                onPressed: m => {
                                                    pressAt = titleMouse.mapToItem(null, m.x, m.y);
                                                    if (m.button === Qt.RightButton)
                                                        win.openMenu(win.spaceMenu(space.wsId), titleMouse, m.x, m.y);
                                                }
                                                onPositionChanged: m => {
                                                    if (pressedButtons & Qt.LeftButton)
                                                        space.dragMove(titleMouse, m.x, m.y, pressAt);
                                                }
                                                onReleased: m => {
                                                    if (m.button === Qt.LeftButton && !space.dragEnd())
                                                        win.go(space.wsId);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Each its own: the other displays as drop zones, each on the
                // edge that faces it. Drop a window on one: it goes to that
                // display's current space (hold it there a moment and that
                // display's spaces open, to pick one). Drag a space onto one:
                // it moves there, windows and all, as that display's next
                // space. A click goes to that display.
                Item {
                    id: zoneLayer
                    anchors.fill: parent
                    visible: win.zones.length > 0
                    readonly property real topY: win.mac ? macBar.height + 16 : 12

                    Repeater {
                        model: ScriptModel { values: win.zones }
                        Item {
                            id: zone
                            required property var modelData
                            readonly property string target: modelData.name
                            readonly property string side: modelData.side
                            readonly property bool vertical: side === "left" || side === "right"
                            readonly property real m: zoneGlass.shadowMargin
                            property int hotChip: 0 // the space chip a window is over
                            property bool expanded: false
                            readonly property bool over: zoneDrop.containsDrag || hotChip > 0
                            readonly property bool hot: over || win.spaceZone === target || win.edgeZone === target
                            readonly property var spaceIds: Spaces.idsFor(target, false).filter(id => Spaces.isOwn(id, target)).slice(0, 9)
                            readonly property real headH: vertical ? 68 : 44
                            readonly property real pillW: Math.max(vertical ? 96 : head.implicitWidth + 32, expanded ? chips.implicitWidth + 16 : 0)
                            readonly property real pillH: headH + (expanded ? chips.implicitHeight + 8 : 0)
                            // several on one edge: side by side along it
                            readonly property real along: (modelData.index - (modelData.count - 1) / 2) * (vertical ? 68 + 12 : 200)
                            width: pillW + 2 * m
                            height: pillH + 2 * m
                            x: side === "left" ? 16 - m : side === "right" ? zoneLayer.width - 16 - pillW - m : Math.round(zoneLayer.width / 2 - width / 2 + along)
                            y: side === "top" ? zoneLayer.topY - m : side === "bottom" ? zoneLayer.height - 16 - pillH - m : Math.round(zoneLayer.height / 2 - height / 2 + along)
                            function hit(gx, gy) {
                                const p = mapFromItem(null, gx, gy);
                                return p.x >= m && p.y >= m && p.x < m + pillW && p.y < m + pillH;
                            }
                            Component.onCompleted: win.zoneItems = win.zoneItems.concat([zone])
                            Component.onDestruction: win.zoneItems = win.zoneItems.filter(z => z !== zone)

                            // Held over it ~600 ms: its spaces open; closed again once the window leaves
                            Timer {
                                running: zoneDrop.containsDrag && !zone.expanded && zone.spaceIds.length > 1
                                interval: 600
                                onTriggered: zone.expanded = true
                            }
                            Timer {
                                running: zone.expanded && !zone.over
                                interval: 350
                                onTriggered: zone.expanded = false
                            }

                            Glass {
                                id: zoneGlass
                                anchors.fill: parent
                                radius: zone.expanded || zone.vertical ? 18 : 22
                                shadowMargin: 16
                            }
                            // Highlight while something is over it
                            Squircle {
                                x: zone.m
                                y: zone.m
                                width: zone.pillW
                                height: zone.pillH
                                radius: zoneGlass.radius
                                color: zone.hot ? Theme.accentAlpha(0.22) : zoneMouse.containsMouse ? Theme.color.fill : "transparent"
                                border.width: zone.hot ? 2 : 0
                                border.color: Theme.accent
                            }
                            MouseArea {
                                id: zoneMouse
                                x: zone.m
                                y: zone.m
                                width: zone.pillW
                                height: zone.pillH
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: win.go(Spaces.active[zone.target] ?? Spaces.idFor(zone.target, 1))
                            }
                            DropArea {
                                id: zoneDrop
                                x: zone.m
                                y: zone.m
                                width: zone.pillW
                                height: zone.pillH
                                onDropped: drop => {
                                    const t = drop.source?.toplevel;
                                    if (t && zone.hotChip === 0) {
                                        drop.accept(Qt.MoveAction);
                                        win.moveWindowToDisplay(t, zone.target);
                                    } else {
                                        drop.accepted = false;
                                    }
                                    zone.expanded = false;
                                }
                            }
                            Grid {
                                id: head
                                columns: zone.vertical ? 1 : 2
                                spacing: zone.vertical ? 4 : 8
                                horizontalItemAlignment: Grid.AlignHCenter
                                verticalItemAlignment: Grid.AlignVCenter
                                x: zone.m + Math.round((zone.pillW - width) / 2)
                                y: zone.m + Math.round((zone.headH - height) / 2)
                                Icon {
                                    name: "monitor"
                                    size: 20
                                    color: zone.hot ? Theme.accentInk : Theme.color.secondary
                                }
                                Text {
                                    width: zone.vertical ? Math.min(implicitWidth, 84) : implicitWidth
                                    horizontalAlignment: Text.AlignHCenter
                                    elide: Text.ElideRight
                                    text: Spaces.labelForName(zone.target)
                                    color: zone.hot || win.anyDrag ? Theme.color.label : Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: zone.vertical ? Theme.font.small : Theme.font.body; weight: Font.DemiBold }
                                }
                            }
                            // Its spaces, to drop a window on one
                            Grid {
                                id: chips
                                visible: zone.expanded
                                columns: zone.vertical ? 2 : 9
                                spacing: 8
                                x: zone.m + Math.round((zone.pillW - width) / 2)
                                y: zone.m + zone.headH
                                Repeater {
                                    model: ScriptModel { values: zone.expanded ? zone.spaceIds : [] }
                                    Item {
                                        id: chip
                                        required property int modelData
                                        readonly property bool current: Spaces.active[zone.target] === modelData
                                        width: 36
                                        height: 36
                                        Squircle {
                                            anchors.fill: parent
                                            radius: 10
                                            color: chipDrop.containsDrag ? Theme.accent : Theme.color.fill
                                            border.width: chip.current && !chipDrop.containsDrag ? 2 : 0
                                            border.color: Theme.accentAlpha(0.55)
                                        }
                                        Text {
                                            anchors.centerIn: parent
                                            text: Spaces.numberOf(chip.modelData)
                                            color: chipDrop.containsDrag ? "white" : Theme.color.label
                                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: chip.current ? Font.DemiBold : Font.Medium; features: { "tnum": 1 } }
                                        }
                                        DropArea {
                                            id: chipDrop
                                            objectName: "chip" // (a window dropped here goes to this space even when pushed at the edge)
                                            anchors.fill: parent
                                            onContainsDragChanged: {
                                                if (containsDrag)
                                                    zone.hotChip = chip.modelData;
                                                else if (zone.hotChip === chip.modelData)
                                                    zone.hotChip = 0;
                                            }
                                            onDropped: drop => {
                                                const t = drop.source?.toplevel;
                                                if (t) {
                                                    drop.accept(Qt.MoveAction);
                                                    win.moveWindow(t, chip.modelData);
                                                } else {
                                                    drop.accepted = false;
                                                }
                                                zone.hotChip = 0;
                                                zone.expanded = false;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
