import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.modules.menubar
import qs.services

// The Material HUD, as Android 16's volume panel: a pill at the right edge,
// vertically centred, that springs in from the edge. In it, top to bottom: a
// mute toggle (volume and microphone), a tall Expressive slider (its filled
// part primary with the symbol inside at the bottom, a thin handle with a
// gap each side, a stop dot at the top) and a button that opens the outputs,
// inputs or displays (IslandLevels) in a card beside it. A device that just
// arrived, or the microphone's state, shows in a toast pill to its left.
// The same behaviour as the Glass capsule: resting on it keeps it up; click,
// drag or scroll the level. Osd.qml owns the state; this is only the look.
PanelWindow {
    id: win

    required property var osd

    // Sizes: Material's XL-ish vertical slider, at a laptop's scale
    readonly property int trackWidth: 44
    readonly property int trackHeight: 200
    readonly property int pad: 8
    readonly property int button: 44
    readonly property int edge: 16 // from the screen's right edge
    readonly property int cardWidth: 296
    readonly property bool hasToggle: osd.kind !== "brightness"
    readonly property int panelWidth: trackWidth + 2 * pad
    readonly property int panelHeight: pad + (hasToggle ? button + pad : 0) + trackHeight + pad + button + pad
    // Resting place of the panel (window coordinates)
    readonly property real panelX: width - edge - panelWidth
    readonly property real panelY: Math.round((height - panelHeight) / 2)

    screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "irice:osd"
    WlrLayershell.layer: WlrLayer.Overlay
    anchors { top: true; bottom: true; right: true }
    implicitWidth: edge + panelWidth + 12 + cardWidth + 24

    // In on the expressive default spring (overshooting a little past its
    // place), out faster on the emphasized accelerate curve
    property real progress: 0
    readonly property bool shown: osd.shown
    onShownChanged: shown ? enter() : leave()
    Component.onCompleted: if (shown) enter()
    function enter() {
        exitAnim.stop();
        enterAnim.start();
    }
    function leave() {
        enterAnim.stop();
        exitAnim.start();
    }
    SpringAnimation {
        id: enterAnim
        target: win
        property: "progress"
        to: 1
        spring: Theme.motion.springDefault.spring
        damping: Theme.motion.springDefault.damping
        epsilon: 0.002
    }
    NumberAnimation {
        id: exitAnim
        target: win
        property: "progress"
        to: 0
        duration: Math.round(200 * CoolDown.motionScale)
        easing.type: Easing.BezierSpline
        easing.bezierCurve: Theme.motion.exit
    }

    // Input only where the panel (and the open card) rest
    mask: Region {
        Region {
            x: win.panelX
            y: win.panelY
            width: win.shown ? win.panelWidth : 0
            height: win.panelHeight
        }
        Region {
            x: card.x
            y: card.y
            width: win.shown && win.osd.levelsOpen ? card.width : 0
            height: card.height
        }
    }
    // (on the window itself: the controls' own pointer areas would hide the
    // pointer from a sibling)
    HoverHandler { onHoveredChanged: win.osd.pointerOn = hovered }

    // ── The panel ───────────────────────────────────────────────────────
    Item {
        id: panel
        x: win.panelX + (1 - win.progress) * (win.panelWidth + win.edge + 16)
        y: win.panelY
        width: win.panelWidth
        height: win.panelHeight
        opacity: Math.max(0, Math.min(1, win.progress * 2))

        // Elevation 2: it floats over windows (blurred, it's see-through:
        // no shadow under it, as Android 17's)
        RectangularShadow {
            visible: !Theme.blur
            anchors.fill: parent
            radius: width / 2
            offset.y: 2
            blur: 12
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.4 : 0.16)
        }
        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: Theme.veil(Theme.m3.surfaceContainer)
        }

        Column {
            id: col
            x: win.pad
            y: win.pad
            spacing: win.pad

            // Mute: round when on, a squarer tertiary container when muted
            // (Material's selected toggle)
            Rectangle {
                id: toggle
                readonly property bool selected: win.osd.kind === "mic" ? Audio.micMuted : Audio.muted
                visible: win.hasToggle
                width: win.button
                height: win.button
                radius: selected ? 12 : width / 2
                Behavior on radius {
                    SpringAnimation {
                        spring: Theme.motion.springFast.spring
                        damping: Theme.motion.springFast.damping
                        epsilon: 0.05
                    }
                }
                color: Qt.tint(selected ? Theme.m3.tertiaryContainer : Theme.m3.surfaceContainerHighest, Qt.alpha(selected ? Theme.m3.onTertiaryContainer : Theme.m3.onSurfaceVariant, toggleMouse.pressed ? 0.1 : toggleMouse.containsMouse ? 0.08 : 0))
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                Accessible.role: Accessible.Button
                Accessible.name: win.osd.kind === "mic" ? (selected ? "Turn microphone on" : "Turn microphone off") : (selected ? "Unmute" : "Mute")
                Accessible.onPressAction: win.osd.kind === "mic" ? Audio.toggleMic(true) : Audio.toggleMute(true)
                Icon {
                    anchors.centerIn: parent
                    name: win.osd.kind === "mic" ? (toggle.selected ? "microphone-slash" : "microphone") : (toggle.selected ? "speaker-x" : "speaker-high")
                    fill: toggle.selected
                    size: 20
                    color: toggle.selected ? Theme.m3.onTertiaryContainer : Theme.m3.onSurfaceVariant
                }
                MouseArea {
                    id: toggleMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: win.osd.kind === "mic" ? Audio.toggleMic(true) : Audio.toggleMute(true)
                }
            }

            // The slider: inactive part on top (secondaryContainer, a stop
            // dot at its end), the handle, the filled part below (primary)
            Item {
                id: slider
                width: win.trackWidth
                height: win.trackHeight
                readonly property real ratio: Math.max(0, Math.min(1, win.osd.value))
                // (follows the level, quickly; straight under the pointer)
                property real shownRatio: ratio
                Behavior on shownRatio {
                    enabled: !area.pressed
                    NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.standard }
                }
                readonly property real outer: width / 2
                readonly property real inner: 4
                readonly property real gap: 6
                property real thick: area.pressed ? 2 : 4 // the handle: thinner while held
                Behavior on thick { NumberAnimation { duration: Theme.motion.press } }
                readonly property real hy: (1 - shownRatio) * (height - thick) // the handle's top
                readonly property real activeTop: hy + thick + gap
                readonly property real activeHeight: Math.max(0, height - activeTop)
                // The symbol sits inside the filled part while it fits, else
                // just above the handle (Material's inset icon)
                readonly property bool iconInside: activeHeight >= 24 + 20

                Accessible.role: Accessible.Slider
                Accessible.name: win.osd.label
                Accessible.description: Math.round(ratio * 100) + "%"
                Accessible.onIncreaseAction: win.osd.setLevel(ratio + 0.05)
                Accessible.onDecreaseAction: win.osd.setLevel(ratio - 0.05)

                Rectangle {
                    visible: height > 0
                    width: parent.width
                    height: Math.max(0, slider.hy - slider.gap)
                    topLeftRadius: Math.min(slider.outer, Math.max(0, height - slider.inner))
                    topRightRadius: topLeftRadius
                    bottomLeftRadius: Math.min(slider.inner, height / 2)
                    bottomRightRadius: bottomLeftRadius
                    color: Theme.m3.secondaryContainer
                    Rectangle {
                        visible: parent.height > 20
                        anchors.horizontalCenter: parent.horizontalCenter
                        y: 8
                        width: 4
                        height: 4
                        radius: 2
                        color: Theme.m3.primary
                    }
                }
                Rectangle {
                    visible: height > 0
                    y: slider.activeTop
                    width: parent.width
                    height: slider.activeHeight
                    topLeftRadius: Math.min(slider.inner, height / 2)
                    topRightRadius: topLeftRadius
                    bottomLeftRadius: Math.min(slider.outer, Math.max(0, height - slider.inner))
                    bottomRightRadius: bottomLeftRadius
                    color: Theme.m3.primary
                }
                // The handle, a little wider than the track
                Rectangle {
                    x: -4
                    y: slider.hy
                    width: slider.width + 8
                    height: slider.thick
                    radius: height / 2
                    color: Theme.m3.primary
                }
                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: slider.iconInside ? slider.height - 12 - height : slider.hy - slider.gap - 8 - height
                    visible: slider.iconInside || y >= 0
                    name: win.osd.icon
                    fill: true
                    size: 20
                    color: slider.iconInside ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer
                }

                MouseArea {
                    id: area
                    anchors.fill: parent
                    anchors.topMargin: -win.pad
                    anchors.bottomMargin: -win.pad
                    anchors.leftMargin: -win.pad
                    anchors.rightMargin: -win.pad
                    cursorShape: Qt.PointingHandCursor
                    preventStealing: true
                    function set(y) {
                        win.osd.setLevel(1 - (y - win.pad - slider.thick / 2) / (slider.height - slider.thick));
                    }
                    onPressed: e => set(e.y)
                    onPositionChanged: e => {
                        if (pressed)
                            set(e.y);
                    }
                }
                WheelHandler {
                    target: null
                    onWheel: e => win.osd.setLevel(win.osd.value + (e.angleDelta.y > 0 ? 0.05 : -0.05))
                }
            }

            // Outputs, inputs or displays
            Rectangle {
                id: more
                readonly property bool selected: win.osd.levelsOpen
                width: win.button
                height: win.button
                radius: selected ? 12 : width / 2
                Behavior on radius {
                    SpringAnimation {
                        spring: Theme.motion.springFast.spring
                        damping: Theme.motion.springFast.damping
                        epsilon: 0.05
                    }
                }
                color: Qt.tint(selected ? Theme.m3.secondaryContainer : Theme.m3.surfaceContainerHighest, Qt.alpha(selected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurfaceVariant, moreMouse.pressed ? 0.1 : moreMouse.containsMouse ? 0.08 : 0))
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                Accessible.role: Accessible.Button
                Accessible.name: win.osd.kind === "brightness" ? "Displays" : win.osd.kind === "mic" ? "Inputs" : "Outputs"
                Accessible.onPressAction: win.osd.levelsOpen = !win.osd.levelsOpen
                Icon {
                    anchors.centerIn: parent
                    name: "sliders-horizontal"
                    fill: more.selected
                    size: 20
                    color: more.selected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurfaceVariant
                }
                MouseArea {
                    id: moreMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: win.osd.levelsOpen = !win.osd.levelsOpen
                }
            }
        }

        // The level while dragging: Material's value indicator, beside the handle
        Rectangle {
            readonly property bool showing: area.pressed
            x: -width - 8
            y: Math.max(0, Math.min(parent.height - height, col.y + slider.y + slider.hy + slider.thick / 2 - height / 2))
            width: 52
            height: 36
            radius: height / 2
            color: Theme.m3.inverseSurface
            opacity: showing ? 1 : 0
            visible: opacity > 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.press } }
            Text {
                anchors.centerIn: parent
                text: Math.round(slider.ratio * 100)
                color: Theme.m3.inverseOnSurface
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
            }
        }

        // A toast pill: the device that just arrived, or the microphone's state
        Rectangle {
            readonly property bool showing: (win.osd.device !== "" || win.osd.kind === "mic") && !win.osd.levelsOpen && !area.pressed
            x: -width - 8 + (showing ? 0 : 8)
            y: win.pad
            Behavior on x { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
            width: Math.min(280, toastRow.implicitWidth + 32)
            height: 44
            radius: height / 2
            color: Theme.m3.inverseSurface
            opacity: showing ? 1 : 0
            visible: opacity > 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
            Row {
                id: toastRow
                anchors.centerIn: parent
                spacing: 8
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: win.osd.icon
                    fill: true
                    size: 18
                    color: Theme.m3.inverseOnSurface
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, 220)
                    elide: Text.ElideRight
                    text: win.osd.label
                    color: Theme.m3.inverseOnSurface
                    font { family: Theme.font.family; pixelSize: 14; letterSpacing: 0.25 }
                }
            }
        }
    }

    // ── Outputs, inputs or displays: a card beside the panel ────────────
    Item {
        id: card
        readonly property bool open: win.osd.levelsOpen && win.shown
        onOpenChanged: if (!open) levels.page = "list"
        width: win.cardWidth
        height: levels.implicitHeight + 48 + 8
        x: win.panelX - 12 - width
        y: Math.max(16, Math.min(win.height - height - 16, win.panelY + win.panelHeight - height))
        opacity: open ? Math.min(1, win.progress) : 0
        visible: opacity > 0
        scale: open ? 1 : 0.92
        transformOrigin: Item.BottomRight
        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
        Behavior on scale {
            SpringAnimation {
                spring: Theme.motion.springFast.spring
                damping: Theme.motion.springFast.damping
                epsilon: 0.002
            }
        }

        RectangularShadow {
            anchors.fill: parent
            radius: Theme.radius.module
            offset.y: 2
            blur: 12
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.4 : 0.16)
        }
        Rectangle {
            anchors.fill: parent
            radius: Theme.radius.module
            color: Theme.m3.surfaceContainer
        }
        Text {
            x: 20
            y: 16
            text: win.osd.kind === "brightness" ? "Displays" : win.osd.kind === "mic" ? "Inputs" : "Outputs"
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; letterSpacing: 0.1 }
        }
        IslandLevels {
            id: levels
            x: 8
            y: 48
            width: parent.width - 16
            kind: win.osd.kind
            ink: Theme.m3.onSurface
        }
    }
}
