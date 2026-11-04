import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.modules.menubar
import qs.services

// Volume / brightness HUD: a small glass capsule under the menu bar, shown for
// a moment whenever either changes (or the island's HUD row, when it's set
// to show there). Over a fullscreen app (a video, a game) it's only a thin
// bar along the top edge instead. Also owns the media-key shortcuts.
// The Material look draws Android's volume panel instead of the capsule
// (MaterialVolumePanel: the same state and actions, at the right edge).
Scope {
    id: root

    property string kind: "volume" // volume | brightness | mic
    property bool shown: false
    property bool armed: false // ignore the initial values at startup

    readonly property real value: kind === "brightness" ? Brightness.value : kind === "mic" ? (Audio.micMuted ? 0 : Audio.micVolume) : (Audio.muted ? 0 : Audio.volume)
    readonly property string icon: kind === "brightness" ? "sun" : kind === "mic" ? (Audio.micMuted ? "microphone-slash" : "microphone") : Audio.icon
    // (just after switching to another output or microphone: its name)
    property string device: ""
    readonly property string label: device || (kind === "brightness" ? "Display" : kind === "mic" ? (Audio.micMuted ? "Microphone off" : "Microphone on") : "Sound")

    Binding {
        target: Panels
        property: "osdVisible"
        value: root.shown && !Theme.material // (Material's panel is at the right edge, clear of the banners)
    }

    function show(k, dev) {
        if (!armed)
            return;
        // (set with Tessera's own sliders: you're looking at the control)
        if (!dev && Date.now() < (k === "brightness" ? Brightness.quietUntil : Audio.quietUntil))
            return;
        device = dev ?? "";
        // A fullscreen window on the focused display: the thin top bar
        if (Hyprland.focusedMonitor?.activeWorkspace?.hasFullscreen ?? false) {
            barKind = k;
            barShown = true;
            barHide.restart();
            return;
        }
        // (Material: Android's volume panel at the screen's edge, not the island)
        if (Config.options.island.enabled && Config.options.island.hud && !Theme.material) {
            Island.hud(k, dev);
            return;
        }
        kind = k;
        shown = true;
        hideTimer.interval = device ? 2500 : 1600;
        hideTimer.restart();
    }

    Timer {
        id: switchShow
        property string kind: "volume"
        interval: 700
        onTriggered: root.show(kind, Audio.nameOf(kind === "mic" ? Audio.source : Audio.sink))
    }
    Timer { interval: 2000; running: true; onTriggered: root.armed = true }
    Timer { id: hideTimer; interval: 1600; onTriggered: root.shown = false }
    // The capsule takes the pointer: resting on it keeps it up; click, drag
    // or scroll its level; its icon mutes; its button opens the outputs,
    // inputs or displays underneath (IslandLevels, in the theme's colours)
    property bool levelsOpen: false
    property bool pointerOn: false
    onPointerOnChanged: pointerOn ? hideTimer.stop() : hideTimer.restart()
    onShownChanged: if (!shown) levelsOpen = false
    function setLevel(v) {
        v = Math.max(0, Math.min(1, v));
        if (kind === "brightness")
            Brightness.set(v, true);
        else if (kind === "mic")
            Audio.setMicVolume(v, true);
        else
            Audio.setVolume(v, true);
    }

    // Another output or microphone (headphones connecting): its name and
    // level, once they've arrived (its level and mute arriving aren't changes
    // of their own; with both switching, the output's is shown)
    property var sinkSeen: null
    property var sourceSeen: null
    property real switchedAt: 0
    function switched() {
        if (Audio.sink !== sinkSeen || Audio.source !== sourceSeen) {
            sinkSeen = Audio.sink;
            sourceSeen = Audio.source;
            switchedAt = Date.now();
        }
        return Date.now() - switchedAt < 1500;
    }
    Connections {
        target: Audio
        function onSinkChanged() {
            if (root.switched() && Audio.sink) {
                switchShow.kind = "volume";
                switchShow.restart();
            }
        }
        function onSourceChanged() {
            if (root.switched() && Audio.source && !(switchShow.running && switchShow.kind === "volume")) {
                switchShow.kind = "mic";
                switchShow.restart();
            }
        }
        function onVolumeChanged() { if (!root.switched()) root.show("volume"); }
        function onMutedChanged() { if (!root.switched()) root.show("volume"); }
        function onMicMutedChanged() { if (!root.switched()) root.show("mic"); }
    }
    Connections {
        target: Brightness
        function onValueChanged() { root.show("brightness"); }
    }

    // Exists only while visible (plus time for the exit animation)
    property bool windowAlive: false
    readonly property bool windowWanted: root.shown
    onWindowWantedChanged: windowWanted ? (windowAlive = true, windowUnload.stop()) : windowUnload.restart()
    Timer { id: windowUnload; interval: 600; onTriggered: root.windowAlive = false }

    LazyLoader {
        active: root.windowAlive && !Theme.material

        GlassWindow {
            id: win
            readonly property int m: 24
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => root.shown ? 1 : 0)
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: root.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            name: "osd"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            visible: progress > 0
            WlrLayershell.layer: WlrLayer.Overlay
            anchors { top: true; right: true }
            margins.top: 30 + 6 - m
            margins.right: 8 - m
            implicitWidth: 280 + 2 * m
            implicitHeight: 52 + (root.levelsOpen ? capLevels.implicitHeight + 8 : 0) + 2 * m
            mask: Region { item: hit }

            Item {
                id: hit
                anchors.fill: parent
                anchors.margins: win.m
            }
            // (on the window itself: the button's and the level's own
            // pointer areas would hide the pointer from a sibling)
            HoverHandler { onHoveredChanged: root.pointerOn = hovered }

            Glass {
                id: glass
                anchors.fill: parent
                radius: 26
                bevel: 18
                shadowMargin: win.m
                opacity: win.progress
                scale: 0.94 + 0.06 * win.progress
            }

            Row {
                x: win.m + 16
                y: win.m + (52 - height) / 2
                spacing: 12
                opacity: win.progress

                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: root.icon
                    fill: true
                    size: 20
                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -6
                        enabled: root.kind !== "brightness"
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.kind === "mic" ? Audio.toggleMic(true) : Audio.toggleMute(true)
                    }
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 5
                    Text {
                        width: Math.min(implicitWidth, 180)
                        elide: Text.ElideRight
                        text: root.label
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                    }
                    Rectangle {
                        id: capBar
                        width: 180
                        height: 6
                        radius: 3
                        color: Theme.color.toggleOff
                        Rectangle {
                            width: parent.width * Math.max(0, Math.min(1, root.value))
                            height: parent.height
                            radius: 3
                            color: Theme.color.label
                            Behavior on width {
                                enabled: !capBarMouse.pressed
                                NumberAnimation { duration: Theme.motion.hover }
                            }
                        }
                        MouseArea {
                            id: capBarMouse
                            anchors.fill: parent
                            anchors.margins: -10
                            cursorShape: Qt.PointingHandCursor
                            preventStealing: true
                            onPressed: e => root.setLevel((e.x - 10) / capBar.width)
                            onPositionChanged: e => {
                                if (pressed)
                                    root.setLevel((e.x - 10) / capBar.width);
                            }
                        }
                        WheelHandler {
                            target: null
                            onWheel: e => root.setLevel(root.value + (e.angleDelta.y > 0 ? 0.05 : -0.05))
                        }
                    }
                }
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 28
                    height: 28
                    radius: 14
                    color: capButton.pressed ? Theme.color.fillPressed : capButton.containsMouse || root.levelsOpen ? Theme.color.fill : "transparent"
                    Accessible.role: Accessible.Button
                    Accessible.name: root.kind === "brightness" ? "Displays" : root.kind === "mic" ? "Inputs" : "Outputs"
                    Icon {
                        anchors.centerIn: parent
                        name: root.levelsOpen ? "caret-up" : "caret-down"
                        bold: true
                        size: 13
                        color: Theme.color.label
                    }
                    MouseArea {
                        id: capButton
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.levelsOpen = !root.levelsOpen
                    }
                }
            }
            IslandLevels {
                id: capLevels
                visible: root.levelsOpen
                x: win.m + 8
                y: win.m + 52
                width: 280 - 16
                kind: root.kind
                ink: Theme.color.label
                opacity: win.progress
                onVisibleChanged: if (!visible) page = "list"
            }
        }
    }

    LazyLoader {
        active: root.windowAlive && Theme.material

        MaterialVolumePanel {
            osd: root
        }
    }

    // ── Over a fullscreen app: a thin bar centred along the top edge ──────
    // (a 5 px level on a faint track, a small icon before it); fades in, and
    // out after 1.2 s without a change
    property string barKind: "volume"
    property bool barShown: false
    Timer { id: barHide; interval: 1200; onTriggered: root.barShown = false }
    readonly property real barValue: barKind === "brightness" ? Brightness.value : barKind === "mic" ? (Audio.micMuted ? 0 : Audio.micVolume) : (Audio.muted ? 0 : Audio.volume)
    readonly property string barIcon: barKind === "brightness" ? "sun" : barKind === "mic" ? (Audio.micMuted ? "microphone-slash" : "microphone") : Audio.icon
    property bool barAlive: false
    onBarShownChanged: barShown ? (barAlive = true, barUnload.stop()) : barUnload.restart()
    Timer { id: barUnload; interval: 400; onTriggered: root.barAlive = false }

    LazyLoader {
        active: root.barAlive

        PanelWindow {
            id: fsBar
            property real fade: 0
            Component.onCompleted: fade = Qt.binding(() => root.barShown ? 1 : 0)
            Behavior on fade {
                NumberAnimation { duration: root.barShown ? 150 : 100; easing.type: root.barShown ? Easing.OutCubic : Easing.InCubic }
            }
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            visible: fade > 0
            WlrLayershell.namespace: "tessera:fshud"
            WlrLayershell.layer: WlrLayer.Overlay
            exclusionMode: ExclusionMode.Ignore
            color: "transparent"
            anchors { top: true }
            implicitWidth: Math.round((screen?.width ?? 1920) * 0.3) + 32
            implicitHeight: 24
            mask: Region {}

            Row {
                id: barRow
                anchors.horizontalCenter: parent.horizontalCenter
                y: 6 - (Config.options.appearance.reduceMotion === true ? 0 : Math.round((1 - fsBar.fade) * 4))
                spacing: 8
                opacity: fsBar.fade
                // A soft dark halo so it reads over bright video too
                layer.enabled: true
                layer.effect: MultiEffect {
                    shadowEnabled: true
                    shadowColor: Qt.rgba(0, 0, 0, 0.55)
                    shadowBlur: 0.35
                    shadowVerticalOffset: 0
                    shadowHorizontalOffset: 0
                }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: root.barIcon
                    fill: true
                    size: 12
                    color: Theme.material ? Theme.m3.primaryFixedDim : "#F5F5F7"
                }
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.round((fsBar.screen?.width ?? 1920) * 0.3) - 20
                    height: 5
                    radius: 2.5
                    // (Material: its fixed primary tones, which read the same over
                    // any video in light or dark mode)
                    color: Theme.material ? Qt.alpha(Theme.m3.scrim, 0.35) : Qt.rgba(0, 0, 0, 0.35)
                    border.width: 1
                    border.color: Theme.material ? Qt.alpha(Theme.m3.primaryFixed, 0.22) : Qt.rgba(1, 1, 1, 0.22)
                    Rectangle {
                        width: Math.max(parent.height, parent.width * Math.max(0, Math.min(1, root.barValue)))
                        visible: root.barValue > 0
                        height: parent.height
                        radius: 2.5
                        color: Theme.material ? Theme.m3.primaryFixedDim : "#F5F5F7"
                        Behavior on width { NumberAnimation { duration: 100; easing.type: Easing.OutCubic } }
                    }
                }
            }
            Accessible.role: Accessible.ProgressBar
            Accessible.name: root.barKind === "brightness" ? "Brightness" : root.barKind === "mic" ? "Microphone" : "Volume"
        }
    }

    // Shows the HUD without changing anything: qs ipc call osd preview volume
    IpcHandler {
        target: "osd"
        function preview(kind: string): void {
            root.armed = true;
            root.show(kind);
        }
    }

    // Media keys: bind XF86 keys to hl.dsp.global("tessera:<name>")
    GlobalShortcut { appid: "tessera"; name: "volumeUp"; onPressed: { Audio.setVolume(Audio.volume + 0.05); Sounds.play("volumeKeys"); } }
    GlobalShortcut { appid: "tessera"; name: "volumeDown"; onPressed: { Audio.setVolume(Audio.volume - 0.05); Sounds.play("volumeKeys"); } }
    GlobalShortcut { appid: "tessera"; name: "volumeMute"; onPressed: Audio.toggleMute() }
    GlobalShortcut { appid: "tessera"; name: "micMute"; onPressed: Audio.toggleMic() }
    GlobalShortcut { appid: "tessera"; name: "brightnessUp"; onPressed: if (Brightness.available) Brightness.set(Brightness.value + 0.05) } // (no backlight: no OSD either)
    GlobalShortcut { appid: "tessera"; name: "brightnessDown"; onPressed: if (Brightness.available) Brightness.set(Brightness.value - 0.05) }
}
