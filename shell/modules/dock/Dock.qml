import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Glass dock, macOS layout: Apps, pinned and running apps | recent apps |
// minimized windows, Downloads, Trash. Uniform icons with a hover lift;
// magnification is optional. The window has a fixed size (no per-frame
// resizing, which made the old dock flicker); input is masked to what's visible.
// Auto-hide: off, always, or only when a window overlaps the dock.
// Settings → Dock → Show the dock off: no dock windows at all.
Variants {
    model: Config.options.dock.enabled ? Quickshell.screens : []

    GlassWindow {
        id: win
        required property var modelData
        screen: modelData
        name: "dock"
        visible: Wallpaper.started // after the wallpaper at login

        readonly property var cfg: Config.options.dock
        readonly property int iconSize: cfg.iconSize
        readonly property real maxScale: cfg.magnification ? cfg.magnifiedScale : 1
        // Tahoe's dock: a wide glass slab, not a pill. Generous padding, icons
        // apart, softly rounded continuous corners (about a third of an icon)
        readonly property int pad: Math.round(iconSize * 0.26)
        readonly property int corner: Math.round(iconSize * 0.32) // a rounded rectangle (continuous corners), never a pill
        readonly property int gap: Math.round(iconSize * Math.max(0.05, Math.min(0.4, cfg.gap ?? 0.18))) // Settings → Dock → Space between icons
        readonly property int barHeight: iconSize + 2 * pad
        readonly property int bottomGap: 6
        readonly property int dividerWidth: 1 + 2 * gap

        // ── Items ───────────────────────────────────────────────────────
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
        readonly property real restWidth: items.length * iconSize + (items.length - 1) * gap + dividers * dividerWidth

        // ── Pointer ─────────────────────────────────────────────────────
        // One static tracker over the whole window: every hover decision is
        // its pointer position tested against fixed rectangles. Nothing that
        // moves (the pill slides, magnification resizes it) decides hover, so
        // nothing can feed back and oscillate.
        readonly property bool settled: slide < 0.001 // fully shown
        readonly property point pointer: tracker.hovered ? tracker.point.position : Qt.point(-1, -1)
        readonly property bool onEdge: tracker.hovered && pointer.y >= height - 2
        readonly property bool inZone: tracker.hovered && pointer.x >= zoneRect.x && pointer.x < zoneRect.x + zoneRect.width && pointer.y >= zoneRect.y
        // On the pill (or the icons above it): drives magnification, labels, lift
        readonly property bool hovered: settled && tracker.hovered && pointer.y >= height - bottomGap - barHeight - iconSize * (maxScale - 1) - 12 && pointer.y < height - bottomGap && pointer.x >= dock.x && pointer.x < dock.x + dock.width
        // Relative to the dock's centre; magnification only once fully shown
        readonly property real pointerX: hovered ? pointer.x - width / 2 : -10000
        readonly property real pointerLocal: pointer.x - dock.x // within the pill
        property string menuFor: "" // item id whose right-click menu is open
        property var menuItem: null
        property real menuX: 0 // screen x the menu points at
        property bool stackOpen: false // Downloads stack
        property real stackX: 0

        // Largest the pill gets (magnified), so the zone never depends on it
        readonly property real maxWidth: restWidth + 2 * pad + iconSize * (maxScale - 1) * 2.6
        readonly property rect zoneRect: Qt.rect((width - maxWidth) / 2 - 24, height - (barHeight + bottomGap + iconSize * (maxScale - 1) + 44), maxWidth + 48, barHeight + bottomGap + iconSize * (maxScale - 1) + 44)
        readonly property rect pillRect: Qt.rect((width - maxWidth) / 2, height - bottomGap - barHeight - iconSize * (maxScale - 1) - 12, maxWidth, barHeight + iconSize * (maxScale - 1) + 12)

        // ── Auto-hide (macOS style) ─────────────────────────────────────
        // Hidden: only a 2px strip at the bottom edge takes input; resting the
        // pointer there for a moment reveals the dock. Shown: the fixed zone
        // keeps it up; leaving it hides the dock after a short grace.
        readonly property bool autoHiding: cfg.autoHide === "always" || (cfg.autoHide === "windows" && overlapped)
        readonly property bool overlapped: Desktop.covered(screen, Qt.rect((screen.width - restWidth) / 2 - pad, screen.height - barHeight - bottomGap, restWidth + 2 * pad, barHeight + bottomGap))
        property bool revealed: false
        readonly property bool hidden: autoHiding && !revealed && menuFor === "" && !stackOpen
        readonly property bool wantsUp: onEdge || inZone
        onOnEdgeChanged: onEdge && !revealed ? revealDelay.restart() : revealDelay.stop()
        onWantsUpChanged: wantsUp ? hideDelay.stop() : (revealed ? hideDelay.restart() : undefined)
        onRevealedChanged: if (revealed && !wantsUp) hideDelay.restart()
        Timer {
            id: revealDelay
            interval: 120
            onTriggered: win.revealed = true
        }
        Timer {
            id: hideDelay
            interval: 300
            onTriggered: if (!touchHold.running) win.revealed = false
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
            onTriggered: if (!win.wantsUp) win.revealed = false
        }
        property real slide: hidden ? 1 : 0
        Behavior on slide {
            NumberAnimation {
                duration: win.hidden ? 250 : 350
                easing.type: win.hidden ? Easing.InCubic : Easing.OutBack
                easing.overshoot: 1.2
            }
        }

        WlrLayershell.layer: WlrLayer.Top
        exclusionMode: cfg.autoHide === "off" ? ExclusionMode.Normal : ExclusionMode.Ignore
        exclusiveZone: cfg.autoHide === "off" ? barHeight + bottomGap : 0
        anchors { bottom: true; left: true; right: true }
        implicitHeight: Math.ceil(iconSize * maxScale) + 2 * pad + bottomGap + 44 // room for magnified icons and the name label

        // Input: the edge strip (auto-hide), plus the fixed zone once shown
        // or the pill's largest extent. All fixed rectangles: the mask only
        // changes when hidden flips, and then the pointer is inside both shapes
        // (revealing from the edge) or outside both (hiding after leaving).
        mask: Region {
            // The bottom edge always takes input when auto-hiding: anywhere along
            // it reveals the dock and keeps it up (no reveal/hide loop at the edge)
            Region {
                y: win.height - 2
                width: win.autoHiding || win.hidden ? win.width : 0
                height: 2
            }
            Region {
                x: win.zoneRect.x
                y: win.zoneRect.y
                width: !win.hidden && win.autoHiding ? win.zoneRect.width : 0
                height: win.zoneRect.height
            }
            Region {
                x: win.pillRect.x
                y: win.pillRect.y
                width: !win.autoHiding ? win.pillRect.width : 0
                height: win.pillRect.height
            }
        }
        Item {
            anchors.fill: parent
            z: 100
            // Passive: sees the pointer, takes no clicks or hover from the icons
            HoverHandler { id: tracker }
            DragHandler {
                target: null
                acceptedDevices: PointerDevice.TouchScreen
                xAxis.enabled: false
                onTranslationChanged: if (translation.y < -24 && win.autoHiding) win.touchReveal()
            }
        }

        // The pill; its width follows the icons (and magnification)
        Item {
            id: dock
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            // Slides all the way out of the window (lifted icons included)
            anchors.bottomMargin: win.bottomGap - (win.barHeight + win.iconSize * (win.maxScale - 1) + win.bottomGap + 24) * win.slide
            width: row.width + 2 * win.pad
            height: win.barHeight
            onXChanged: glass.updatePosition()
            onYChanged: glass.updatePosition()

            Glass {
                id: glass
                anchors.fill: parent
                anchors.margins: -shadowMargin
                radius: win.corner // (the glass shape already leaves out its shadow margin)
                bevel: 14
            }

            Row {
                id: row
                x: win.pad
                anchors.bottom: parent.bottom
                anchors.bottomMargin: win.pad
                spacing: win.gap

                Repeater {
                    model: win.shownItems
                    DockItem {
                        required property var modelData
                        required property int index
                        item: modelData
                        dock: win
                        // Resting centre of this icon, relative to the dock's centre
                        restCenter: index * (win.iconSize + win.gap) + win.items.slice(0, index + 1).filter(i => i.divider).length * win.dividerWidth + win.iconSize / 2 - win.restWidth / 2
                    }
                }
            }
        }

        // Right-click menu and Downloads stack: their own overlay windows, so a
        // click anywhere else closes them
        Loader {
            id: menuLoader
            active: win.menuFor !== ""
            sourceComponent: DockMenu { dock: win }
        }
        // Downloads stack
        Loader {
            id: stackLoader
            active: win.stackOpen
            sourceComponent: DownloadsStack { dock: win }
        }
        // Opening any panel closes them
        Connections {
            target: Panels
            function onOpenChanged() {
                win.menuFor = "";
                win.stackOpen = false;
            }
        }
    }
}
