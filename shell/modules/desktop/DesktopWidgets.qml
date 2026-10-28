import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// Desktop widgets on a grid, drawn inside the wallpaper window. Drag a widget
// to move it (it snaps to the nearest free cell); right-click it for sizes and
// Remove. Right-click the desktop, or Settings → Desktop, for Edit Widgets:
// the gallery to add more, and remove badges.
Item {
    id: root

    anchors.fill: parent
    required property var screen

    WidgetDefs { id: widgetDefs }
    readonly property var defs: widgetDefs
    readonly property point origin: Qt.point(32, 54) // below the menu bar
    readonly property int cols: Math.max(1, Math.floor((width - 2 * origin.x + defs.gap) / (defs.cell + defs.gap)))
    readonly property int rows: Math.max(1, Math.floor((height - origin.y - 110 + defs.gap) / (defs.cell + defs.gap))) // leave the dock clear
    // Saved entries this shell doesn't know (a newer build added the kind)
    // are kept when saving, not dropped
    readonly property var unknownEntries: Config.options.desktop.widgets.filter(e => !defs.kind(defs.parse(e).kind))
    // As saved; shown positions come from `placed` (moved inside this screen's
    // grid if the saved cell doesn't fit, without rewriting the layout)
    readonly property var widgets: Config.options.desktop.widgets.map(e => defs.parse(e)).filter(w => defs.kind(w.kind))
    readonly property var placed: {
        const out = [];
        for (const w of widgets) {
            if (fits(out, w.col, w.row, w.size, -1)) {
                out.push(w);
                continue;
            }
            const spot = nearestFree(out, w.col, w.row, w.size, -1);
            out.push(spot ? Object.assign({}, w, spot) : w);
        }
        return out;
    }
    readonly property bool editing: Panels.widgetEditing && Panels.screen === screen
    // A note or the large reminders widget takes typing
    readonly property bool wantsKeys: widgets.some(w => w.kind === "notes" || ((w.kind === "reminders" || w.kind === "calendar") && w.size === "l")) // (text fields)

    function save(list) {
        Config.options.desktop.widgets = list.map(w => defs.format(w)).concat(unknownEntries);
    }

    // ── Grid placement ──────────────────────────────────────────────────
    function fits(list, c, r, size, skip) {
        const u = defs.units[size];
        if (c < 0 || r < 0 || c + u[0] > cols || r + u[1] > rows)
            return false;
        for (let i = 0; i < list.length; i++) {
            if (i === skip)
                continue;
            const o = list[i], ou = defs.units[o.size];
            if (c < o.col + ou[0] && o.col < c + u[0] && r < o.row + ou[1] && o.row < r + u[1])
                return false;
        }
        return true;
    }
    // The free cell closest to (c, r), or null if the grid is full
    function nearestFree(list, c, r, size, skip) {
        let best = null, bestD = 1e9;
        for (let cc = 0; cc < cols; cc++)
            for (let rr = 0; rr < rows; rr++)
                if (fits(list, cc, rr, size, skip)) {
                    const d = (cc - c) * (cc - c) + (rr - r) * (rr - r) * 1.1;
                    if (d < bestD) {
                        bestD = d;
                        best = { col: cc, row: rr };
                    }
                }
        return best;
    }

    function add(kind, size) {
        const list = widgets.slice();
        const spot = nearestFree(list, 0, 0, size, -1);
        if (!spot) {
            Island.post({ kind: "custom", icon: "squares-four", title: "No room on the desktop", subtitle: "Remove or shrink a widget first", color: Theme.accents.orange });
            return;
        }
        list.push({ kind: kind, size: size, col: spot.col, row: spot.row, id: defs.kind(kind)?.ownId ? Math.random().toString(36).slice(2, 8) : "" });
        save(list);
    }
    function remove(i) {
        const list = widgets.slice();
        list.splice(i, 1);
        save(list);
    }
    function resize(i, size) {
        const list = widgets.slice();
        const w = list[i];
        const spot = fits(list, w.col, w.row, size, i) ? { col: w.col, row: w.row } : nearestFree(list, w.col, w.row, size, i);
        if (!spot)
            return;
        list[i] = Object.assign({}, w, { size: size }, spot);
        save(list);
    }
    function moveTo(i, x, y) {
        const list = widgets.slice();
        const c = Math.round((x - origin.x) / (defs.cell + defs.gap));
        const r = Math.round((y - origin.y) / (defs.cell + defs.gap));
        const spot = nearestFree(list, c, r, list[i].size, i);
        if (!spot) {
            save(list); // snap back
            return;
        }
        list[i] = Object.assign({}, list[i], spot);
        save(list);
    }

    Connections {
        target: Panels
        function onAddWidget(kind, size) {
            if (Panels.screen === root.screen)
                root.add(kind, size);
        }
    }

    // ── Grid and snap preview ───────────────────────────────────────────
    // While editing or dragging: faint cells, and where the widget will land
    property var ghost: null // { col, row, size }
    function preview(i, x, y, active) {
        if (!active) {
            ghost = null;
            return;
        }
        const c = Math.round((x - origin.x) / (defs.cell + defs.gap));
        const r = Math.round((y - origin.y) / (defs.cell + defs.gap));
        const spot = nearestFree(widgets, c, r, widgets[i].size, i);
        ghost = spot ? { col: spot.col, row: spot.row, size: widgets[i].size } : null;
    }
    Item {
        id: gridLayer
        anchors.fill: parent
        z: -1
        opacity: root.editing || root.ghost ? 1 : 0
        visible: opacity > 0.01
        Behavior on opacity { NumberAnimation { duration: Theme.motion.popover } }
        Repeater {
            model: gridLayer.visible ? root.cols * root.rows : 0
            Rectangle {
                required property int index
                x: root.origin.x + (index % root.cols) * (defs.cell + defs.gap)
                y: root.origin.y + Math.floor(index / root.cols) * (defs.cell + defs.gap)
                width: defs.cell
                height: defs.cell
                radius: Theme.radius.tile
                color: Qt.rgba(1, 1, 1, 0.03)
                border.width: 1
                border.color: Qt.rgba(1, 1, 1, 0.12)
            }
        }
        // Where the dragged widget will snap
        Rectangle {
            visible: !!root.ghost
            readonly property size px: root.ghost ? defs.pixelSize(root.ghost.size) : Qt.size(0, 0)
            x: root.origin.x + (root.ghost?.col ?? 0) * (defs.cell + defs.gap)
            y: root.origin.y + (root.ghost?.row ?? 0) * (defs.cell + defs.gap)
            width: px.width
            height: px.height
            radius: Theme.radius.tile
            color: Qt.rgba(1, 1, 1, 0.1)
            border.width: 2
            border.color: Qt.rgba(1, 1, 1, 0.55)
            Behavior on x { SpringAnimation { spring: 5; damping: 0.4; epsilon: 0.3 } }
            Behavior on y { SpringAnimation { spring: 5; damping: 0.4; epsilon: 0.3 } }
        }
    }

    // ── Desktop right-click ─────────────────────────────────────────────
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: point => root.openMenu(root.widgetAt(point.position.x, point.position.y), point.position.x, point.position.y)
    }
    TapHandler {
        // A click on the empty desktop ends editing, like macOS
        enabled: root.editing
        onTapped: point => { if (root.widgetAt(point.position.x, point.position.y) < 0) Panels.widgetEditing = false; }
    }

    // Index of the widget under a point, or -1 for the bare desktop
    function widgetAt(x, y) {
        for (let i = 0; i < placed.length; i++) {
            const w = placed[i], px = defs.pixelSize(w.size);
            const wx = origin.x + w.col * (defs.cell + defs.gap), wy = origin.y + w.row * (defs.cell + defs.gap);
            if (x >= wx && x < wx + px.width && y >= wy && y < wy + px.height)
                return i;
        }
        return -1;
    }

    property var menu: null // { items, x, y }
    function openMenu(i, x, y) {
        const items = [];
        if (i >= 0) {
            const w = widgets[i];
            for (const s of defs.kind(w.kind).sizes)
                items.push({ text: (s === w.size ? "✓ " : "    ") + defs.sizeNames[s], action: () => root.resize(i, s), enabled: s !== w.size });
            // The widget's own items (Edit Stack, Change Buttons...)
            const own = frames.itemAt(i)?.extraMenu() ?? [];
            items.push(...own);
            items.push({ separator: true });
            items.push({ text: "Edit Widgets…", action: () => root.startEditing() });
            items.push({ text: "Remove Widget", danger: true, action: () => root.remove(i) });
        } else {
            items.push({ text: "Edit Widgets…", action: () => root.startEditing() });
            items.push({ text: "Change Wallpaper…", action: () => Panels.show("wallpaper", root.screen) });
            items.push({ separator: true });
            items.push({ text: (Config.options.desktop.useStacks ? "✓ " : "    ") + "Use Stacks", action: () => Config.options.desktop.useStacks = !Config.options.desktop.useStacks });
            items.push({ text: (Config.options.desktop.icons ? "✓ " : "    ") + "Show Files on Desktop", action: () => Config.options.desktop.icons = !Config.options.desktop.icons });
            items.push({ separator: true });
            items.push({ text: "Desktop Settings…", action: () => Panels.openSettings("desktop") });
        }
        menu = { items: items, x: x, y: y };
    }
    function startEditing() {
        Panels.close();
        Panels.screen = root.screen;
        Panels.widgetEditing = true;
    }
    Loader {
        active: root.menu !== null
        sourceComponent: ContextMenu {
            screen: root.screen
            items: root.menu?.items ?? []
            pointX: (root.menu?.x ?? 0) + menuWidth / 2 // opens at the pointer, like macOS
            pointY: root.menu?.y ?? 0
            upward: false
            onDismissed: root.menu = null
        }
    }

    // ── Files on the desktop (under the widgets) ────────────────────────
    DesktopIcons {
        desk: root
        visible: Config.options.desktop.icons
    }

    // ── Widgets ─────────────────────────────────────────────────────────
    // One slot per widget, changed in place: a move or resize keeps every
    // delegate, and adding or removing one inserts or removes just that one
    // (a count model would rebuild them all, and they'd all flicker)
    ListModel { id: slots }
    property var slotKeys: []
    function syncSlots() {
        const next = widgets.map(w => w.kind + ":" + (w.id ?? ""));
        const prev = slotKeys;
        let head = 0;
        while (head < prev.length && head < next.length && prev[head] === next[head])
            head++;
        let tail = 0;
        while (tail < prev.length - head && tail < next.length - head && prev[prev.length - 1 - tail] === next[next.length - 1 - tail])
            tail++;
        if (prev.length - head - tail > 0)
            slots.remove(head, prev.length - head - tail);
        for (let i = head; i < next.length - tail; i++)
            slots.insert(i, { key: next[i] });
        slotKeys = next;
    }
    onWidgetsChanged: syncSlots()
    Component.onCompleted: syncSlots()
    Repeater {
        id: frames
        model: slots
        WidgetFrame {
            widget: root.placed[index] ?? { kind: "", size: "s", col: 0, row: 0 }
            defs: widgetDefs
            origin: root.origin
            editing: root.editing
            onMoved: (i, x, y) => root.moveTo(i, x, y)
            onDragging: (i, x, y, active) => root.preview(i, x, y, active)
            onRemoveRequested: i => root.remove(i)
        }
    }

    // Empty desktop while editing: a hint where widgets go
    Text {
        visible: root.editing && root.widgets.length === 0
        x: root.origin.x
        y: root.origin.y
        text: "Pick widgets from the gallery below"
        color: Qt.rgba(1, 1, 1, 0.85)
        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
        style: Text.Raised
        styleColor: Qt.rgba(0, 0, 0, 0.4)
    }
}
