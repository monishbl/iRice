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
    // Nobody can see the bar (a full-screen window over it, or the lock
    // screen): its animations hold still
    readonly property bool unseen: System.locked || Desktop.fullscreen(screen)
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
        // (shorter, and without the bounce, when animations are turned down)
        NumberAnimation { target: root; property: "swap"; to: 0; duration: Math.round(110 * CoolDown.motionScale); easing.type: Easing.InCubic }
        ScriptAction { script: root.shownAct = Island.primary }
        NumberAnimation { target: root; property: "swap"; to: 1; duration: Math.round(260 * CoolDown.motionScale); easing.type: CoolDown.motionScale < 1 ? Easing.OutCubic : Easing.OutBack; easing.overshoot: 1.6 }
    }
    // A scroll over the live activity: Island decides whether it's a step
    // (a notch, or one whole swipe) and which way the swap slides
    function scrolled(e) {
        const step = Island.scroll(e.angleDelta.x, e.angleDelta.y);
        if (step !== 0)
            swapDir = step;
        e.accepted = true;
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
    // Corners follow the height as it springs (no spring of their own: lagging
    // behind, they made a short island, the volume and brightness HUD, a
    // rectangle first and a pill after): a pill when short, iOS corners
    // (like Control Center's tiles) when it opens tall
    // (The same rule open or closed: switching to h / 2 the moment it starts
    // closing made the still-tall island a round blob on its way down)
    readonly property real r: Math.min(Theme.radius.tile, h / 2)
    // Closing: how open it still is (1 → 0) as the height comes down, so the
    // glass and the black fade with the shrinking shape and are gone just as
    // it reaches the dots (a fade of its own stalled and then popped: the
    // compositor's glass only hears about changes as the shape moves)
    property real closeFrom: targetH
    onExpandedChanged: if (!expanded) closeFrom = Math.max(h, 27)
    readonly property real openness: expanded ? 1 : Math.max(0, Math.min(1, (h - 26) / Math.max(1, closeFrom - 26)))
    // The content keeps the open width while it fades out (snapping to the
    // closed width moved it sideways and cut it off)
    property real openW: targetW
    onTargetWChanged: if (expanded) openW = targetW
    Behavior on w { SpringAnimation { spring: Theme.motion.spring + 1.2; damping: Theme.motion.damping + 0.04; epsilon: 0.3 } }
    Behavior on h { SpringAnimation { spring: Theme.motion.spring + 1.2; damping: Theme.motion.damping + 0.04; epsilon: 0.3 } }

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
            opacity: root.expanded ? openFade : root.openness
            property real openFade: root.expanded ? 1 : 0
            Behavior on openFade { NumberAnimation { duration: Theme.motion.hover } }
            onOpacityChanged: updatePosition()
        }
        // Black at the top melting into the glass below, like iOS 26. The
        // gradient is a plain rectangle clipped by a rounded mask (translucent
        // gradients on rounded rectangles render unevenly).
        // 0 collapsed (no pill, only the dots), 1 open
        property real openFade: root.expanded ? 1 : 0
        Behavior on openFade { NumberAnimation { duration: Theme.motion.hover } }
        readonly property real fade: root.expanded ? openFade : root.openness
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
            // edge, breathing slowly, behind the black so it only glows
            // through the glassy lower part. A pre-blurred glow image that only
            // fades, on a slow clock: nothing is re-blurred per frame.
            Image {
                id: arch
                visible: Island.showHot && root.expanded
                source: "../../assets/images/heat-glow.png"
                smooth: true
                width: ink.width * 0.95
                height: 150
                x: (ink.width - width) / 2
                y: ink.height + 4 - height / 2
                // fades slowly between faint and half (8 s a breath)
                opacity: 0.16 * (0.1 + 0.4 * (0.5 - 0.5 * Math.cos(archClock.time * Math.PI / 4)))
                Ticker { id: archClock; running: arch.visible; fps: 12 }
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
            // Playing music, open: the album art is the island itself, with the
            // same fall-off as the black (solid up top, melting into the glass
            // below; common/edgefade.frag), dimmed a little for the text
            Image {
                id: islandArt
                anchors.fill: parent
                readonly property bool wanted: Config.options.island.playerArt === "island" && Config.options.island.playerLayout !== "side"
                    && root.expanded && !Island.event && (Island.player?.trackArtUrl ?? "") !== ""
                    && (root.oneAtATime ? root.shownAct?.kind === "music" : Island.activities.some(a => a.kind === "music"))
                // Loaded once at a fixed size and kept while the music plays:
                // a size that follows the island reloaded it on every open
                // (it grows as it opens), and the art flickered
                source: Island.player?.trackArtUrl ?? ""
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(640, 640)
                asynchronous: true
                retainWhileLoading: true
                opacity: wanted && status === Image.Ready ? 0.82 : 0
                Behavior on opacity { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }
                visible: opacity > 0.01
                layer.enabled: visible
                layer.effect: ShaderEffect {
                    property vector4d fade: Qt.vector4d(0, 0, 0, 1 - inkFill.solidTo)
                    fragmentShader: Qt.resolvedUrl("../../common/edgefade.frag.qsb")
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
            // Only soft ripples (no glow under the chip): three pre-blurred
            // glows that swell and fade in turn, 1.5 s apart, on a slow clock
            // (10 fps through the mask is plenty for slow swells; none at all
            // once Cool-down has stepped in, or while nobody can see the bar:
            // it's hot, don't add work)
            Ticker { id: rippleClock; running: hotPill.visible && !root.unseen && !CoolDown.active; fps: 10 }
            Repeater {
                model: 3
                Image {
                    id: ripple
                    required property int index
                    // 0..1 through this ripple's 4.5 s swell, every 7.5 s
                    readonly property real u: {
                        const s = rippleClock.time - index * 1.5;
                        return s < 0 ? 1 : (s % 7.5) / 4.5;
                    }
                    readonly property real t: Math.sin(Math.min(u, 1) * Math.PI / 2) // ease out
                    visible: u < 1
                    source: "../../assets/images/heat-glow.png"
                    smooth: true
                    width: 60 + t * Math.max(80, shape.width * 0.9)
                    height: 54 + t * 34
                    x: hotPill.cx - width / 2
                    y: 13 - height / 2
                    opacity: 0.32 * Math.sin(Math.PI * t)
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
                Behavior on opacity { OpacityAnimator { duration: Theme.motion.hover } }
                Behavior on y { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.2 } }
            }

            // Compact: the live activity's info and controls
            Item {
                id: live
                anchors.fill: parent
                opacity: (root.compact ? 1 : 0) * root.swap
                visible: opacity > 0.01
                Behavior on opacity { enabled: !swapAnim.running; NumberAnimation { duration: Theme.motion.hover } }
                transform: [
                    Scale { origin.x: live.width / 2; origin.y: 13; xScale: 0.85 + 0.15 * root.swap; yScale: xScale },
                    Translate { x: (1 - root.swap) * 10 * root.swapDir }
                ]
                IslandLive {
                    id: lead
                    part: "info"
                    still: root.unseen
                    activity: root.shownAct
                    ink: root.ink
                    maxWidth: root.layout === "beside" ? root.area - root.trailW - 4 : 100000
                    x: root.leadX
                    y: 2
                }
                IslandLive {
                    id: trail
                    part: "controls"
                    still: root.unseen
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
                // Scroll (wheel or a two-finger swipe, up/down or left/right)
                // over it to switch between live activities
                WheelHandler {
                    enabled: root.compact && Island.activities.length > 1
                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                    onWheel: e => root.scrolled(e)
                }
            }

            // Expanded body
            Column {
                id: body
                x: root.showingEvent ? 18 : 12
                y: root.showingEvent ? 16 : 36
                width: (root.expanded ? root.targetW : root.openW) - 2 * x
                spacing: 10
                visible: root.h > 30
                opacity: root.expanded ? 1 : 0
                // (a NumberAnimation: it becomes visible as it fades in, when an
                // OpacityAnimator may not start and leave it invisible). Closing,
                // it's gone quickly, before the island shrinks over it, rather
                // than being cut off by its edges on the way down
                Behavior on opacity { enabled: root.expanded; NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }

                // Live alerts, newest at the bottom; a burst staggers in.
                // Up to four show, as many as fit in the island's window
                // (the menu bar's, 340px); more scroll, newest in view
                Flickable {
                    id: alertScroll
                    readonly property int cap: 4
                    readonly property real room: root.height - shape.y - body.y - 16 - 22 // (and the hint)
                    readonly property real capHeight: {
                        const last = alertRepeater.count > cap ? alertRepeater.itemAt(cap - 1) : null;
                        return Math.min(room, last ? last.y + last.height : alertCol.implicitHeight);
                    }
                    readonly property bool overflows: alertCol.implicitHeight > capHeight + 1
                    width: body.width
                    height: Math.min(alertCol.implicitHeight, capHeight)
                    visible: alertRepeater.count > 0
                    contentHeight: alertCol.implicitHeight
                    interactive: overflows
                    clip: overflows
                    boundsBehavior: Flickable.StopAtBounds
                    // A new alert scrolls the list to it
                    onContentHeightChanged: if (overflows) contentY = Math.max(0, contentHeight - height)
                    Column {
                        id: alertCol
                        width: parent.width
                        spacing: 10
                        Repeater {
                            id: alertRepeater
                            model: Island.alerts
                            IslandEvent {
                                required property int uid
                                required property int index
                                width: alertCol.width
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
                    }
                }
                // How many more there are above, while scrolled to the newest
                Text {
                    visible: alertScroll.overflows
                    width: body.width
                    horizontalAlignment: Text.AlignHCenter
                    // Alerts out of view above (earlier) and below
                    readonly property var outside: {
                        let above = 0, below = 0;
                        const top = alertScroll.contentY, bottom = top + alertScroll.height;
                        for (let i = 0; i < alertRepeater.count; i++) {
                            const it = alertRepeater.itemAt(i);
                            if (!it)
                                continue;
                            if (it.y + it.height / 2 < top)
                                above++;
                            else if (it.y + it.height / 2 > bottom)
                                below++;
                        }
                        return [above, below];
                    }
                    text: outside[0] > 0 ? outside[0] + " earlier · scroll up" : outside[1] + " more · scroll down"
                    color: Qt.rgba(1, 1, 1, 0.55)
                    font { family: Theme.font.family; pixelSize: Theme.font.caption }
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
                            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                            onWheel: e => root.scrolled(e)
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
                // All at once: music still gets the full player (big art,
                // controls over it, open the app, volume), the rest are rows
                IslandFocusCard {
                    readonly property var musicAct: Island.activities.find(a => a.kind === "music") ?? null
                    visible: !Island.event && !root.oneAtATime && musicAct !== null
                    width: body.width
                    height: visible ? implicitHeight : 0
                    activity: musicAct ?? { kind: "music" }
                }
                Repeater {
                    model: Island.event || root.oneAtATime ? [] : Island.activities.filter(a => a.kind !== "music")
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
            visible: opacity > 0.01 && !root.showingEvent
            x: parent.width - width - 6
            y: 3
            width: 26
            height: 26
            icon: Island.pinned ? "push-pin-slash" : "push-pin"
            fill: Island.pinned
            iconSize: 14
            iconColor: Island.pinned ? "#F5F5F7" : Qt.rgba(1, 1, 1, 0.55)
            // Gone the moment it starts closing (as fast as a fade-in would
            // be noticed: an animated fade-out lingered over the shrinking pill)
            opacity: root.expanded && root.h > 40 ? 1 : 0
            Behavior on opacity { enabled: root.expanded; NumberAnimation { duration: Theme.motion.hover } } // (shown as it fades in: see the body above)
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
