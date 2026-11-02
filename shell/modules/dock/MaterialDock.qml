import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// Material dock: a Pixel tablet's taskbar. A floating pill in
// surfaceContainer, 16 px above the bottom edge, with the same items as the
// Glass dock (Apps, pinned and running apps | recent apps | minimized
// windows, Downloads, Trash). No magnification and no lift: a state layer
// circle behind the hovered icon, a short dash under running apps (primary
// for the app in front), plain tooltips. Menus, the Downloads stack and
// auto-hide work as in Glass. The window has a fixed height; input is masked
// to the pill (and the edge strip while auto-hiding).
Variants {
    model: Config.options.dock.enabled && Theme.material ? Quickshell.screens : []

    PanelWindow {
        id: win
        required property var modelData
        screen: modelData
        visible: Wallpaper.started // after the wallpaper at login
        color: "transparent"
        WlrLayershell.namespace: "irice:dock"
        WlrLayershell.layer: WlrLayer.Top

        readonly property var cfg: Config.options.dock
        // A slot is an icon's place; the icon is drawn a little inside it and
        // its state layer circle a little outside (into the pill's padding)
        readonly property int iconSize: Math.max(36, Math.min(56, cfg.iconSize))
        readonly property int glyph: iconSize - 4
        readonly property int pad: 8
        readonly property int gap: Math.round(iconSize * Math.max(0.05, Math.min(0.4, cfg.gap ?? 0.18))) // Settings → Dock → Space between icons
        readonly property int barHeight: iconSize + 2 * pad // 64 at the default size
        readonly property int bottomGap: 16
        readonly property int dividerWidth: 1 + 2 * gap
        readonly property real maxScale: 1 // (no magnification in Material)

        // ── Items (the same lists as the Glass dock) ───────────────────
        readonly property var mainItems: (cfg.appsButton ? [{ kind: "launcher", id: "Apps" }] : []).concat(Apps.dockApps.map(a => Object.assign({ kind: "app" }, a)))
        readonly property var recentItems: cfg.recentApps ? Apps.recentFor(3).map(a => Object.assign({ kind: "recent" }, a)) : []
        readonly property var endItems: Apps.minimized.map(t => ({ kind: "minimized", id: "minimized:" + t.address, entry: Apps.entryFor(t.lastIpcObject?.class ?? ""), minimized: t })).concat(Config.options.phone.dock && Phone.phone ? [{ kind: "phone", id: "phone:" + Phone.phone.serial, phone: Phone.phone }] : []).concat(cfg.downloads ? [{ kind: "downloads", id: "Downloads" }] : []).concat(cfg.trash ? [{ kind: "trash", id: "Trash" }] : [])
        readonly property var items: {
            const out = mainItems.slice();
            if (recentItems.length > 0)
                out.push(...recentItems.map((x, i) => Object.assign({ divider: i === 0 }, x)));
            if (endItems.length > 0)
                out.push(...endItems.map((x, i) => Object.assign({ divider: i === 0 }, x)));
            return out;
        }
        // The Repeater's model only changes when the set of items does (a new
        // array rebuilds every icon), or when the app list reloaded and the
        // old entries are gone
        property var shownItems: []
        property string shownKey: ""
        onItemsChanged: {
            const key = items.map(i => i.kind + ":" + i.id + (i.divider ? "|" : "")).join(",");
            const stale = items.some((x, i) => x.entry !== shownItems[i]?.entry);
            if (key !== shownKey || stale) {
                shownKey = key;
                shownItems = items;
            }
        }
        Component.onCompleted: itemsChanged()
        readonly property int dividers: items.filter(i => i.divider).length
        readonly property real restWidth: items.length * iconSize + Math.max(0, items.length - 1) * gap + dividers * dividerWidth
        readonly property real pillWidth: restWidth + 2 * pad

        // ── Pointer ─────────────────────────────────────────────────────
        // One passive tracker over the window; hover is its position tested
        // against the pill's resting rectangle, so nothing that moves decides it
        readonly property bool settled: slide < 0.001 // fully shown
        readonly property point pointer: tracker.hovered ? tracker.point.position : Qt.point(-1, -1)
        readonly property bool onEdge: tracker.hovered && pointer.y >= height - 2
        readonly property rect restRect: Qt.rect((width - pillWidth) / 2, height - bottomGap - barHeight, pillWidth, barHeight)
        readonly property rect zoneRect: Qt.rect(restRect.x - 24, restRect.y - 44, restRect.width + 48, barHeight + bottomGap + 44)
        readonly property bool inZone: tracker.hovered && pointer.x >= zoneRect.x && pointer.x < zoneRect.x + zoneRect.width && pointer.y >= zoneRect.y
        readonly property bool hovered: settled && tracker.hovered && pointer.y >= restRect.y && pointer.y < restRect.y + barHeight && pointer.x >= restRect.x && pointer.x < restRect.x + pillWidth
        readonly property real pointerLocal: pointer.x - restRect.x // within the pill
        property string menuFor: "" // item id whose right-click menu is open
        property var menuItem: null
        property real menuX: 0 // screen x the menu points at
        property bool stackOpen: false // Downloads stack
        property real stackX: 0

        // Plain tooltips wait a moment the first time, then follow the
        // pointer from icon to icon without waiting again
        property bool tipsWarm: false
        onHoveredChanged: {
            if (hovered) {
                tipDelay.restart();
            } else {
                tipDelay.stop();
                tipsWarm = false;
            }
        }
        Timer {
            id: tipDelay
            interval: 500
            onTriggered: win.tipsWarm = true
        }

        // ── Auto-hide (as the Glass dock) ───────────────────────────────
        // Hidden: a 2 px strip at the bottom edge takes input; resting the
        // pointer there reveals the dock. Shown: the zone keeps it up; leaving
        // it hides the dock after a short grace.
        readonly property bool autoHiding: cfg.autoHide === "always" || (cfg.autoHide === "windows" && overlapped)
        readonly property bool overlapped: Desktop.covered(screen, Qt.rect((screen.width - pillWidth) / 2, screen.height - barHeight - bottomGap, pillWidth, barHeight + bottomGap))
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
        // Touchscreen: swipe up from the bottom edge; it stays a few seconds
        function touchReveal() {
            revealed = true;
            touchHold.restart();
        }
        Timer {
            id: touchHold
            interval: 4000
            onTriggered: if (!win.wantsUp) win.revealed = false
        }
        // Slides on Material's default spatial spring (a little overshoot
        // coming up)
        property real slide: hidden ? 1 : 0
        Behavior on slide {
            SpringAnimation {
                spring: Theme.motion.springDefault.spring
                damping: Theme.motion.springDefault.damping
                epsilon: 0.002
            }
        }

        exclusionMode: cfg.autoHide === "off" ? ExclusionMode.Normal : ExclusionMode.Ignore
        exclusiveZone: cfg.autoHide === "off" ? barHeight + bottomGap : 0
        anchors { bottom: true; left: true; right: true }
        implicitHeight: barHeight + bottomGap + 48 // room for a tooltip and a bounce above the pill

        mask: Region {
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
                x: win.restRect.x
                y: win.restRect.y
                width: !win.autoHiding ? win.restRect.width : 0
                height: win.restRect.height
            }
        }
        Item {
            anchors.fill: parent
            z: 100
            // Passive: sees the pointer, takes no clicks from the icons
            HoverHandler { id: tracker }
            DragHandler {
                target: null
                acceptedDevices: PointerDevice.TouchScreen
                xAxis.enabled: false
                onTranslationChanged: if (translation.y < -24 && win.autoHiding) win.touchReveal()
            }
        }

        // The pill: surfaceContainer, fully round, no shadow (elevation by tone)
        Rectangle {
            id: dock
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            anchors.bottomMargin: win.bottomGap - (win.barHeight + win.bottomGap + 8) * win.slide
            width: row.width + 2 * win.pad
            height: win.barHeight
            radius: height / 2
            color: Theme.m3.surfaceContainer

            Row {
                id: row
                x: win.pad
                y: win.pad
                spacing: win.gap

                Repeater {
                    model: win.shownItems
                    MaterialDockItem {
                        required property var modelData
                        item: modelData
                        dock: win
                    }
                }
            }
        }

        // Right-click menu and Downloads stack: their own overlay windows, so
        // a click anywhere else closes them
        Loader {
            active: win.menuFor !== ""
            sourceComponent: DockMenu { dock: win }
        }
        Loader {
            active: win.stackOpen
            sourceComponent: MaterialDownloadsStack { dock: win }
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
