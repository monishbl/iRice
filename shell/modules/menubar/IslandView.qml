import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import Quickshell.Services.Mpris
import qs.common
import qs.glass
import qs.services

// The dynamic island at the top centre of the menu bar, black like the
// iPhone's, melting into glass at its lower rim. Idle: the workspace dots.
// Compact: an ongoing activity (recording, timer, music, stopwatch) widens
// the pill with an icon on the left and a detail on the right. Expanded: it
// springs open for an event (Island.event: an animated alert, or an iOS-style
// card) or while hovered (the dots on top, live activities below).
Item {
    id: root

    required property var screen
    required property color fg // menu bar text colour (idle and compact)
    property point pointer: Qt.point(-1, -1) // window pointer, from the bar's tracker

    readonly property var primary: Island.primary
    readonly property bool hovering: pointer.x >= shape.x - 4 && pointer.x < shape.x + shape.width + 4 && pointer.y >= 0 && pointer.y < shape.y + shape.height + 4
    property bool hoverOpen: false
    onHoveringChanged: hovering ? (closeDelay.stop(), openDelay.restart()) : (openDelay.stop(), closeDelay.restart())
    Timer { id: openDelay; interval: 140; onTriggered: root.hoverOpen = true }
    Timer { id: closeDelay; interval: 260; onTriggered: root.hoverOpen = false }

    readonly property bool expanded: hoverOpen || Island.event !== null || Island.pinned
    readonly property bool compact: !expanded && primary !== null
    readonly property string layout: Config.options.island.activityLayout // replace | split | beside
    readonly property bool oneAtATime: Config.options.island.activityMode !== "all"

    // The activity on show, swapped with a quick collapse and reopen when
    // the focus moves (scroll, or a new one starts)
    property var shownAct: Island.primary
    property real swap: 1
    property int swapDir: 1
    readonly property string primaryKind: Island.primary?.kind ?? ""
    onPrimaryKindChanged: {
        if (!shownAct || !Island.primary) {
            shownAct = Island.primary;
            return;
        }
        swapAnim.restart();
    }
    SequentialAnimation {
        id: swapAnim
        NumberAnimation { target: root; property: "swap"; to: 0; duration: 110; easing.type: Easing.InCubic }
        ScriptAction { script: root.shownAct = Island.primary }
        NumberAnimation { target: root; property: "swap"; to: 1; duration: 260; easing.type: Easing.OutBack; easing.overshoot: 1.6 }
    }
    readonly property bool showingEvent: Island.event !== null
    // On the black island when open; collapsed it's just the dots on the bar
    readonly property color ink: expanded || Island.showHot ? "#F5F5F7" : fg

    // ── Size, on springs ────────────────────────────────────────────────
    readonly property real dotsW: dots.implicitWidth
    // Collapsed with a live activity: its info and controls replace the dots,
    // or sit around them (split), or after them (beside)
    readonly property int pad: 10
    readonly property real leadW: lead.implicitWidth
    readonly property real trailW: trail.implicitWidth
    // Split: both sides as wide as the wider one, so the dots stay centred.
    // Beside: the activity gets the dots' width (at least room for its
    // controls), after a thin divider.
    readonly property real side: Math.max(leadW, trailW)
    readonly property real area: Math.max(dotsW, trailW + 26)
    // Positions from the island's centre (w animates), so the pill grows
    // evenly around its content instead of the content sliding in
    readonly property real groupLeft: w / 2 - compactW / 2 + pad // left edge of the compact content
    readonly property real leadX: layout === "split" ? w / 2 - dotsW / 2 - 12 - leadW : layout === "beside" ? groupLeft + dotsW + 21 : groupLeft
    readonly property real trailX: layout === "split" ? w / 2 + dotsW / 2 + 12 : w / 2 + compactW / 2 - pad - trailW + 4
    readonly property real compactW: layout === "split" ? 2 * pad + 2 * side + 24 + dotsW
        : layout === "beside" ? 2 * pad + dotsW + 21 + area
        : pad + leadW + 12 + trailW + pad
    readonly property real targetW: showingEvent ? (Island.hasCard ? 440 : 392) : expanded ? 392 : compact ? compactW : dotsW + 14
    readonly property real targetH: expanded ? body.y + body.implicitHeight + (showingEvent ? 16 : 14) : 26
    property real w: targetW
    property real h: targetH
    property real r: expanded ? (Island.hasCard ? 32 : 26) : 13
    Behavior on w { SpringAnimation { spring: Theme.motion.spring + 1.2; damping: Theme.motion.damping + 0.04; epsilon: 0.3 } }
    Behavior on h { SpringAnimation { spring: Theme.motion.spring + 1.2; damping: Theme.motion.damping + 0.04; epsilon: 0.3 } }
    Behavior on r { SpringAnimation { spring: Theme.motion.spring + 1.2; damping: Theme.motion.damping + 0.08; epsilon: 0.1 } }

    // The visible shape, centred at the top
    readonly property alias shape: shape
    Item {
        id: shape
        x: Math.round((root.width - root.w) / 2)
        y: 3
        width: root.w
        height: root.h
        onXChanged: glass.updatePosition()
        onWidthChanged: glass.updatePosition()
        onHeightChanged: glass.updatePosition()

        // Glass underneath shows through only at the lower rim when open
        Glass {
            id: glass
            anchors.fill: parent
            anchors.margins: -shadowMargin
            shadowMargin: 16
            radius: root.r
            bevel: Math.min(16, root.r) + shadowMargin
            opacity: root.expanded ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
        }
        // Black at the top melting into the glass below, like iOS 26. The
        // gradient is a plain rectangle clipped by a rounded mask (translucent
        // gradients on rounded rectangles render unevenly).
        // 0 collapsed (no pill, only the dots), 1 open
        property real fade: root.expanded ? 1 : 0
        Behavior on fade { NumberAnimation { duration: Theme.motion.hover } }
        Item {
            id: ink
            anchors.fill: parent
            opacity: shape.fade
            visible: opacity > 0.01
            layer.enabled: true
            layer.effect: MultiEffect {
                maskEnabled: true
                maskSource: inkMask
                maskThresholdMin: 0.5
                maskSpreadAtMin: 1
            }
            // Running hot, while open: a faint red arch rising from the bottom
            // edge (a blurred ellipse), breathing slowly, behind the black so
            // it only glows through the glassy lower part
            Rectangle {
                id: arch
                visible: Island.showHot && root.expanded
                property real breath: 0
                width: ink.width * 0.8
                height: 96
                opacity: 0.1 + 0.4 * breath // fades slowly between faint and half
                x: (ink.width - width) / 2
                y: ink.height - height * 0.45
                radius: height / 2
                color: Qt.rgba(0.9, 0.16, 0.12, 0.28)
                layer.enabled: visible
                layer.effect: MultiEffect {
                    blurEnabled: true
                    blur: 1
                    blurMax: 64
                }
                SequentialAnimation on breath {
                    running: arch.visible
                    loops: Animation.Infinite
                    NumberAnimation { from: 0; to: 1; duration: 4000; easing.type: Easing.InOutSine }
                    NumberAnimation { from: 1; to: 0; duration: 4000; easing.type: Easing.InOutSine }
                }
            }
            Rectangle {
                id: inkFill
                anchors.fill: parent
                // Solid at the top, then fading into the glass
                readonly property real solidTo: 0.3 // the lower ~70% turns to glass
                gradient: Gradient {
                    GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.94) }
                    GradientStop { position: inkFill.solidTo; color: Qt.rgba(0, 0, 0, 0.9) }
                    GradientStop { position: 1; color: Qt.rgba(0, 0, 0, 0.12) }
                }
            }
        }

        // Running hot (Island.showHot), closed: a soft red glow on the active
        // space's chip (which turns red too) and faint ripples growing out of
        // it, until the CPU cools down
        property real heat: Island.showHot && !root.expanded ? 1 : 0
        Behavior on heat { NumberAnimation { duration: 600; easing.type: Easing.InOutSine } }
        Item {
            id: hotPill
            anchors.fill: parent
            visible: shape.heat > 0.01
            opacity: shape.heat
            layer.enabled: visible
            layer.effect: MultiEffect {
                maskEnabled: true
                maskSource: inkMask
                maskThresholdMin: 0.5
                maskSpreadAtMin: 1
            }
            readonly property real cx: dots.x + dots.activeCenter
            // Only soft, blurred ripples (no glow under the chip)
            Item {
                id: ripples
                anchors.fill: parent
                layer.enabled: hotPill.visible
                layer.effect: MultiEffect {
                    blurEnabled: true
                    blur: 1
                    blurMax: 28
                }
                Repeater {
                model: 3
                Rectangle {
                    id: ripple
                    required property int index
                    property real t: 0
                    width: 20 + t * Math.max(80, shape.width * 0.9)
                    height: 14 + t * 34
                    x: hotPill.cx - width / 2
                    y: 13 - height / 2
                    radius: height / 2
                    // A soft filled glow that swells and fades (blurred above)
                    color: Qt.rgba(1, 0.3, 0.26, 0.2 * Math.sin(Math.PI * t))
                    SequentialAnimation on t {
                        running: hotPill.visible
                        loops: Animation.Infinite
                        PauseAnimation { duration: ripple.index * 1500 }
                        NumberAnimation { from: 0; to: 1; duration: 4500; easing.type: Easing.OutSine }
                        PauseAnimation { duration: (2 - ripple.index) * 1500 }
                    }
                }
                }
            }
        }
        Rectangle {
            id: inkMask
            anchors.fill: parent
            radius: root.r
            visible: false
            layer.enabled: true
        }
        Rectangle {
            anchors.fill: parent
            radius: root.r
            color: "transparent"
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.14)
            opacity: shape.fade
            visible: opacity > 0.01
        }
        // A soft highlight along the glassy lower edge
        Rectangle {
            visible: root.expanded
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: root.r * 0.6; rightMargin: root.r * 0.6; bottomMargin: 1 }
            height: 1
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: Qt.rgba(1, 1, 1, 0) }
                GradientStop { position: 0.5; color: Qt.rgba(1, 1, 1, 0.35) }
                GradientStop { position: 1; color: Qt.rgba(1, 1, 1, 0) }
            }
        }

        Item {
            id: clip
            anchors.fill: parent
            clip: true

            // Top row: activity icon | dots | detail
            Workspaces {
                id: dots
                plain: true
                hot: Island.showHot && !root.expanded
                screen: root.screen
                fg: root.ink
                x: !root.compact || root.layout !== "beside" ? (root.w - width) / 2 : root.groupLeft
                y: root.expanded ? 7 : 2
                opacity: root.showingEvent || (root.compact && root.layout === "replace") ? 0 : 1
                Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                Behavior on y { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.2 } }
            }

            // Compact: the live activity's info and controls
            Item {
                id: live
                anchors.fill: parent
                opacity: (root.compact ? 1 : 0) * root.swap
                visible: opacity > 0.01
                Behavior on opacity { enabled: !swapAnim.running; NumberAnimation { duration: Theme.motion.hover } }
                transform: Scale { origin.x: live.width / 2; origin.y: 13; xScale: 0.85 + 0.15 * root.swap; yScale: xScale }
                IslandLive {
                    id: lead
                    part: "info"
                    activity: root.shownAct
                    ink: root.ink
                    maxWidth: root.layout === "beside" ? root.area - root.trailW - 4 : 100000
                    x: root.leadX
                    y: 2
                }
                IslandLive {
                    id: trail
                    part: "controls"
                    activity: root.shownAct
                    ink: root.ink
                    x: root.trailX
                    y: 2
                }
                // Beside: a thin divider between the spaces and the activity
                Rectangle {
                    visible: root.layout === "beside"
                    x: root.groupLeft + root.dotsW + 10
                    y: 6
                    width: 1
                    height: 14
                    color: Qt.alpha(root.ink, 0.3)
                }
                // Scroll over it to switch between live activities
                WheelHandler {
                    enabled: root.compact && Island.activities.length > 1
                    onWheel: e => Island.cycle(e.angleDelta.y < 0 ? 1 : -1)
                }
            }

            // Expanded body
            Column {
                id: body
                x: root.showingEvent ? 18 : 12
                y: root.showingEvent ? 16 : 36
                width: root.targetW - 2 * x
                spacing: 10
                visible: root.h > 30
                opacity: root.expanded ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }

                // An event takes the whole body
                // Live alerts, newest at the bottom; a burst staggers in
                Repeater {
                    model: Island.alerts
                    IslandEvent {
                        required property int uid
                        required property int index
                        width: body.width
                        event: Island.byUid[uid] ?? null
                        delay: event?.delay ?? 0
                        onActed: Island.dismiss()
                        // A hairline between stacked alerts
                        Rectangle {
                            visible: parent.index > 0
                            y: -6
                            width: parent.width
                            height: 1
                            color: Qt.rgba(1, 1, 1, 0.08)
                        }
                    }
                }

                // Otherwise: what's live (one focused card, or all stacked),
                // then a small status row
                Item {
                    visible: !Island.event && root.oneAtATime && Island.activities.length > 0
                    width: body.width
                    height: visible ? focusCard.implicitHeight + (pageDots.visible ? 14 : 0) : 0
                    IslandFocusCard {
                        id: focusCard
                        width: parent.width
                        activity: root.shownAct
                        opacity: root.swap
                        transform: Translate { x: (1 - root.swap) * 16 * root.swapDir }
                        WheelHandler {
                            enabled: Island.activities.length > 1
                            onWheel: e => {
                                root.swapDir = e.angleDelta.y < 0 ? 1 : -1;
                                Island.cycle(root.swapDir);
                            }
                        }
                    }
                    // Which one this is, when there are several
                    Row {
                        id: pageDots
                        visible: Island.activities.length > 1
                        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom }
                        spacing: 6
                        Repeater {
                            model: Island.activities
                            Rectangle {
                                required property var modelData
                                required property int index
                                width: index === Island.focusIndex ? 14 : 6
                                height: 6
                                radius: 3
                                color: Qt.rgba(1, 1, 1, index === Island.focusIndex ? 0.9 : 0.35)
                                Behavior on width { NumberAnimation { duration: Theme.motion.hover } }
                                MouseArea {
                                    anchors { fill: parent; margins: -4 }
                                    onClicked: Island.focusKind = parent.modelData.kind
                                }
                            }
                        }
                    }
                }
                Repeater {
                    model: Island.event || root.oneAtATime ? [] : Island.activities
                    IslandActivity {
                        required property var modelData
                        required property int index
                        width: body.width
                        activity: modelData
                        delay: index * 45
                        shown: root.expanded
                    }
                }
                IslandStatus {
                    width: parent.width
                    visible: !Island.event
                    delay: Island.activities.length * 45
                    shown: root.expanded
                }
            }
        }

        // Pin: keep the island open even with nothing happening
        IconButton {
            visible: root.expanded && !root.showingEvent
            x: parent.width - width - 6
            y: 3
            width: 26
            height: 26
            icon: Island.pinned ? "push-pin-slash" : "push-pin"
            fill: Island.pinned
            iconSize: 14
            iconColor: Island.pinned ? "#F5F5F7" : Qt.rgba(1, 1, 1, 0.55)
            opacity: root.h > 40 ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
            onClicked: Island.togglePin()
        }

        // Tap the compact pill to open it; right-click pins it open
        TapHandler {
            enabled: root.compact
            onTapped: root.hoverOpen = true
        }
        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: Island.togglePin()
        }
    }
}
