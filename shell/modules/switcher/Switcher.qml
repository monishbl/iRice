import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.glass
import qs.services

// Alt+Tab: hold Alt and press Tab for a centred glass strip of window
// thumbnails, most recently focused first, from every space. Tab / Shift+Tab
// (or the arrows) move the selection; releasing Alt focuses it, Escape
// cancels, a click focuses that window. A quick tap switches straight to the
// previous window: the strip only appears once Alt is held for `holdDelay`
// ms (or on a second Tab), so nothing flashes.
//
// Tab presses arrive as global shortcuts (irice:switcher, irice:switcherBack)
// because Hyprland eats the bound keys. Releasing Alt can't come that way:
// Hyprland drops a release bind once another key was pressed while the
// modifier was held (that's what makes "tap Super" work). So the first press
// maps the switcher's layer straight away, fully transparent and with no
// input region, holding exclusive keyboard focus; the compositor then sends
// the Alt release to it (Keys.onReleased) wherever Alt went down.
// Thumbnails are captured once, when the strip appears, never live.
Scope {
    id: root

    readonly property bool enabled: Config.options.switcher?.enabled ?? true
    readonly property int holdDelay: 150
    // Keep the closed window this long, so the next Alt+Tab maps at once
    // (a cold window takes a few frames, and a very quick tap could release
    // Alt before it has the keyboard)
    readonly property int linger: 60000

    property string phase: "idle" // idle | pending (held, strip hidden) | shown
    property var items: [] // HyprlandToplevels, snapshot taken on open
    property int selected: 0
    property var screen: null
    property bool capture: false // thumbnails are taken once the strip shows

    // Referencing the history here starts the service with the shell, so it
    // has seen every focus change before the first Alt+Tab
    readonly property int known: FocusHistory.order.length

    function key(a) {
        return FocusHistory.key(a);
    }

    // Every window on a regular space, plus minimized ones; not other special
    // workspaces (scratchpads, Show Desktop's stash)
    function candidates() {
        return Hyprland.toplevels.values.filter(t => {
            const ws = t.workspace;
            return !ws || ws.id > 0 || ws.name === "special:minimized" || ws.name === "special:irice-desktop";
        });
    }

    function begin(dir) {
        if (!enabled || System.locked)
            return;
        Hyprland.refreshToplevels(); // fresh titles and sizes for the strip
        const list = FocusHistory.sorted(candidates());
        const active = key(Apps.activeHyprland?.address ?? Hyprland.activeToplevel?.address);
        const firstIsActive = list.length > 0 && key(list[0].address) === active;
        items = list;
        selected = list.length === 0 ? 0 : dir > 0 ? (firstIsActive ? Math.min(1, list.length - 1) : 0) : list.length - 1;
        capture = false;
        screen = Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
        // Popovers close (one of them may hold the keyboard); the switcher
        // only registers as the open panel once the strip shows, so a quick
        // tap doesn't reveal an auto-hidden menu bar
        if (Panels.open !== "")
            Panels.close();
        phase = "pending";
        holdTimer.restart();
    }

    function step(d) {
        if (phase === "idle")
            return begin(d);
        if (items.length > 0)
            selected = ((selected + d) % items.length + items.length) % items.length;
        reveal(); // a second Tab shows the strip without waiting
    }

    function reveal() {
        holdTimer.stop();
        if (phase === "pending") {
            phase = "shown";
            capture = true;
            Panels.show("switcher", screen);
        }
    }

    function commit() {
        if (phase === "idle")
            return;
        const t = items[selected] ?? null;
        finish();
        if (t) {
            // After the strip has given the keyboard back: Hyprland ignores a
            // focus change while a layer holds exclusive keyboard focus
            focusLater.target = t;
            focusLater.restart();
        }
    }
    Timer {
        id: focusLater
        property var target: null
        interval: 50
        onTriggered: {
            if (target)
                root.focusWindow(target);
            target = null;
        }
    }

    function cancel() {
        finish();
    }

    function finish() {
        holdTimer.stop();
        phase = "idle";
        if (Panels.open === "switcher")
            Panels.close();
    }

    // Focusing a window by address also switches to its space (on its own
    // monitor); minimized windows come back to the current space
    function focusWindow(t) {
        if (Apps.isHidden(t)) { // minimized, or set aside by Show Desktop
            Apps.show(t);
            return;
        }
        Hyprland.dispatch("hl.dsp.focus({ window = \"address:0x" + key(t.address) + "\" })");
    }

    Timer {
        id: holdTimer
        interval: root.holdDelay
        onTriggered: root.reveal()
    }

    // Another panel opened, Escape-all (Super+Escape), or the screen locked
    Connections {
        target: Panels
        function onOpenChanged() {
            if (Panels.open !== "switcher" && root.phase === "shown") {
                holdTimer.stop();
                root.phase = "idle";
            }
        }
    }
    Connections {
        target: System
        function onLockedChanged() {
            if (System.locked)
                root.cancel();
        }
    }

    // hypr/irice/keybinds.lua: ALT + Tab -> irice:switcher,
    // ALT + SHIFT + Tab -> irice:switcherBack
    GlobalShortcut {
        appid: "irice"
        name: "switcher"
        description: "Switch windows (hold Alt, press Tab)"
        onPressed: root.step(1)
    }
    GlobalShortcut {
        appid: "irice"
        name: "switcherBack"
        description: "Switch windows, backwards"
        onPressed: root.step(-1)
    }
    // Optional: a compositor-side "Alt released" signal, if one is bound
    GlobalShortcut {
        appid: "irice"
        name: "switcherCommit"
        description: "Switch to the selected window"
        onPressed: root.commit()
    }

    // ── Window ──────────────────────────────────────────────────────────
    property bool alive: false
    onPhaseChanged: {
        if (phase !== "idle") {
            unload.stop();
            alive = true;
        } else {
            unload.restart();
        }
    }
    Timer {
        id: unload
        interval: root.linger
        onTriggered: root.alive = false
    }

    LazyLoader {
        active: root.alive

        GlassWindow {
            id: win
            name: "switcher"
            screen: root.screen ?? Quickshell.screens[0]

            readonly property bool active: root.phase !== "idle"
            readonly property bool shown: root.phase === "shown"
            readonly property int count: root.items.length

            // Layout: fixed cells, as many per row as fit the screen, rows
            // scroll past what fits vertically
            readonly property int margin: 24 // Glass shadowMargin
            readonly property int pad: 12 // tile radius 14 + 12 = panel radius 26
            readonly property int gap: 8
            readonly property int cellW: 208
            readonly property int thumbH: 120
            readonly property int cellH: 8 + thumbH + 8 + 20 + 10
            readonly property int perRow: Math.max(1, Math.min(count, Math.floor(((screen?.width ?? 1920) - 96 - 2 * margin - 2 * pad + gap) / (cellW + gap))))
            readonly property int rows: Math.ceil(count / perRow)
            readonly property int visibleRows: Math.max(1, Math.min(rows, Math.floor(((screen?.height ?? 1080) - 160 - 2 * margin - 2 * pad + gap) / (cellH + gap))))

            implicitWidth: body.width + 2 * margin
            implicitHeight: body.height + 2 * margin

            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: active ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            // No input while waiting (and while fading out): clicks go through
            mask: Region { item: win.shown ? body : null }

            // Starts at 0 so the strip animates in when the window is created
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => win.shown ? 1 : 0)
            Behavior on progress {
                NumberAnimation {
                    duration: win.shown ? Theme.motion.popover : Math.round(Theme.motion.popover * 0.65)
                    easing.type: win.shown ? Easing.OutCubic : Easing.InCubic
                }
            }

            // Mapped (transparent) as soon as Alt+Tab is pressed, so it owns
            // the keyboard before Alt comes up
            visible: active || (progress > 0 && closeGuard.open)
            CloseGuard { id: closeGuard; shown: win.active }

            Item {
                anchors.fill: parent
                focus: true
                Keys.onReleased: e => {
                    if (e.isAutoRepeat)
                        return;
                    if ([Qt.Key_Alt, Qt.Key_AltGr, Qt.Key_Meta, Qt.Key_Super_L, Qt.Key_Super_R].includes(e.key)) {
                        e.accepted = true;
                        root.commit();
                    }
                }
                Keys.onPressed: e => {
                    switch (e.key) {
                    case Qt.Key_Escape:
                        root.cancel();
                        break;
                    case Qt.Key_Return:
                    case Qt.Key_Enter:
                        root.commit();
                        break;
                    case Qt.Key_Tab:
                    case Qt.Key_Right:
                        root.step(1);
                        break;
                    case Qt.Key_Backtab:
                    case Qt.Key_Left:
                        root.step(-1);
                        break;
                    case Qt.Key_Down:
                        root.step(win.perRow);
                        break;
                    case Qt.Key_Up:
                        root.step(-win.perRow);
                        break;
                    default:
                        return;
                    }
                    e.accepted = true;
                }
            }

            Item {
                id: body
                x: win.margin
                y: win.margin
                width: win.count === 0 ? 240 : grid.width - win.gap + 2 * win.pad
                height: win.count === 0 ? 72 : grid.height - win.gap + 2 * win.pad
                opacity: win.progress
                scale: 0.97 + 0.03 * win.progress

                Glass {
                    anchors.fill: parent
                    anchors.margins: -win.margin
                    shadowMargin: win.margin
                    radius: Theme.radius.panel
                }

                // Empty state (held Alt+Tab with nothing open)
                Text {
                    anchors.centerIn: parent
                    visible: win.count === 0
                    text: "No open windows"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                }

                GridView {
                    id: grid
                    x: win.pad
                    y: win.pad
                    width: win.perRow * cellWidth
                    height: win.visibleRows * cellHeight
                    cellWidth: win.cellW + win.gap
                    cellHeight: win.cellH + win.gap
                    interactive: win.rows > win.visibleRows
                    boundsBehavior: Flickable.StopAtBounds
                    clip: win.rows > win.visibleRows
                    // Tiles exist only while the strip shows: a quick tap builds nothing
                    model: win.shown || win.progress > 0 ? root.items : []
                    currentIndex: root.selected
                    highlightFollowsCurrentItem: false
                    onCurrentIndexChanged: positionViewAtIndex(currentIndex, GridView.Contain)

                    delegate: Item {
                        id: tile
                        required property var modelData
                        required property int index
                        readonly property var ipc: modelData.lastIpcObject ?? ({})
                        readonly property string cls: ipc.class ?? modelData.wayland?.appId ?? ""
                        readonly property var entry: Apps.entryFor(cls)
                        readonly property bool current: index === root.selected
                        readonly property bool minimized: modelData.workspace?.name === "special:minimized"
                        readonly property int wsId: modelData.workspace?.id ?? 0
                        readonly property bool elsewhere: wsId > 0 && wsId !== (Hyprland.monitorFor(win.screen)?.activeWorkspace?.id ?? 0)
                        width: win.cellW
                        height: win.cellH
                        scale: mouse.pressed ? 0.97 : 1
                        Behavior on scale { ScaleAnimator { duration: Theme.motion.press; easing.type: Easing.OutCubic } }

                        // Selection (accent ring and tint) and hover
                        Rectangle {
                            anchors.fill: parent
                            radius: 14
                            color: tile.current ? Qt.alpha(Theme.accent, Theme.dark ? 0.22 : 0.14) : mouse.containsMouse ? Theme.color.fillHover : "transparent"
                            border.width: tile.current ? 2 : 0
                            border.color: Theme.accent
                            Behavior on color { ColorAnimation { duration: Theme.motion.press } }
                        }

                        // The window, fitted into the thumbnail area; its app
                        // icon until (or if) the capture arrives
                        Item {
                            id: thumbBox
                            x: 8
                            y: 8
                            width: parent.width - 16
                            height: win.thumbH
                            readonly property real srcW: tile.ipc.size?.[0] ?? 16
                            readonly property real srcH: tile.ipc.size?.[1] ?? 10
                            readonly property real k: Math.min(width / Math.max(1, srcW), height / Math.max(1, srcH))

                            ClippingRectangle {
                                anchors.centerIn: parent
                                width: Math.max(32, Math.round(thumbBox.srcW * thumbBox.k))
                                height: Math.max(24, Math.round(thumbBox.srcH * thumbBox.k))
                                radius: 8
                                color: Theme.color.fill

                                ScreencopyView {
                                    id: shot
                                    anchors.fill: parent
                                    captureSource: root.capture ? tile.modelData.wayland : null
                                    live: false
                                    constraintSize: Qt.size(width, height)
                                }
                                AppIcon {
                                    anchors.centerIn: parent
                                    visible: !shot.hasContent
                                    size: 48
                                    source: Apps.iconFor(tile.entry ?? tile.cls)
                                }
                            }
                        }

                        // Icon and title; where it lives if not on this space
                        Item {
                            x: 10
                            y: 8 + win.thumbH + 8
                            width: parent.width - 20
                            height: 20

                            AppIcon {
                                id: smallIcon
                                anchors.verticalCenter: parent.verticalCenter
                                size: 16
                                source: Apps.iconFor(tile.entry ?? tile.cls)
                            }
                            Text {
                                anchors { left: smallIcon.right; leftMargin: 6; right: place.visible ? place.left : parent.right; rightMargin: place.visible ? 6 : 0; verticalCenter: parent.verticalCenter }
                                elide: Text.ElideRight
                                text: tile.modelData.title || tile.entry?.name || tile.cls || "Window"
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: tile.current ? Font.DemiBold : Font.Medium }
                            }
                            Text {
                                id: place
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                visible: tile.minimized || tile.elsewhere
                                text: tile.minimized ? "Minimized" : "Space " + tile.wsId
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                            }
                        }

                        MouseArea {
                            id: mouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.selected = tile.index;
                                root.commit();
                            }
                        }
                    }
                }
            }
        }
    }
}
