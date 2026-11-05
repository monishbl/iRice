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
// (None while tessera-ui draws the dock in Rust: Settings' rust.surfaces)
Variants {
    model: Config.options.dock.enabled && Theme.material && !RustUi.drawing("dock") ? Quickshell.screens : []

    PanelWindow {
        id: win
        required property var modelData
        screen: modelData
        visible: Wallpaper.started // after the wallpaper at login
        color: "transparent"
        WlrLayershell.namespace: "tessera:dock"
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

        // Items, pointer and auto-hide: DockLogic, as the Glass dock
        readonly property var items: logic.items
        readonly property real restWidth: items.length * iconSize + Math.max(0, items.length - 1) * gap + logic.dividers * dividerWidth
        readonly property real pillWidth: restWidth + 2 * pad

        // ── Pointer ─────────────────────────────────────────────────────
        // Hover is the pointer tested against the pill's resting rectangle,
        // so nothing that moves decides it
        readonly property bool settled: slide < 0.001 // fully shown
        readonly property point pointer: logic.pointer
        readonly property rect restRect: Qt.rect((width - pillWidth) / 2, height - bottomGap - barHeight, pillWidth, barHeight)
        readonly property rect zoneRect: Qt.rect(restRect.x - 24, restRect.y - 44, restRect.width + 48, barHeight + bottomGap + 44)
        readonly property bool hovered: settled && logic.tracking && pointer.y >= restRect.y && pointer.y < restRect.y + barHeight && pointer.x >= restRect.x && pointer.x < restRect.x + pillWidth
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

        // Auto-hide: DockLogic. Slides on Material's default spatial spring
        // (a little overshoot coming up)
        readonly property bool hidden: logic.hidden
        property real slide: hidden ? 1 : 0
        Behavior on slide {
            MaterialSpring { epsilon: 0.002 }
        }

        exclusionMode: cfg.autoHide === "off" ? ExclusionMode.Normal : ExclusionMode.Ignore
        exclusiveZone: cfg.autoHide === "off" ? barHeight + bottomGap : 0
        anchors { bottom: true; left: true; right: true }
        implicitHeight: barHeight + bottomGap + 48 // room for a tooltip and a bounce above the pill

        mask: logic.mask
        DockLogic {
            id: logic
            window: win
            zoneRect: win.zoneRect
            pillRect: win.restRect
            coverRect: Qt.rect((win.screen.width - win.pillWidth) / 2, win.screen.height - win.barHeight - win.bottomGap, win.pillWidth, win.barHeight + win.bottomGap)
        }

        // The pill: surfaceContainer, fully round, no shadow (elevation by tone)
        Rectangle {
            id: dock
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            anchors.bottomMargin: win.bottomGap - (win.barHeight + win.bottomGap + 8) * win.slide - 20 * LockFx.settle // (and the login intro: settling in)
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
                    model: logic.shownItems
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
    }
}
