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
// (None while tessera-ui draws the dock in Rust: Settings' rust.surfaces)
Variants {
    model: Config.options.dock.enabled && !Theme.material && !RustUi.drawing("dock") ? Quickshell.screens : []

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

        // Items, pointer and auto-hide: DockLogic
        readonly property var items: logic.items
        readonly property real restWidth: items.length * iconSize + (items.length - 1) * gap + logic.dividers * dividerWidth

        // ── Pointer ─────────────────────────────────────────────────────
        readonly property bool settled: slide < 0.001 // fully shown
        readonly property point pointer: logic.pointer
        // On the pill (or the icons above it): drives magnification, labels, lift
        readonly property bool hovered: settled && logic.tracking && pointer.y >= height - bottomGap - barHeight - iconSize * (maxScale - 1) - 12 && pointer.y < height - bottomGap && pointer.x >= dock.x && pointer.x < dock.x + dock.width
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

        // Auto-hide: DockLogic
        readonly property bool hidden: logic.hidden
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

        mask: logic.mask
        DockLogic {
            id: logic
            window: win
            zoneRect: win.zoneRect
            pillRect: win.pillRect
            coverRect: Qt.rect((win.screen.width - win.restWidth) / 2 - win.pad, win.screen.height - win.barHeight - win.bottomGap, win.restWidth + 2 * win.pad, win.barHeight + win.bottomGap)
        }

        // The pill; its width follows the icons (and magnification)
        Item {
            id: dock
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            // Slides all the way out of the window (lifted icons included)
            anchors.bottomMargin: win.bottomGap - (win.barHeight + win.iconSize * (win.maxScale - 1) + win.bottomGap + 24) * win.slide - 20 * LockFx.settle // (and the login intro: settling in)
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
                    model: logic.shownItems
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
    }
}
