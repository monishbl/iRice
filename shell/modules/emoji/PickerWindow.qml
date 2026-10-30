import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass

// The emoji picker panel: search, category tabs, a sectioned grid and a
// footer naming the selected item. A transparent full-screen layer (like
// Popover) so a click outside closes it; the panel sits near the pointer.
GlassWindow {
    id: root

    readonly property bool shown: Panels.isOpen("emoji", screen)

    // ---- Geometry ----------------------------------------------------------

    readonly property int cell: 44 // emoji cell (glyph 28)
    readonly property int cols: 9
    readonly property int pad: 12
    readonly property int contentW: cell * cols // 396
    readonly property int gridH: 276
    readonly property int panelW: contentW + 2 * pad
    readonly property int panelH: pad + 36 + 8 + 32 + 4 + gridH + 56 + 4
    readonly property real margin: glass.shadowMargin

    // Top-left of the panel on this screen, set once per open (see EmojiStore.prepare)
    property real panelX: ((screen?.width ?? panelW) - panelW) / 2
    property real panelY: ((screen?.height ?? panelH) - panelH) / 2
    function place() {
        const sw = screen?.width ?? 0, sh = screen?.height ?? 0;
        const ox = screen?.x ?? 0, oy = screen?.y ?? 0;
        const p = EmojiStore.pointer, w = EmojiStore.window;
        const gap = 8;
        let x = (sw - panelW) / 2, y = (sh - panelH) / 2;
        // The window must be (mostly) on this screen to count
        const onScreen = w && w.x + w.w / 2 >= ox && w.x + w.w / 2 < ox + sw && w.y + w.h / 2 >= oy && w.y + w.h / 2 < oy + sh;
        if (onScreen && p && p.x >= w.x && p.x < w.x + w.w && p.y >= w.y && p.y < w.y + w.h) {
            // Below and right of the pointer; flip when there's no room
            const lx = p.x - ox, ly = p.y - oy;
            x = lx + gap;
            y = ly + 20;
            if (x + panelW > sw - gap)
                x = lx - panelW - gap;
            if (y + panelH > sh - gap)
                y = ly - panelH - gap;
        } else if (onScreen) {
            x = w.x - ox + (w.w - panelW) / 2;
            y = w.y - oy + (w.h - panelH) / 2;
        }
        panelX = Math.round(Math.max(gap, Math.min(x, sw - panelW - gap)));
        panelY = Math.round(Math.max(gap, Math.min(y, sh - panelH - gap)));
    }

    // ---- Open and close ----------------------------------------------------

    // Waits (briefly) for the pointer lookup so it doesn't jump after appearing
    property real progress: 0
    function startOpen() {
        place();
        closeAnim.stop();
        openAnim.start();
    }
    Connections {
        target: EmojiStore
        function onPlacedChanged() {
            if (!EmojiStore.placed || !root.shown)
                return;
            if (!openAnim.running && root.progress === 0)
                root.startOpen();
            else if (root.progress < 0.5)
                root.place(); // arrived just after the fallback: still barely visible
        }
    }
    Timer {
        id: placeFallback
        interval: 250
        onTriggered: if (root.shown && root.progress === 0 && !openAnim.running) root.startOpen()
    }
    function opened() {
        reset();
        // (later: EmojiStore.prepare() runs on the same Panels change, maybe after this)
        Qt.callLater(() => {
            if (!root.shown)
                return;
            if (EmojiStore.placed)
                root.startOpen();
            else
                placeFallback.restart();
        });
    }
    Component.onCompleted: if (shown) opened()
    onShownChanged: {
        if (shown) {
            opened();
        } else {
            openAnim.stop();
            closeAnim.start();
            // Closed before warm-up finished: the window unmaps and may drop
            // its glyphs, so start over next time
            if (!warmed)
                warmAt = 0;
        }
    }
    SpringAnimation {
        id: openAnim
        target: root
        property: "progress"
        to: 1
        spring: Theme.motion.spring + 0.8
        damping: Theme.motion.damping + 0.06
        epsilon: 0.004
    }
    NumberAnimation {
        id: closeAnim
        target: root
        property: "progress"
        to: 0
        duration: Theme.motion.popover * 0.7
        easing.type: Easing.BezierSpline
        easing.bezierCurve: Theme.motion.exit
    }

    name: "emoji"
    // Once its emoji are warmed, the window stays mapped while closed (fully
    // transparent, no input, no keyboard): hiding it would drop the drawn
    // glyphs and the next open would draw them all again
    visible: (progress > 0 && closeGuard.open) || warmed
    mask: Region { width: root.shown ? root.width : 0; height: root.shown ? root.height : 0 }
    CloseGuard { id: closeGuard; shown: root.shown }
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }

    // ---- State -------------------------------------------------------------

    // Tabs: 0 frequently used, 1-8 emoji groups, 9 kaomoji, 10 symbols, 11 GIFs
    readonly property var tabs: [
        { icon: "clock-counter-clockwise", label: "Frequently used" },
        { icon: "smiley", label: "Smileys & people" },
        { icon: "paw-print", label: "Animals & nature" },
        { icon: "hamburger", label: "Food & drink" },
        { icon: "soccer-ball", label: "Activities" },
        { icon: "car", label: "Travel & places" },
        { icon: "lightbulb", label: "Objects" },
        { icon: "heart", label: "Symbols" },
        { icon: "flag", label: "Flags" },
        { text: "^_^", label: "Kaomoji" },
        { icon: "sigma", label: "Text symbols" },
        { icon: "gif", label: "GIFs" }
    ]
    property int tab: 0 // highlighted tab
    readonly property string mode: tab === 11 ? "gif" : tab === 10 ? "symbols" : tab === 9 ? "kaomoji" : "emoji"
    property string query: ""
    readonly property bool searching: query.trim() !== ""
    property int sel: 0
    property bool keyboardNav: false // the last move was a key (hover doesn't steal the selection then)

    function reset() {
        field.text = "";
        query = "";
        tab = 0; // the list opens at the top: frequently used
        sel = 0;
        tonePopup.close();
        field.forceActiveFocus();
        Qt.callLater(() => list.positionViewAtBeginning());
    }

    // ---- Warming the emoji glyphs ------------------------------------------
    // Qt draws each colour emoji from the font the first time it's shown
    // (about a millisecond each), which made new search results stall. While
    // the picker is open and left alone, one tiny, nearly transparent text
    // (`warmText`, outside the panel) shows the emoji 40 at a time so each is
    // drawn once into this window's glyph cache. The window then stays mapped
    // while closed (transparent, no input), so the cache is kept.
    property bool warmed: false
    property int warmAt: 0
    // A different emoji style needs its own glyphs drawn
    Connections {
        target: EmojiFont
        function onFamilyChanged() {
            root.warmAt = 0;
            root.warmed = false;
        }
    }
    Timer {
        id: warmUp
        interval: 200
        repeat: true
        running: root.shown && !root.warmed && EmojiStore.loaded
        property real idleUntil: 0
        function pause() {
            idleUntil = Date.now() + 800;
        }
        // Emoji first (at the cell size, in the emoji font), then the text
        // symbols and kaomoji (as the search results show them)
        onTriggered: {
            if (Date.now() < idleUntil)
                return;
            const e = EmojiStore.emoji, sy = EmojiStore.symbolsFlat, ka = EmojiStore.kaomojiFlat;
            const total = e.length + sy.length + ka.length;
            if (root.warmAt >= total) {
                warmText.text = "";
                root.warmed = true;
                return;
            }
            const at = root.warmAt;
            const [src, base, emoji, size] = at < e.length ? [e, 0, true, 28] : at < e.length + sy.length ? [sy, e.length, false, 20] : [ka, e.length + sy.length, false, Theme.font.body];
            const from = at - base, to = Math.min(src.length, from + 40);
            let t = "";
            for (let k = from; k < to; k++)
                t += src[k].c + (emoji ? "" : " ");
            warmText.font.family = emoji ? EmojiFont.family : Theme.font.family;
            warmText.font.pixelSize = size;
            warmText.text = t;
            root.warmAt = base + to;
        }
    }
    Text {
        id: warmText
        x: -2000 // off screen: drawn (so its glyphs are cached), never seen
        y: 0
        opacity: 0.01
        font.family: EmojiFont.family
        font.pixelSize: 28
    }

    // ---- The list model ----------------------------------------------------

    // Sections → rows. A row is a header, a hint (empty section), or up to
    // `cols` items. `flat` holds every item in order; `pos[i]` its row and column.
    readonly property var view: {
        EmojiStore.loaded;
        EmojiStore.usage;
        const sections = [];
        const S = EmojiStore;
        if (!S.loaded || mode === "gif")
            return { rows: [], flat: [], pos: [], headers: [] };
        const emojiItem = e => ({ kind: "emoji", e: e });
        const textItem = kind => e => ({ kind: kind, e: e });
        if (searching) {
            const r = S.search(query, 120);
            const parts = {
                emoji: { title: "Emoji", kind: "emoji", cols: cols, items: r.emoji.slice(0, 90).map(emojiItem) },
                kaomoji: { title: "Kaomoji", kind: "kaomoji", cols: 3, items: r.kaomoji.slice(0, 30).map(textItem("kaomoji")) },
                symbols: { title: "Text symbols", kind: "symbol", cols: cols, items: r.symbols.slice(0, 45).map(textItem("symbol")) }
            };
            const order = mode === "kaomoji" ? ["kaomoji", "emoji", "symbols"] : mode === "symbols" ? ["symbols", "emoji", "kaomoji"] : ["emoji", "kaomoji", "symbols"];
            for (const k of order)
                if (parts[k].items.length > 0)
                    sections.push(parts[k]);
        } else if (mode === "emoji") {
            const freq = S.frequent("emoji", 2 * cols).map(c => S.byChar[c]).filter(e => !!e);
            sections.push({ title: "Frequently used", kind: "emoji", cols: cols, tab: 0, items: freq.map(emojiItem), hint: "Emoji you use will show up here." });
            S.groups.forEach((g, gi) => sections.push({ title: g.title, kind: "emoji", cols: cols, tab: gi + 1, items: (S.emojiByGroup[gi] ?? []).map(emojiItem) }));
        } else {
            const kind = mode === "kaomoji" ? "kaomoji" : "symbol";
            const src = mode === "kaomoji" ? S.kaomoji : S.symbols;
            const c = mode === "kaomoji" ? 3 : cols;
            const all = {};
            src.forEach(s => s.items.forEach(i => all[i.c] = i));
            const recent = S.frequent(kind, c * (mode === "kaomoji" ? 2 : 1)).map(x => all[x]).filter(x => !!x);
            if (recent.length > 0)
                sections.push({ title: "Recently used", kind: kind, cols: c, items: recent.map(textItem(kind)) });
            src.forEach(s => sections.push({ title: s.title, kind: kind, cols: c, items: s.items.map(textItem(kind)) }));
        }
        const rows = [], flat = [], pos = [], headers = [];
        sections.forEach((s, si) => {
            headers.push(rows.length);
            rows.push({ type: "header", title: s.title, sec: si, tab: s.tab ?? -1 });
            if (s.items.length === 0 && s.hint)
                rows.push({ type: "hint", text: s.hint, sec: si, tab: s.tab ?? -1 });
            for (let i = 0; i < s.items.length; i += s.cols) {
                const chunk = s.items.slice(i, i + s.cols);
                const r = rows.length;
                chunk.forEach((it, ci) => pos.push({ row: r, col: ci }));
                rows.push({ type: "items", kind: s.kind, cols: s.cols, start: flat.length, items: chunk, sec: si, tab: s.tab ?? -1 });
                flat.push(...chunk);
            }
        });
        return { rows: rows, flat: flat, pos: pos, headers: headers };
    }
    onViewChanged: sel = Math.min(sel, Math.max(0, view.flat.length - 1))
    readonly property var current: view.flat[sel] ?? null

    // ---- Keyboard ----------------------------------------------------------

    function moveH(d) {
        if (view.flat.length === 0)
            return;
        keyboardNav = true;
        sel = Math.max(0, Math.min(view.flat.length - 1, sel + d));
        reveal();
    }
    function moveV(d) {
        if (view.flat.length === 0)
            return;
        keyboardNav = true;
        const p = view.pos[sel];
        const from = view.rows[p.row];
        let r = p.row + d;
        while (r >= 0 && r < view.rows.length && view.rows[r].type !== "items")
            r += d;
        if (r < 0 || r >= view.rows.length) {
            // Past the end: go to the first or last item
            sel = d < 0 ? 0 : view.flat.length - 1;
        } else {
            const to = view.rows[r];
            const col = from.cols === to.cols ? p.col : Math.round(p.col * to.cols / from.cols);
            sel = to.start + Math.min(col, to.items.length - 1);
        }
        reveal();
    }
    function reveal() {
        const r = view.pos[sel]?.row ?? 0;
        // Show the section title too when on a section's first row
        if (r > 0 && view.rows[r - 1].type === "header")
            list.positionViewAtIndex(r - 1, ListView.Contain);
        list.positionViewAtIndex(r, ListView.Contain);
    }
    function nextTab(d) {
        setTab((tab + d + tabs.length) % tabs.length);
    }
    function setTab(t) {
        tonePopup.close();
        const wasMode = mode;
        tab = t;
        sel = 0;
        if (mode === "gif") {
            Klipy.fetch(query);
            gifGrid.currentIndex = 0;
            return;
        }
        if (searching && mode === wasMode)
            return;
        if (mode === "emoji" && !searching) {
            // Jump to that section (frequently used is the first)
            Qt.callLater(() => {
                const h = view.headers[t];
                if (h !== undefined) {
                    scrollSync.until = Date.now() + 250;
                    list.positionViewAtIndex(h, ListView.Beginning);
                    sel = view.rows.find((r, i) => i > h && r.type === "items")?.start ?? 0;
                    tab = t;
                }
            });
        } else {
            Qt.callLater(() => list.positionViewAtBeginning());
        }
    }

    function activate(i, copyOnly) {
        const it = view.flat[i];
        if (!it)
            return;
        if (it.kind === "emoji")
            EmojiStore.pick(EmojiStore.display(it.e), "emoji", copyOnly, it.e.c);
        else
            EmojiStore.pick(it.e.c, it.kind, copyOnly);
    }
    function activateGif(i, copyOnly) {
        const g = Klipy.at(i);
        if (g)
            EmojiStore.pick(g.url, "gif", copyOnly);
    }

    // ---- Outside click closes ----------------------------------------------

    MouseArea {
        anchors.fill: parent
        enabled: root.shown
        onClicked: Panels.close()
    }

    // ---- The panel ---------------------------------------------------------

    Item {
        id: body
        x: root.panelX - root.margin
        y: root.panelY - root.margin + (1 - root.progress) * -6
        width: root.panelW + 2 * root.margin
        height: root.panelH + 2 * root.margin
        opacity: Math.min(1, root.progress)
        scale: 0.97 + 0.03 * Math.min(1, root.progress)
        onXChanged: glass.updatePosition()
        onYChanged: glass.updatePosition()
        onScaleChanged: glass.updatePosition()

        Glass {
            id: glass
            anchors.fill: parent
            radius: Theme.radius.popover + 4
            bevel: 20
        }
        // Clicks on the panel don't reach the outside catcher
        MouseArea {
            anchors.fill: parent
            anchors.margins: root.margin
            onClicked: tonePopup.close()
        }

        Item {
            id: content
            x: root.margin + root.pad
            y: root.margin + root.pad
            width: root.contentW
            height: root.panelH - root.pad - 4

            // Search
            Squircle {
                id: search
                width: parent.width
                height: 36
                radius: Theme.radius.control
                color: Theme.color.fill
                border.width: 1
                border.color: field.activeFocus ? Theme.accent : "transparent"

                Icon {
                    x: 10
                    anchors.verticalCenter: parent.verticalCenter
                    name: "magnifying-glass"
                    size: 16
                    color: Theme.color.tertiary
                }
                TextInput {
                    id: field
                    anchors { fill: parent; leftMargin: 34; rightMargin: clearButton.visible ? 36 : 10 }
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.color.label
                    selectionColor: Theme.accentAlpha(0.4)
                    selectByMouse: true
                    clip: true
                    focus: true
                    font { family: Theme.font.family; pixelSize: Theme.font.body + 1 }
                    onTextChanged: {
                        warmUp.pause();
                        root.sel = 0;
                        tonePopup.close();
                        searchDebounce.restart();
                    }
                    Keys.onPressed: e => {
                        const shift = (e.modifiers & Qt.ShiftModifier) !== 0;
                        if (tonePopup.visible) {
                            tonePopup.key(e);
                            return;
                        }
                        const gif = root.mode === "gif";
                        switch (e.key) {
                        case Qt.Key_Escape:
                            Panels.close();
                            break;
                        case Qt.Key_Tab:
                            root.nextTab(1);
                            break;
                        case Qt.Key_Backtab:
                            root.nextTab(-1);
                            break;
                        case Qt.Key_Left:
                            gif ? gifGrid.moveCurrentIndexLeft() : root.moveH(-1);
                            break;
                        case Qt.Key_Right:
                            gif ? gifGrid.moveCurrentIndexRight() : root.moveH(1);
                            break;
                        case Qt.Key_Up:
                            gif ? gifGrid.moveCurrentIndexUp() : root.moveV(-1);
                            break;
                        case Qt.Key_Down:
                            if (!gif && (e.modifiers & Qt.AltModifier))
                                tonePopup.openFor(root.sel);
                            else
                                gif ? gifGrid.moveCurrentIndexDown() : root.moveV(1);
                            break;
                        case Qt.Key_Menu:
                            if (!gif)
                                tonePopup.openFor(root.sel);
                            break;
                        case Qt.Key_Return:
                        case Qt.Key_Enter:
                            if (stateArea.visible && stateArea.c.action)
                                stateArea.act(false);
                            else
                                gif ? root.activateGif(gifGrid.currentIndex, shift) : root.activate(root.sel, shift);
                            break;
                        default:
                            return;
                        }
                        e.accepted = true;
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: field.text === ""
                        text: root.mode === "gif" ? "Search KLIPY" : "Search emoji, kaomoji and symbols"
                        color: Theme.color.tertiary
                        font: field.font
                        elide: Text.ElideRight
                        width: parent.width
                    }
                }
                IconButton {
                    id: clearButton
                    anchors { right: parent.right; rightMargin: 4; verticalCenter: parent.verticalCenter }
                    visible: field.text !== ""
                    icon: "x-circle"
                    fill: true
                    iconColor: Theme.color.tertiary
                    onClicked: {
                        field.text = "";
                        field.forceActiveFocus();
                    }
                }
                Timer {
                    id: searchDebounce
                    interval: root.mode === "gif" ? 350 : 60
                    onTriggered: {
                        root.query = field.text;
                        if (root.mode === "gif")
                            Klipy.fetch(field.text);
                        else
                            Qt.callLater(() => list.positionViewAtBeginning());
                    }
                }
            }

            // Category tabs
            Row {
                id: tabRow
                z: 5 // tooltips over the grid
                anchors { top: search.bottom; topMargin: 8 }
                width: parent.width
                height: 32
                readonly property real tabW: width / root.tabs.length
                Repeater {
                    model: root.tabs
                    Item {
                        id: tabItem
                        required property var modelData
                        required property int index
                        readonly property bool active: index === root.tab
                        width: tabRow.tabW
                        height: tabRow.height

                        Squircle {
                            anchors.centerIn: parent
                            width: 30
                            height: 30
                            radius: Theme.radius.control - 2
                            color: tabMouse.pressed ? Theme.color.fillPressed : tabItem.active ? Theme.color.fill : tabMouse.containsMouse ? Qt.alpha(Theme.color.fill, 0.6) : "transparent"
                            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: !!tabItem.modelData.icon
                            name: tabItem.modelData.icon ?? ""
                            fill: tabItem.active
                            size: 18
                            color: tabItem.active ? Theme.color.label : Theme.color.tertiary
                        }
                        Text {
                            anchors.centerIn: parent
                            visible: !!tabItem.modelData.text
                            text: tabItem.modelData.text ?? ""
                            color: tabItem.active ? Theme.color.label : Theme.color.tertiary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                        }
                        // Active marker under the tab
                        Rectangle {
                            anchors { bottom: parent.bottom; horizontalCenter: parent.horizontalCenter }
                            width: tabItem.active ? 14 : 0
                            height: 2
                            radius: 1
                            color: Theme.accent
                            Behavior on width { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
                        }
                        MouseArea {
                            id: tabMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.setTab(tabItem.index);
                                field.forceActiveFocus();
                            }
                        }
                        // Name on hover, after a moment
                        Squircle {
                            visible: tabMouse.containsMouse && !tipTimer.running && !tabMouse.pressed
                            z: 10
                            anchors { top: parent.bottom; topMargin: 4; horizontalCenter: parent.horizontalCenter }
                            width: tipText.implicitWidth + 16
                            height: 24
                            radius: Theme.radius.item
                            color: Theme.dark ? "#3A3A3F" : "#1C1C1E" // (same as the editor's tooltips)
                            Text {
                                id: tipText
                                anchors.centerIn: parent
                                text: tabItem.modelData.label
                                color: "#F2F2F5"
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                            Timer {
                                id: tipTimer
                                interval: 600
                                running: tabMouse.containsMouse
                            }
                        }
                    }
                }
            }

            // ---- Emoji, kaomoji, symbols, search results ----
            ListView {
                id: list
                visible: root.mode !== "gif" && EmojiStore.loaded && root.view.flat.length + root.view.rows.length > 0 && !(root.searching && root.view.flat.length === 0)
                anchors { top: tabRow.bottom; topMargin: 4; left: parent.left; right: parent.right }
                height: root.gridH
                clip: true
                model: root.view.rows
                boundsBehavior: Flickable.StopAtBounds
                cacheBuffer: 200
                reuseItems: true
                interactive: true

                // The tab follows the section at the top while scrolling
                QtObject {
                    id: scrollSync
                    property real until: 0 // a tab click scrolls: don't let that pick a tab
                }
                onContentYChanged: {
                    warmUp.pause();
                    if (root.mode !== "emoji" || root.searching)
                        return;
                    if (Date.now() < scrollSync.until)
                        return;
                    const i = indexAt(10, contentY + 8);
                    const r = root.view.rows[i];
                    if (r && r.tab >= 0 && r.tab !== root.tab)
                        root.tab = r.tab;
                }

                delegate: Item {
                    id: rowItem
                    required property var modelData
                    required property int index
                    width: list.width
                    height: modelData.type === "header" ? 28 : modelData.type === "hint" ? 40 : modelData.kind === "kaomoji" ? 40 : root.cell

                    Text {
                        visible: rowItem.modelData.type === "header"
                        anchors { left: parent.left; leftMargin: 6; bottom: parent.bottom; bottomMargin: 6 }
                        text: rowItem.modelData.title ?? ""
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                    }
                    Text {
                        visible: rowItem.modelData.type === "hint"
                        anchors { left: parent.left; leftMargin: 6; verticalCenter: parent.verticalCenter }
                        text: rowItem.modelData.text ?? ""
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }

                    // One highlight and one mouse area per row (cells are just
                    // their text): a search result builds fewer objects
                    Squircle {
                        readonly property int col: root.sel - rowItem.modelData.start
                        visible: rowItem.modelData.type === "items" && col >= 0 && col < (rowItem.modelData.items?.length ?? 0)
                        x: col * rowItem.cellW + 1
                        y: 1
                        width: rowItem.cellW - 2
                        height: rowItem.height - 2
                        radius: Theme.radius.control
                        color: rowMouse.pressed && rowMouse.pressCol === col ? Theme.color.fillPressed : Qt.alpha(Theme.accent, Theme.dark ? 0.32 : 0.16)
                    }
                    readonly property real cellW: list.width / (modelData.cols || 1)
                    Row {
                        visible: rowItem.modelData.type === "items"
                        Repeater {
                            id: cells
                            model: rowItem.modelData.type === "items" ? rowItem.modelData.items : []
                            Item {
                                id: cellItem
                                required property var modelData
                                required property int index
                                readonly property bool isEmoji: modelData.kind === "emoji"
                                width: rowItem.cellW
                                height: rowItem.height
                                Text {
                                    anchors.centerIn: parent
                                    width: parent.width - 8
                                    horizontalAlignment: Text.AlignHCenter
                                    text: cellItem.isEmoji ? EmojiStore.display(cellItem.modelData.e) : cellItem.modelData.e.c
                                    elide: cellItem.isEmoji ? Text.ElideNone : Text.ElideRight
                                    maximumLineCount: 1
                                    color: Theme.color.label
                                    font.family: cellItem.isEmoji ? EmojiFont.family : Theme.font.family
                                    font.pixelSize: cellItem.isEmoji ? 28 : cellItem.modelData.kind === "kaomoji" ? Theme.font.body : 20
                                    scale: rowMouse.pressed && rowMouse.pressCol === cellItem.index ? 0.9 : 1
                                }
                                // Has skin tones (right-click or hold)
                                Rectangle {
                                    visible: cellItem.isEmoji && EmojiStore.supportsTone(cellItem.modelData.e)
                                    anchors { right: parent.right; bottom: parent.bottom; margins: 5 }
                                    width: 4
                                    height: 4
                                    radius: 2
                                    color: Theme.color.tertiary
                                }
                            }
                        }
                    }
                    MouseArea {
                        id: rowMouse
                        visible: rowItem.modelData.type === "items"
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        pressAndHoldInterval: 450
                        cursorShape: colAt(mouseX) >= 0 ? Qt.PointingHandCursor : Qt.ArrowCursor
                        property bool held: false
                        property int pressCol: -1
                        function colAt(x) {
                            const c = Math.floor(x / rowItem.cellW);
                            return c >= 0 && c < (rowItem.modelData.items?.length ?? 0) ? c : -1;
                        }
                        function toneable(c) {
                            const it = rowItem.modelData.items[c];
                            return it.kind === "emoji" && EmojiStore.supportsTone(it.e);
                        }
                        function hover(x) {
                            const c = colAt(x);
                            if (c >= 0 && !tonePopup.visible)
                                root.sel = rowItem.modelData.start + c;
                        }
                        onPositionChanged: m => {
                            if (root.keyboardNav) {
                                root.keyboardNav = false;
                                return;
                            }
                            hover(m.x);
                        }
                        onEntered: if (!root.keyboardNav) hover(mouseX)
                        onPressed: m => {
                            held = false;
                            pressCol = colAt(m.x);
                        }
                        onReleased: pressCol = -1
                        onCanceled: pressCol = -1
                        onPressAndHold: m => {
                            const c = colAt(m.x);
                            if (c >= 0 && toneable(c)) {
                                held = true;
                                root.sel = rowItem.modelData.start + c;
                                tonePopup.openFor(root.sel, cells.itemAt(c));
                            }
                        }
                        onClicked: m => {
                            const c = colAt(m.x);
                            if (held || c < 0)
                                return;
                            if (m.button === Qt.RightButton) {
                                if (toneable(c)) {
                                    root.sel = rowItem.modelData.start + c;
                                    tonePopup.openFor(root.sel, cells.itemAt(c));
                                }
                                return;
                            }
                            root.activate(rowItem.modelData.start + c, (m.modifiers & Qt.ShiftModifier) !== 0);
                        }
                    }
                }
            }

            // ---- GIFs ----
            GridView {
                id: gifGrid
                visible: root.mode === "gif" && Klipy.hasKey && (Klipy.status === "ready" || Klipy.status === "loading") && Klipy.count > 0
                anchors { top: tabRow.bottom; topMargin: 8; left: parent.left; right: parent.right }
                height: root.gridH - 4
                clip: true
                model: Klipy.gifs
                cellWidth: width / 3
                cellHeight: 104
                boundsBehavior: Flickable.StopAtBounds
                cacheBuffer: 104
                highlightMoveDuration: 0
                keyNavigationWraps: false
                onCurrentIndexChanged: if (currentIndex >= count - 6) Klipy.more()
                onAtYEndChanged: if (atYEnd) Klipy.more()

                delegate: Item {
                    id: gifCell
                    required property string preview
                    required property string still
                    required property int index
                    readonly property bool isCurrent: GridView.isCurrentItem
                    width: gifGrid.cellWidth
                    height: gifGrid.cellHeight

                    Squircle {
                        id: frame
                        anchors.fill: parent
                        anchors.margins: 3
                        radius: Theme.radius.control
                        color: Theme.color.fill
                        clip: true
                        // Still frame for every cell; only the selected one animates
                        Image {
                            anchors.fill: parent
                            source: gifCell.preview
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(260, 200)
                            asynchronous: true
                        }
                        AnimatedImage {
                            readonly property bool live: gifCell.isCurrent && root.shown
                            anchors.fill: parent
                            visible: live && status === AnimatedImage.Ready
                            source: live ? gifCell.still : ""
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            playing: live
                        }
                    }
                    Squircle {
                        anchors.fill: frame
                        radius: frame.radius
                        color: "transparent"
                        border.width: 2
                        border.color: Theme.accent
                        visible: gifCell.isCurrent
                    }
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onEntered: gifGrid.currentIndex = gifCell.index
                        onClicked: m => root.activateGif(gifCell.index, (m.modifiers & Qt.ShiftModifier) !== 0)
                    }
                }

                footer: Item {
                    width: gifGrid.width
                    height: Klipy.loadingMore ? 40 : 0
                    Spinner {
                        anchors.centerIn: parent
                        visible: Klipy.loadingMore
                    }
                }
            }

            // ---- States: loading, no results, GIF key / error / offline ----
            Item {
                id: stateArea
                anchors { top: tabRow.bottom; topMargin: 4; left: parent.left; right: parent.right }
                height: root.gridH
                readonly property string kind: {
                    if (root.mode === "gif") {
                        if (!Klipy.hasKey)
                            return "noKey";
                        if (Klipy.status === "loading" && Klipy.count === 0 || Klipy.status === "idle")
                            return "gifLoading";
                        if (Klipy.status === "offline" || Klipy.status === "error" || Klipy.status === "badKey")
                            return Klipy.status;
                        if (Klipy.status === "ready" && Klipy.count === 0)
                            return "noGifs";
                        return "";
                    }
                    if (EmojiStore.failed)
                        return "dataError";
                    if (!EmojiStore.loaded)
                        return "loading";
                    if (root.searching && root.view.flat.length === 0)
                        return "noResults";
                    return "";
                }
                visible: kind !== ""

                readonly property var copy: ({
                        loading: { spin: true, title: "Loading emoji…" },
                        gifLoading: { spin: true, title: Klipy.query === "" ? "Loading trending GIFs…" : "Searching GIFs…" },
                        dataError: { icon: "warning-circle", title: "Emoji couldn't load", body: "The emoji list is missing or damaged. Run tools/gen-emoji-data.py to rebuild it.", action: "Try again" },
                        noResults: { icon: "magnifying-glass", title: "No results for “" + root.query.trim() + "”", body: "Try another word, like heart, cat or shrug." },
                        noKey: { icon: "gif", title: "Add a KLIPY key to use GIFs", body: "Get a free app key at partner.klipy.com, then paste it in Settings.", action: "Open Settings", secondary: "Get a key" },
                        offline: { icon: "wifi-slash", title: "Can't reach KLIPY", body: "Check your internet connection, then try again.", action: "Try again" },
                        error: { icon: "warning-circle", title: "GIFs didn't load", body: "KLIPY returned an error" + (Klipy.errorCode > 0 ? " (" + Klipy.errorCode + ")" : "") + ". Try again in a moment.", action: "Try again" },
                        badKey: { icon: "key", title: "KLIPY didn't accept the key", body: "Check the app key in Settings. It's on partner.klipy.com.", action: "Open Settings" },
                        noGifs: { icon: "magnifying-glass", title: "No GIFs for “" + Klipy.query + "”", body: "Try another word." }
                    })
                readonly property var c: copy[kind] ?? {}

                function act(secondary) {
                    if (secondary) {
                        Qt.openUrlExternally("https://partner.klipy.com/api-keys");
                        Panels.close();
                        return;
                    }
                    if (kind === "noKey" || kind === "badKey")
                        Panels.openSettings(EmojiStore.settingsPage);
                    else if (kind === "dataError")
                        EmojiStore.load();
                    else
                        Klipy.retry();
                }

                Column {
                    anchors.centerIn: parent
                    width: parent.width - 48
                    spacing: 8
                    Spinner {
                        anchors.horizontalCenter: parent.horizontalCenter
                        visible: !!stateArea.c.spin
                    }
                    Icon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        visible: !!stateArea.c.icon
                        name: stateArea.c.icon ?? ""
                        size: 32
                        color: Theme.color.tertiary
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        text: stateArea.c.title ?? ""
                        wrapMode: Text.Wrap
                        maximumLineCount: 2
                        elide: Text.ElideRight
                        color: stateArea.c.spin ? Theme.color.secondary : Theme.color.label
                        font { family: Theme.font.family; pixelSize: stateArea.c.spin ? Theme.font.body : Theme.font.title; weight: stateArea.c.spin ? Font.Normal : Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        visible: !!stateArea.c.body
                        horizontalAlignment: Text.AlignHCenter
                        text: stateArea.c.body ?? ""
                        wrapMode: Text.Wrap
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                    Item {
                        width: 1
                        height: 8
                        visible: !!stateArea.c.action
                    }
                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 8
                        visible: !!stateArea.c.action
                        PickerButton {
                            text: stateArea.c.action ?? ""
                            primary: true
                            onClicked: stateArea.act(false)
                        }
                        PickerButton {
                            visible: !!stateArea.c.secondary
                            text: stateArea.c.secondary ?? ""
                            onClicked: stateArea.act(true)
                        }
                    }
                }
            }

            // ---- Footer: the selected item, large, with its name ----
            Item {
                id: footer
                anchors { left: parent.left; right: parent.right; top: tabRow.bottom; topMargin: 4 + root.gridH }
                height: 56

                Rectangle {
                    anchors { left: parent.left; right: parent.right; top: parent.top }
                    height: 1
                    color: Theme.color.separator
                }

                readonly property var item: root.mode === "gif" ? null : root.current
                readonly property var gif: root.mode === "gif" ? Klipy.at(gifGrid.currentIndex) : null
                readonly property bool has: (item !== null && !stateArea.visible) || (gif !== null && gifGrid.visible)

                Text {
                    id: bigGlyph
                    visible: footer.item !== null && footer.has
                    anchors { left: parent.left; leftMargin: 4; verticalCenter: parent.verticalCenter; verticalCenterOffset: 2 }
                    width: footer.item?.kind === "kaomoji" ? Math.min(implicitWidth, 150) : 40
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: !footer.item ? "" : footer.item.kind === "emoji" ? EmojiStore.display(footer.item.e) : footer.item.e.c
                    color: Theme.color.label
                    font.family: footer.item?.kind === "emoji" ? EmojiFont.family : Theme.font.family
                    font.pixelSize: footer.item?.kind === "emoji" ? 34 : footer.item?.kind === "kaomoji" ? Theme.font.title : 28
                }
                Column {
                    visible: footer.has
                    anchors { left: footer.gif ? parent.left : bigGlyph.right; leftMargin: footer.gif ? 4 : 12; right: hints.left; rightMargin: 12; verticalCenter: parent.verticalCenter; verticalCenterOffset: 2 }
                    spacing: 2
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: {
                            const g = footer.gif;
                            if (g)
                                return g.title || "GIF";
                            const it = footer.item;
                            if (!it)
                                return "";
                            let n = it.e.n || (it.kind === "kaomoji" ? "Kaomoji" : "Symbol");
                            if (it.kind === "kaomoji")
                                n = n.split(" ")[0];
                            n = n.charAt(0).toUpperCase() + n.slice(1);
                            const t = it.kind === "emoji" ? EmojiStore.toneOf(it.e) : 0;
                            return t > 0 ? n + ", " + EmojiStore.toneNames[t].toLowerCase() + " skin tone" : n;
                        }
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: footer.gif ? "Powered by KLIPY" : footer.item?.kind === "emoji" ? ":" + footer.item.e.s + ":" + (EmojiStore.supportsTone(footer.item.e) ? "  ·  right-click for skin tones" : "") : footer.item?.kind === "kaomoji" ? footer.item.e.n : footer.item ? EmojiStore.codePoints(footer.item.e.c) : ""
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                }
                Row {
                    id: hints
                    anchors { right: parent.right; rightMargin: 2; verticalCenter: parent.verticalCenter; verticalCenterOffset: 2 }
                    spacing: 10
                    Repeater {
                        model: [[["Return"], "Insert"], [["SHIFT", "Return"], "Copy"]]
                        Row {
                            required property var modelData
                            spacing: 4
                            KeyCombo { keys: parent.modelData[0] }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: parent.modelData[1]
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                    }
                }
            }

            // While choosing a tone, a click elsewhere just closes the chooser
            MouseArea {
                anchors.fill: parent
                z: 19
                visible: tonePopup.visible
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onPressed: tonePopup.close()
            }

            // ---- Skin tone chooser (right-click, hold, or Alt+Down) ----
            Squircle {
                id: tonePopup
                visible: false
                z: 20
                width: 6 * 40 + 8
                height: 48
                radius: Theme.radius.menu
                color: Theme.glass.solid
                border.width: 1
                border.color: Theme.color.separator
                property var entry: null
                property int choice: 0

                function openFor(i, cellItem) {
                    const it = root.view.flat[i];
                    if (!it || it.kind !== "emoji" || !EmojiStore.supportsTone(it.e))
                        return;
                    entry = it.e;
                    choice = EmojiStore.toneOf(it.e);
                    // Above the cell, or below when it's near the top
                    let cx, cy, ch;
                    if (cellItem) {
                        const p = cellItem.mapToItem(content, 0, 0);
                        cx = p.x + cellItem.width / 2;
                        cy = p.y;
                        ch = cellItem.height;
                    } else {
                        const pos = root.view.pos[i];
                        const rowDelegate = list.itemAtIndex(pos.row);
                        const p = rowDelegate ? rowDelegate.mapToItem(content, 0, 0) : Qt.point(0, list.y);
                        cx = (pos.col + 0.5) * root.cell;
                        cy = p.y;
                        ch = root.cell;
                    }
                    x = Math.max(0, Math.min(content.width - width, cx - width / 2));
                    y = cy - height - 4 >= list.y ? cy - height - 4 : cy + ch + 4;
                    visible = true;
                }
                function close() {
                    visible = false;
                }
                function choose(t, copyOnly) {
                    const e = entry;
                    close();
                    if (!e)
                        return;
                    EmojiStore.setTone(e, t);
                    EmojiStore.pick(EmojiStore.withTone(e, t), "emoji", copyOnly, e.c);
                }
                function key(e) {
                    e.accepted = true;
                    if (e.key === Qt.Key_Escape)
                        close();
                    else if (e.key === Qt.Key_Left)
                        choice = Math.max(0, choice - 1);
                    else if (e.key === Qt.Key_Right)
                        choice = Math.min(5, choice + 1);
                    else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter)
                        choose(choice, (e.modifiers & Qt.ShiftModifier) !== 0);
                    else if (e.key === Qt.Key_Up || e.key === Qt.Key_Down || e.key === Qt.Key_Tab)
                        close();
                    else
                        e.accepted = false;
                }

                Row {
                    anchors.centerIn: parent
                    Repeater {
                        model: 6
                        Item {
                            id: toneItem
                            required property int index
                            width: 40
                            height: 40
                            Squircle {
                                anchors.fill: parent
                                radius: Theme.radius.control
                                color: toneMouse.pressed ? Theme.color.fillPressed : toneItem.index === tonePopup.choice ? Qt.alpha(Theme.accent, Theme.dark ? 0.32 : 0.16) : toneMouse.containsMouse ? Theme.color.fill : "transparent"
                            }
                            Text {
                                anchors.centerIn: parent
                                text: tonePopup.entry ? EmojiStore.withTone(tonePopup.entry, toneItem.index) : ""
                                font { family: EmojiFont.family; pixelSize: 26 }
                            }
                            MouseArea {
                                id: toneMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onEntered: tonePopup.choice = toneItem.index
                                onClicked: m => tonePopup.choose(toneItem.index, (m.modifiers & Qt.ShiftModifier) !== 0)
                            }
                        }
                    }
                }
            }
        }
    }

    // Small spinning ring for loading states
    component Spinner: Icon {
        id: spinner
        name: "circle-notch"
        size: 24
        color: Theme.color.tertiary
        RotationAnimator on rotation {
            running: spinner.visible
            from: 0
            to: 360
            duration: 900
            loops: Animation.Infinite
        }
    }
}
