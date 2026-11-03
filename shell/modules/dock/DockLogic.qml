import QtQuick
import Quickshell
import qs.common
import qs.services

// What the dock does in both looks, once (DockGlass and MaterialDock only
// draw it): its items in sections, where the pointer is, and auto-hide. Put
// it in the dock window over everything (it fills the window and watches the
// pointer without taking it), give it the window's rectangles, and use its
// `mask` as the window's.
//
// Auto-hide (macOS style): hidden, only a 2 px strip at the bottom edge takes
// input; resting the pointer there for a moment reveals the dock. Shown, the
// fixed zone keeps it up; leaving it hides the dock after a short grace.
Item {
    id: root

    required property var window // the dock window (its menuFor and stackOpen)
    // In window coordinates: the zone that keeps a revealed dock up, and the
    // pill at its largest (input when it doesn't auto-hide)
    property rect zoneRect
    property rect pillRect
    // On screen: where the pill rests, for "only when a window overlaps it"
    property rect coverRect
    readonly property var cfg: Config.options.dock

    anchors.fill: parent
    z: 100

    // ── Items ───────────────────────────────────────────────────────────
    readonly property var mainItems: (cfg.appsButton ? [{ kind: "launcher", id: "Apps" }] : []).concat(Apps.dockApps.map(a => Object.assign({ kind: "app" }, a)))
    readonly property var recentItems: cfg.recentApps ? Apps.recentFor(3).map(a => Object.assign({ kind: "recent" }, a)) : []
    readonly property var endItems: Apps.minimized.map(t => ({ kind: "minimized", id: "minimized:" + t.address, entry: Apps.entryFor(t.lastIpcObject?.class ?? ""), minimized: t })).concat(Config.options.phone.dock && Phone.phone ? [{ kind: "phone", id: "phone:" + Phone.phone.serial, phone: Phone.phone }] : []).concat(cfg.downloads ? [{ kind: "downloads", id: "Downloads" }] : []).concat(cfg.trash ? [{ kind: "trash", id: "Trash" }] : [])
    // Flat list with section breaks marked on the first item of a section
    readonly property var items: {
        const out = mainItems.slice();
        if (recentItems.length > 0)
            out.push(...recentItems.map((x, i) => Object.assign({ divider: i === 0 }, x)));
        if (endItems.length > 0)
            out.push(...endItems.map((x, i) => Object.assign({ divider: i === 0 }, x)));
        return out;
    }
    // The Repeater's model only changes when the set of items does: the
    // recent-apps list is rewritten on every focus change, and a new
    // array would rebuild every icon (reloading icons, cutting animations)
    property var shownItems: []
    property string shownKey: ""
    onItemsChanged: {
        const key = items.map(i => i.kind + ":" + i.id + (i.divider ? "|" : "")).join(",");
        // Same ids but new entry objects: the app list reloaded (an app
        // was installed or updated) and the old entries are gone, which
        // left blank icons until something else changed the dock
        const stale = items.some((x, i) => x.entry !== shownItems[i]?.entry);
        if (key !== shownKey || stale) {
            shownKey = key;
            shownItems = items;
        }
    }
    Component.onCompleted: itemsChanged()
    readonly property int dividers: items.filter(i => i.divider).length

    // ── Pointer ─────────────────────────────────────────────────────────
    // One static tracker over the whole window: every hover decision is its
    // pointer position tested against fixed rectangles. Nothing that moves
    // (the pill slides, magnification resizes it) decides hover, so nothing
    // can feed back and oscillate.
    readonly property bool tracking: tracker.hovered
    readonly property point pointer: tracker.hovered ? tracker.point.position : Qt.point(-1, -1)
    readonly property bool onEdge: tracker.hovered && pointer.y >= height - 2
    readonly property bool inZone: tracker.hovered && pointer.x >= zoneRect.x && pointer.x < zoneRect.x + zoneRect.width && pointer.y >= zoneRect.y
    // Passive: sees the pointer, takes no clicks or hover from the icons
    HoverHandler { id: tracker }
    DragHandler {
        target: null
        acceptedDevices: PointerDevice.TouchScreen
        xAxis.enabled: false
        onTranslationChanged: if (translation.y < -24 && root.autoHiding) root.touchReveal()
    }

    // ── Auto-hide ───────────────────────────────────────────────────────
    readonly property bool autoHiding: cfg.autoHide === "always" || (cfg.autoHide === "windows" && overlapped)
    readonly property bool overlapped: Desktop.covered(window.screen, coverRect)
    property bool revealed: false
    readonly property bool hidden: autoHiding && !revealed && window.menuFor === "" && !window.stackOpen
    readonly property bool wantsUp: onEdge || inZone
    onOnEdgeChanged: onEdge && !revealed ? revealDelay.restart() : revealDelay.stop()
    onWantsUpChanged: wantsUp ? hideDelay.stop() : (revealed ? hideDelay.restart() : undefined)
    onRevealedChanged: if (revealed && !wantsUp) hideDelay.restart()
    Timer {
        id: revealDelay
        interval: 120
        onTriggered: root.revealed = true
    }
    Timer {
        id: hideDelay
        interval: 300
        onTriggered: if (!touchHold.running) root.revealed = false
    }
    // Touchscreen: swipe up from the bottom edge; there's no hover to keep
    // the dock up, so it stays a few seconds
    function touchReveal() {
        revealed = true;
        touchHold.restart();
    }
    Timer {
        id: touchHold
        interval: 4000
        onTriggered: if (!root.wantsUp) root.revealed = false
    }

    // Input: the edge strip (auto-hide), plus the fixed zone once shown or
    // the pill's largest extent. All fixed rectangles: the mask only changes
    // when hidden flips, and then the pointer is inside both shapes
    // (revealing from the edge) or outside both (hiding after leaving).
    readonly property Region mask: Region {
        // The bottom edge always takes input when auto-hiding: anywhere along
        // it reveals the dock and keeps it up (no reveal/hide loop at the edge)
        Region {
            y: root.height - 2
            width: root.autoHiding || root.hidden ? root.width : 0
            height: 2
        }
        Region {
            x: root.zoneRect.x
            y: root.zoneRect.y
            width: !root.hidden && root.autoHiding ? root.zoneRect.width : 0
            height: root.zoneRect.height
        }
        Region {
            x: root.pillRect.x
            y: root.pillRect.y
            width: !root.autoHiding ? root.pillRect.width : 0
            height: root.pillRect.height
        }
    }

    // Opening any panel closes the menu and the Downloads stack
    Connections {
        target: Panels
        function onOpenChanged() {
            root.window.menuFor = "";
            root.window.stackOpen = false;
        }
    }
}
