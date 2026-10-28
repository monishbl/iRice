import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Today, the morning screen (services/Today.qml decides when): the screen
// frosts over and a glass panel the size of the Settings window holds the
// day — a greeting, today's events and to-dos, the weather. Three entrances
// (Settings → Desktop & Widgets → Today):
//   bloom   the panel condenses out of the frost: it settles from a touch
//           larger, its blur clearing, the rows rising in one after another
//   shrink  a full-screen glass sheet draws in to the panel
//   drop    a drop falls from the top, lands with a soft ripple and spreads
//           into the panel
// Esc or Close puts it away for the day; Pin for today keeps it as a widget.
Scope {
    id: root

    // Kept alive through the exit animation
    property bool alive: false
    property var where: null // the display it opened on (it stays there)
    Connections {
        target: Today
        function onOpenChanged() {
            if (Today.open && !root.alive) {
                root.where = Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
                root.alive = true;
            }
        }
    }

    LazyLoader {
        active: root.alive

        GlassWindow {
            id: win
            name: "today"
            // The display you're looking at
            screen: root.where
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: Today.open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            color: "transparent"

            readonly property string fx: Today.effect
            readonly property bool still: Config.options.appearance.reduceMotion === true
            readonly property real ms: CoolDown.motionScale

            // Where the panel ends up: the Settings window's size, centred
            readonly property real tw: Math.min(860, width - 64)
            readonly property real th: Math.min(620, height - 96)
            readonly property real tx: Math.round((width - tw) / 2)
            readonly property real ty: Math.round((height - th) / 2)
            readonly property real tr: Theme.radius.panel + 8

            // Animated: the frost behind, the panel's rectangle and look, the
            // content (blur and the staggered rows)
            property real frost: 0
            property real px: tx
            property real py: ty
            property real pw: tw
            property real ph: th
            property real pr: tr
            property real panelOpacity: 0
            property real contentBlur: 0
            property real rows: 0 // 0 → 1: rows rise in one after another
            property real ripple: 0 // the drop's landing ring, 0 → 1

            function begin() {
                if (still || fx === "bloom") {
                    pw = tw * (still ? 1 : 1.06);
                    ph = th * (still ? 1 : 1.06);
                    px = tx - (pw - tw) / 2;
                    py = ty - (ph - th) / 2;
                    pr = tr;
                    contentBlur = still ? 0 : 1;
                    (still ? enterStill : enterBloom).start();
                } else if (fx === "shrink") {
                    px = 0;
                    py = 0;
                    pw = width;
                    ph = height;
                    pr = 0;
                    panelOpacity = 1;
                    enterShrink.start();
                } else {
                    const d = 44;
                    pw = d;
                    ph = d;
                    pr = d / 2;
                    px = width / 2 - d / 2;
                    py = -d;
                    panelOpacity = 1;
                    enterDrop.start();
                }
            }
            Component.onCompleted: begin()

            // ── Entrances ───────────────────────────────────────────────
            ParallelAnimation {
                id: enterStill
                NumberAnimation { target: win; property: "frost"; to: 1; duration: 200 }
                NumberAnimation { target: win; property: "panelOpacity"; to: 1; duration: 200 }
                NumberAnimation { target: win; property: "rows"; to: 1; duration: 200 }
            }
            ParallelAnimation {
                id: enterBloom
                NumberAnimation { target: win; property: "frost"; to: 1; duration: 380 * win.ms; easing.type: Easing.OutCubic }
                SequentialAnimation {
                    PauseAnimation { duration: 140 * win.ms }
                    ParallelAnimation {
                        NumberAnimation { target: win; property: "panelOpacity"; to: 1; duration: 320 * win.ms; easing.type: Easing.OutCubic }
                        NumberAnimation { target: win; properties: "pw"; to: win.tw; duration: 520 * win.ms; easing.type: Easing.OutCubic }
                        NumberAnimation { target: win; properties: "ph"; to: win.th; duration: 520 * win.ms; easing.type: Easing.OutCubic }
                        NumberAnimation { target: win; properties: "px"; to: win.tx; duration: 520 * win.ms; easing.type: Easing.OutCubic }
                        NumberAnimation { target: win; properties: "py"; to: win.ty; duration: 520 * win.ms; easing.type: Easing.OutCubic }
                        NumberAnimation { target: win; property: "contentBlur"; to: 0; duration: 520 * win.ms; easing.type: Easing.OutCubic }
                        SequentialAnimation {
                            PauseAnimation { duration: 120 * win.ms }
                            NumberAnimation { target: win; property: "rows"; to: 1; duration: 620 * win.ms; easing.type: Easing.Linear }
                        }
                    }
                }
            }
            ParallelAnimation {
                id: enterShrink
                NumberAnimation { target: win; property: "frost"; to: 1; duration: 260 * win.ms; easing.type: Easing.OutCubic }
                NumberAnimation { target: win; property: "pw"; to: win.tw; duration: 640 * win.ms; easing.type: Easing.InOutCubic }
                NumberAnimation { target: win; property: "ph"; to: win.th; duration: 640 * win.ms; easing.type: Easing.InOutCubic }
                NumberAnimation { target: win; property: "px"; to: win.tx; duration: 640 * win.ms; easing.type: Easing.InOutCubic }
                NumberAnimation { target: win; property: "py"; to: win.ty; duration: 640 * win.ms; easing.type: Easing.InOutCubic }
                NumberAnimation { target: win; property: "pr"; to: win.tr; duration: 640 * win.ms; easing.type: Easing.InOutCubic }
                SequentialAnimation {
                    PauseAnimation { duration: 420 * win.ms }
                    NumberAnimation { target: win; property: "rows"; to: 1; duration: 620 * win.ms; easing.type: Easing.Linear }
                }
            }
            SequentialAnimation {
                id: enterDrop
                ParallelAnimation {
                    NumberAnimation { target: win; property: "frost"; to: 1; duration: 420 * win.ms; easing.type: Easing.OutCubic }
                    // falls, gathering speed
                    NumberAnimation { target: win; property: "py"; to: win.height / 2 - 22; duration: 420 * win.ms; easing.type: Easing.InQuad }
                }
                // lands: a soft ring spreads while the drop opens into the panel
                ParallelAnimation {
                    NumberAnimation { target: win; property: "ripple"; from: 0; to: 1; duration: 700 * win.ms; easing.type: Easing.OutCubic }
                    NumberAnimation { target: win; property: "pw"; to: win.tw; duration: 560 * win.ms; easing.type: Easing.OutCubic }
                    NumberAnimation { target: win; property: "ph"; to: win.th; duration: 560 * win.ms; easing.type: Easing.OutCubic }
                    NumberAnimation { target: win; property: "px"; to: win.tx; duration: 560 * win.ms; easing.type: Easing.OutCubic }
                    NumberAnimation { target: win; property: "py"; to: win.ty; duration: 560 * win.ms; easing.type: Easing.OutCubic }
                    NumberAnimation { target: win; property: "pr"; to: win.tr; duration: 560 * win.ms; easing.type: Easing.OutCubic }
                    SequentialAnimation {
                        PauseAnimation { duration: 300 * win.ms }
                        NumberAnimation { target: win; property: "rows"; to: 1; duration: 620 * win.ms; easing.type: Easing.Linear }
                    }
                }
            }

            // ── Exit (any entrance): fades and settles a little smaller ──
            Connections {
                target: Today
                function onOpenChanged() {
                    if (!Today.open) {
                        enterStill.stop();
                        enterBloom.stop();
                        enterShrink.stop();
                        enterDrop.stop();
                        exit.start();
                    }
                }
            }
            SequentialAnimation {
                id: exit
                ParallelAnimation {
                    NumberAnimation { target: win; property: "rows"; to: 0; duration: 140 * win.ms; easing.type: Easing.InCubic }
                    NumberAnimation { target: win; property: "panelOpacity"; to: 0; duration: 240 * win.ms; easing.type: Easing.InCubic }
                    NumberAnimation { target: win; property: "pw"; to: win.tw * 0.97; duration: 240 * win.ms; easing.type: Easing.InCubic }
                    NumberAnimation { target: win; property: "ph"; to: win.th * 0.97; duration: 240 * win.ms; easing.type: Easing.InCubic }
                    NumberAnimation { target: win; property: "px"; to: win.tx + win.tw * 0.015; duration: 240 * win.ms; easing.type: Easing.InCubic }
                    NumberAnimation { target: win; property: "py"; to: win.ty + win.th * 0.015; duration: 240 * win.ms; easing.type: Easing.InCubic }
                    NumberAnimation { target: win; property: "frost"; to: 0; duration: 280 * win.ms; easing.type: Easing.InCubic }
                }
                ScriptAction { script: root.alive = false }
            }

            // ── The frost behind: your wallpaper, blurred, over everything ──
            Item {
                anchors.fill: parent
                opacity: win.frost
                Image {
                    anchors.fill: parent
                    source: Wallpaper.backdropUrl(win.screen?.name ?? "default")
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    smooth: true
                }
                Rectangle {
                    anchors.fill: parent
                    color: Theme.dark ? Qt.rgba(0, 0, 0, 0.42) : Qt.rgba(1, 1, 1, 0.3)
                }
                // Clicks outside the panel do nothing (Esc or Close put it away)
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.AllButtons
                    onWheel: wheel => wheel.accepted = true
                }
            }

            // The drop's landing: a soft ring spreading from where it lands
            Rectangle {
                visible: win.fx === "drop" && win.ripple > 0 && win.ripple < 1
                readonly property real d: 44 + win.ripple * Math.min(win.width, win.height) * 0.9
                x: win.width / 2 - d / 2
                y: win.height / 2 - d / 2
                width: d
                height: d
                radius: d / 2
                color: "transparent"
                border.width: 1.5 + 10 * (1 - win.ripple)
                border.color: Qt.rgba(1, 1, 1, 0.22 * (1 - win.ripple))
                layer.enabled: true
                layer.effect: MultiEffect { blurEnabled: true; blur: 0.6; blurMax: 24 }
            }

            // ── The panel ───────────────────────────────────────────────
            Item {
                id: panel
                x: win.px
                y: win.py
                width: win.pw
                height: win.ph
                opacity: win.panelOpacity
                onXChanged: glass.updatePosition()
                onYChanged: glass.updatePosition()

                Glass {
                    id: glass
                    anchors.fill: parent
                    anchors.margins: -shadowMargin
                    radius: win.pr
                }
                // Glass over an already-frosted backdrop reads as nothing: a
                // soft surface and a hairline make the panel its own layer
                Rectangle {
                    anchors.fill: parent
                    radius: win.pr
                    color: Theme.dark ? Qt.rgba(0.07, 0.07, 0.08, 0.42) : Qt.rgba(1, 1, 1, 0.45)
                    border.width: 1
                    border.color: Theme.dark ? Qt.rgba(1, 1, 1, 0.1) : Qt.rgba(0, 0, 0, 0.08)
                }

                Item {
                    id: contentClip
                    anchors.fill: parent
                    clip: true
                    // Laid out at the final size, so nothing reflows as the panel grows
                    TodayContent {
                        id: content
                        x: (parent.width - win.tw) / 2
                        y: (parent.height - win.th) / 2
                        width: win.tw
                        height: win.th
                        rows: win.rows
                        focus: true
                        layer.enabled: win.contentBlur > 0.01
                        layer.effect: MultiEffect {
                            blurEnabled: true
                            blur: win.contentBlur
                            blurMax: 32
                        }
                    }
                }
            }
        }
    }
}
