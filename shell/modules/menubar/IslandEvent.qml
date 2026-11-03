import QtQuick
import QtQuick.Shapes
import qs.common
import qs.services

// One event in the expanded (black) island. Two layouts:
//  alert  an animated glyph, a title and detail that slide in one after the
//         other, and on the right a battery ring or an action ("Open")
//  card   like iOS: a line of large text, then a glass card holding an
//         avatar, a name, a time and a message (notifications)
// Volume and brightness use a slim HUD row; a ringing call its own card
// (IslandCall), which only Answer, Decline or Esc put away.
// Material: the island's colours (IslandInk, `inverse` on an alert), tonal
// and filled pill buttons, a tall slider for the HUD, avatars in a cookie.
Item {
    id: root

    property var event: null
    property int delay: 0 // ms before it appears (stacked alerts stagger in)
    readonly property string kind: event?.kind ?? ""
    readonly property bool isCard: !!event?.card
    readonly property bool isHud: kind === "hud"
    readonly property bool isRinging: kind === "ringing"
    property bool unseen: false // nobody can see the island (its animations hold)
    // One of its buttons ran (`uid`: this alert's). Alerts the button itself
    // posted or changed ("Connecting…") are from `since` on: they stay up.
    signal acted(int uid, real since)
    function act(f) {
        const uid = event?.uid ?? 0, since = Date.now();
        f?.();
        acted(uid, since);
    }
    signal dismissRequested // swiped up, the close button, or Esc (IslandView removes it)
    // A wheel or two-finger swipe up over it dismisses it too (off while the
    // alert list scrolls instead)
    property bool wheelDismiss: true

    property bool inverse: false // (Material: shown on the alert surface, IslandInk)
    readonly property color ink: IslandInk.ink(inverse)
    readonly property color ink2: IslandInk.dim(0.6, inverse)
    // Material's pill buttons: filled (the action), tonal (the others),
    // with their state layers
    function filledColor(m) {
        return IslandInk.layer(IslandInk.accent(inverse), IslandInk.onAccent(inverse), m);
    }
    function tonalColor(m) {
        return IslandInk.layer(IslandInk.tonal(inverse), IslandInk.onTonal(inverse), m);
    }

    // More buttons than the two beside the title (a notification's own,
    // added to the alert): further rows under it
    readonly property var more: event?.more ?? []
    // (a device's picture is shown larger than an icon)
    readonly property int rowHeight: event?.picture && !event?.art ? 60 : 52
    implicitHeight: isHud ? 34 + (levelsOpen ? levels.implicitHeight + 6 : 0) : isRinging ? callCard.implicitHeight : isCard ? cardLayout.implicitHeight : rowHeight + (more.length > 0 ? moreRow.implicitHeight + 8 : 0)

    // Appear after `delay`, sliding down into place; then the text staggers.
    // A new event (a different uid) replays it; an update in place doesn't.
    property real t: 0
    property real appear: 0
    property int playedUid: -1
    onEventChanged: {
        // Given more time while on its way out (the same alert again, the
        // HUD's level changing): it stays
        if (leaving && event && (event.expires ?? 0) > Date.now() + 200) {
            leaveAnim.stop();
            leaving = false;
            gone = 0;
        }
        if (!event || event.uid === playedUid)
            return;
        playedUid = event.uid ?? -1;
        t = 0;
        appear = 0;
        intro.restart();
    }
    SequentialAnimation {
        id: intro
        PauseAnimation { duration: root.delay }
        ScriptAction { script: glyph.play() }
        ParallelAnimation {
            NumberAnimation { target: root; property: "appear"; to: 1; duration: 320; easing.type: Easing.OutCubic }
            NumberAnimation { target: root; property: "t"; from: 0; to: 1; duration: 700 }
        }
    }
    // Dismissing: pulled up by a drag (pull, ≤ 0), then leaving (gone 0 → 1),
    // sliding up and fading; only fading with Reduce motion
    readonly property bool still: Config.options.appearance.reduceMotion === true
    property real pull: 0
    property real gone: 0
    property bool leaving: false
    function leave() {
        if (leaving)
            return;
        leaving = true;
        settle.stop();
        leaveAnim.start();
    }
    NumberAnimation {
        id: leaveAnim
        target: root
        property: "gone"
        to: 1
        duration: Math.round(140 * CoolDown.motionScale)
        easing.type: Easing.InCubic
        onFinished: root.dismissRequested()
    }
    NumberAnimation {
        id: settle
        target: root
        property: "pull"
        to: 0
        duration: Theme.motion.popover
        easing.type: Easing.OutCubic
    }
    opacity: appear * (1 - gone) * Math.max(0.3, 1 + pull / 90)
    transform: Translate { y: root.still ? 0 : (1 - root.appear) * -10 + root.pull - root.gone * 18 }

    // Its time is up (Island's sweep): out the same way as a dismissal
    Connections {
        target: Island
        function onExpiring(uid) {
            if (uid === root.event?.uid)
                root.leave();
        }
    }

    HoverHandler { id: hover }
    // Drag it up to dismiss it (a short pull springs back)
    DragHandler {
        id: swipe
        target: null
        enabled: !root.isHud && !root.isRinging // (a call isn't flicked away by accident)
        xAxis.enabled: false
        grabPermissions: PointerHandler.CanTakeOverFromAnything
        onTranslationChanged: if (active) root.pull = Math.min(0, translation.y)
        onActiveChanged: {
            if (active) {
                settle.stop();
                return;
            }
            if (root.pull < -24 || centroid.velocity.y < -400)
                root.leave();
            else
                settle.start();
        }
    }
    WheelHandler {
        enabled: !root.isHud && !root.isRinging && root.wheelDismiss
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        property real sum: 0
        property real lastAt: 0
        onWheel: e => {
            const now = Date.now();
            if (now - lastAt > 400)
                sum = 0;
            lastAt = now;
            // Up: the wheel away from you, or two fingers up (natural
            // scrolling reports it inverted)
            const up = e.inverted ? -e.angleDelta.y : e.angleDelta.y;
            sum = up > 0 ? sum + up : 0;
            if (sum >= 120)
                root.leave();
            e.accepted = true;
        }
    }
    // 0..1 progress of an element that starts at `from` (fraction of the stagger)
    function step(from) {
        return Math.max(0, Math.min(1, (t - from) / 0.45));
    }
    function ease(x) {
        return 1 - Math.pow(1 - x, 3);
    }

    // ── HUD ─────────────────────────────────────────────────────────────
    // The live level: click or drag along it, or scroll over it, to set it;
    // the icon mutes; the button at the end opens the outputs, inputs or
    // displays underneath (IslandLevels)
    property bool levelsOpen: false
    onIsHudChanged: if (!isHud) levelsOpen = false
    readonly property string hudKind: event?.hud ?? "volume"
    readonly property real hudValue: hudKind === "brightness" ? Brightness.value : hudKind === "mic" ? (Audio.micMuted ? 0 : Audio.micVolume) : (Audio.muted ? 0 : Audio.volume)
    function setHud(v) {
        v = Math.max(0, Math.min(1, v));
        if (hudKind === "brightness")
            Brightness.set(v, true);
        else if (hudKind === "mic")
            Audio.setMicVolume(v, true);
        else
            Audio.setVolume(v, true);
    }
    Item {
        id: hudRow
        visible: root.isHud
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: 34
        Icon {
            id: hudIcon
            anchors.verticalCenter: parent.verticalCenter
            x: 6
            name: root.hudKind === "brightness" ? "sun" : root.hudKind === "mic" ? (Audio.micMuted ? "microphone-slash" : "microphone") : Audio.icon
            fill: true
            size: 18
            color: root.ink
            MouseArea {
                anchors.fill: parent
                anchors.margins: -6
                enabled: root.hudKind !== "brightness"
                cursorShape: Qt.PointingHandCursor
                onClicked: root.hudKind === "mic" ? Audio.toggleMic(true) : Audio.toggleMute(true)
            }
        }
        // (just after switching to another output or microphone: its name)
        Text {
            id: hudName
            visible: !!root.event?.device
            anchors { left: hudIcon.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
            width: visible ? Math.min(Math.ceil(nameWidth.advanceWidth), parent.width * 0.4) : 0
            elide: Text.ElideRight
            text: root.event?.device ?? ""
            color: root.ink
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            TextMetrics { id: nameWidth; font: hudName.font; text: hudName.text }
        }
        Rectangle {
            id: hudBar
            visible: !Theme.material
            anchors { left: hudName.visible ? hudName.right : hudIcon.right; leftMargin: 12; right: levelsButton.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            height: 8
            radius: 4
            color: Qt.rgba(1, 1, 1, 0.16)
            Rectangle {
                width: parent.width * Math.max(0, Math.min(1, root.hudValue))
                height: parent.height
                radius: 4
                color: root.ink
                Behavior on width {
                    enabled: !barMouse.pressed
                    SpringAnimation { spring: 5; damping: 0.4; epsilon: 0.5 }
                }
            }
            MouseArea {
                id: barMouse
                anchors.fill: parent
                anchors.margins: -10
                cursorShape: Qt.PointingHandCursor
                preventStealing: true
                onPressed: e => root.setHud((e.x - 10) / hudBar.width)
                onPositionChanged: e => {
                    if (pressed)
                        root.setHud((e.x - 10) / hudBar.width);
                }
            }
            WheelHandler {
                target: null
                onWheel: e => root.setHud(root.hudValue + (e.angleDelta.y > 0 ? 0.05 : -0.05))
            }
        }
        // Material: the level on Material's tall slider
        Loader {
            active: Theme.material && root.isHud
            anchors { left: hudName.visible ? hudName.right : hudIcon.right; leftMargin: 12; right: levelsButton.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            sourceComponent: Slider {
                value: Math.max(0, Math.min(1, root.hudValue))
                accessibleName: root.hudKind === "brightness" ? "Brightness" : root.hudKind === "mic" ? "Microphone volume" : "Volume"
                onMoved: v => root.setHud(v)
                WheelHandler {
                    target: null
                    onWheel: e => root.setHud(root.hudValue + (e.angleDelta.y > 0 ? 0.05 : -0.05))
                }
            }
        }
        Rectangle {
            id: levelsButton
            anchors { right: parent.right; rightMargin: 2; verticalCenter: parent.verticalCenter }
            width: 28
            height: 28
            radius: 14
            color: buttonMouse.pressed ? IslandInk.dim(0.2, root.inverse) : buttonMouse.containsMouse || root.levelsOpen ? IslandInk.dim(0.12, root.inverse) : "transparent"
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            Accessible.role: Accessible.Button
            Accessible.name: root.hudKind === "brightness" ? "Displays" : root.hudKind === "mic" ? "Inputs" : "Outputs"
            Icon {
                anchors.centerIn: parent
                name: root.levelsOpen ? "caret-up" : "caret-down"
                bold: true
                size: 13
                color: root.ink
            }
            MouseArea {
                id: buttonMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.levelsOpen = !root.levelsOpen
            }
        }
    }
    IslandLevels {
        id: levels
        visible: root.isHud && root.levelsOpen
        anchors { left: parent.left; right: parent.right; top: hudRow.bottom; topMargin: 6 }
        kind: root.hudKind
        ink: root.ink
        onVisibleChanged: if (!visible) page = "list"
    }

    // ── Alert ───────────────────────────────────────────────────────────
    Flow {
        id: moreRow
        visible: !root.isHud && !root.isCard && !root.isRinging && root.more.length > 0
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: 4; rightMargin: 4 }
        layoutDirection: Qt.RightToLeft // under the buttons beside the title
        spacing: 6
        opacity: root.ease(root.step(0.45))
        Repeater {
            model: root.more
            Rectangle {
                required property var modelData
                width: moreText.implicitWidth + 24
                height: Theme.material ? 32 : 28
                radius: height / 2
                color: Theme.material ? root.tonalColor(moreMouse) : moreMouse.pressed ? Qt.rgba(1, 1, 1, 0.3) : moreMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.24) : Qt.rgba(1, 1, 1, 0.16)
                Text {
                    id: moreText
                    anchors.centerIn: parent
                    text: parent.modelData.text
                    color: Theme.material ? IslandInk.onTonal(root.inverse) : root.ink
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea {
                    id: moreMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        const m = parent.modelData;
                        root.act(() => m.act?.());
                    }
                }
            }
        }
    }
    Item {
        visible: !root.isHud && !root.isCard && !root.isRinging
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: root.rowHeight

        IslandGlyph {
            id: glyph
            inverse: root.inverse
            size: root.rowHeight - 8
            anchors.verticalCenter: parent.verticalCenter
            event: !root.isHud && !root.isCard && !root.isRinging ? root.event : null
        }
        Column {
            anchors { left: glyph.right; leftMargin: root.kind === "audio" ? 26 : 14; right: trailing.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                textFormat: Text.PlainText
                text: root.event?.title ?? ""
                elide: Text.ElideRight
                color: root.ink
                font { family: Theme.font.family; pixelSize: 15; weight: Font.DemiBold }
                opacity: root.ease(root.step(0.1))
                transform: Translate { x: (1 - root.ease(root.step(0.1))) * 14 }
            }
            Text {
                width: parent.width
                visible: text !== ""
                textFormat: Text.PlainText
                text: root.event?.subtitle ?? ""
                elide: Text.ElideRight
                color: root.event?.subtitleColor ?? root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                opacity: root.ease(root.step(0.25))
                transform: Translate { x: (1 - root.ease(root.step(0.25))) * 14 }
            }
        }

        // Trailing: a device battery ring and/or an action (and a second one,
        // e.g. Decline before Accept); the ring sits before the buttons
        Item {
            id: trailing
            readonly property real pills: root.event?.action ? actionPill.width + (root.event?.action2 ? secondPill.width + 6 : 0) : 0
            anchors { right: parent.right; rightMargin: 4; verticalCenter: parent.verticalCenter }
            // (earbuds: left, right and case rings instead of one)
            readonly property var parts: root.event?.batteries ?? []
            readonly property real rings: parts.length > 0 ? parts.length * 28 + (parts.length - 1) * 6 : root.event?.battery !== undefined ? 34 : 0
            width: (rings > 0 ? rings + (pills > 0 ? 8 : 0) : 0) + pills
            height: parts.length > 0 ? 42 : 34
            Behavior on width { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }
            opacity: root.ease(root.step(0.4))
            scale: 0.7 + 0.3 * root.ease(root.step(0.4))

            Shape {
                id: batRing
                width: 34
                height: 34
                visible: root.event?.battery !== undefined && trailing.parts.length === 0
                anchors.verticalCenter: parent.verticalCenter
                preferredRendererType: Shape.CurveRenderer
                readonly property real level: Math.max(0, Math.min(1, root.event?.battery ?? 0))
                readonly property color col: level <= 0.2 ? Theme.color.danger : Theme.material ? IslandInk.accent(root.inverse) : Theme.color.success
                // The level arriving after the card is up (headphones report
                // it seconds after connecting): the ring draws itself round
                // from empty and the number fades in
                readonly property bool has: root.event?.battery !== undefined
                property real traced: 1
                onHasChanged: if (has && root.t >= 1) traceIn.restart()
                NumberAnimation { id: traceIn; target: batRing; property: "traced"; from: 0; to: 1; duration: Math.round(600 * CoolDown.motionScale); easing.type: Easing.OutCubic }
                ShapePath {
                    strokeColor: IslandInk.dim(0.16, root.inverse)
                    strokeWidth: 3
                    fillColor: "transparent"
                    PathAngleArc { centerX: 17; centerY: 17; radiusX: 15; radiusY: 15; startAngle: -90; sweepAngle: 360 }
                }
                ShapePath {
                    strokeColor: batRing.col
                    strokeWidth: 3
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    PathAngleArc { centerX: 17; centerY: 17; radiusX: 15; radiusY: 15; startAngle: -90; sweepAngle: 360 * batRing.level * root.ease(root.step(0.4)) * batRing.traced }
                }
                Text {
                    anchors.centerIn: parent
                    text: Math.round(batRing.level * 100)
                    opacity: Math.min(1, batRing.traced * 1.6)
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: 11; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
            }
            Row {
                id: partRings
                visible: trailing.parts.length > 0
                spacing: 6
                // (they trace in as they arrive, like the single ring)
                property real traced: 1
                readonly property int count: trailing.parts.length
                onCountChanged: if (count > 0 && root.t >= 1) partsIn.restart()
                NumberAnimation { id: partsIn; target: partRings; property: "traced"; from: 0; to: 1; duration: Math.round(600 * CoolDown.motionScale); easing.type: Easing.OutCubic }
                Repeater {
                    model: trailing.parts
                    Column {
                        id: part
                        required property var modelData
                        spacing: 2
                        readonly property color col: modelData.value <= 0.2 ? Theme.color.danger : Theme.material ? IslandInk.accent(root.inverse) : Theme.color.success
                        Shape {
                            width: 28
                            height: 28
                            preferredRendererType: Shape.CurveRenderer
                            ShapePath {
                                strokeColor: IslandInk.dim(0.16, root.inverse)
                                strokeWidth: 2.5
                                fillColor: "transparent"
                                PathAngleArc { centerX: 14; centerY: 14; radiusX: 12.5; radiusY: 12.5; startAngle: -90; sweepAngle: 360 }
                            }
                            ShapePath {
                                strokeColor: part.col
                                strokeWidth: 2.5
                                fillColor: "transparent"
                                capStyle: ShapePath.RoundCap
                                PathAngleArc { centerX: 14; centerY: 14; radiusX: 12.5; radiusY: 12.5; startAngle: -90; sweepAngle: 360 * Math.max(0, Math.min(1, part.modelData.value)) * root.ease(root.step(0.4)) * partRings.traced }
                            }
                            Text {
                                anchors.centerIn: parent
                                text: Math.round(part.modelData.value * 100)
                                opacity: Math.min(1, partRings.traced * 1.6)
                                color: root.ink
                                font { family: Theme.font.family; pixelSize: 10; weight: Font.DemiBold; features: { "tnum": 1 } }
                            }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: part.modelData.label
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: 10; weight: Font.Medium }
                        }
                    }
                }
            }

            Rectangle {
                id: secondPill
                visible: !!root.event?.action2
                anchors.verticalCenter: parent.verticalCenter
                width: secondText.implicitWidth + 24
                height: Theme.material ? 32 : 28
                radius: height / 2
                color: Theme.material ? IslandInk.dim(secondMouse.pressed ? 0.1 : secondMouse.containsMouse ? 0.08 : 0, root.inverse) : secondMouse.pressed ? Qt.rgba(1, 1, 1, 0.16) : secondMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.1) : "transparent"
                border.width: 1
                border.color: Theme.material ? IslandInk.dim(0.38, root.inverse) : Qt.rgba(1, 1, 1, 0.2)
                Text {
                    id: secondText
                    anchors.centerIn: parent
                    text: root.event?.action2 ?? ""
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                MouseArea {
                    id: secondMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.act(root.event?.onAction2);
                    }
                }
            }
            Rectangle {
                id: actionPill
                visible: !!root.event?.action
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: actionText.implicitWidth + 24
                height: Theme.material ? 32 : 28
                radius: height / 2
                color: Theme.material ? root.filledColor(actionMouse) : actionMouse.pressed ? Qt.rgba(1, 1, 1, 0.3) : actionMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.24) : Qt.rgba(1, 1, 1, 0.16)
                Text {
                    id: actionText
                    anchors.centerIn: parent
                    text: root.event?.action ?? ""
                    color: Theme.material ? IslandInk.onAccent(root.inverse) : root.ink
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea {
                    id: actionMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.act(root.event?.onAction);
                    }
                }
            }
        }
    }

    // ── Ringing call ────────────────────────────────────────────────────
    IslandCall {
        id: callCard
        visible: root.isRinging
        width: parent.width
        event: root.isRinging ? root.event : null
        inverse: root.inverse
        t: root.t
        still: root.unseen
    }

    // ── Card (iOS style) ────────────────────────────────────────────────
    Column {
        id: cardLayout
        visible: root.isCard
        width: parent.width
        spacing: 12

        Text {
            width: parent.width
            visible: text !== ""
            text: root.event?.headline ?? ""
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            color: root.ink
            font { family: Theme.font.family; pixelSize: 18; weight: Font.Medium }
            lineHeight: 1.1
            opacity: root.ease(root.step(0.05))
            transform: Translate { y: (1 - root.ease(root.step(0.05))) * 8 }
        }

        // The glass card: lighter than the island, with a soft rim
        // (Material: a tonal layer, no rim)
        Squircle {
            id: card
            width: parent.width
            height: cardRow.implicitHeight + 24
            radius: 20
            color: Theme.material ? IslandInk.dim(0.1, root.inverse) : Qt.rgba(1, 1, 1, 0.14)
            border.width: Theme.material ? 0 : 1
            border.color: Qt.rgba(1, 1, 1, 0.22)
            opacity: root.ease(root.step(0.2))
            transform: Translate { y: (1 - root.ease(root.step(0.2))) * 18 }
            // Top highlight, like light catching the glass edge
            Rectangle {
                visible: !Theme.material
                anchors { left: parent.left; right: parent.right; top: parent.top; margins: 1 }
                height: parent.height / 2
                radius: parent.radius - 1
                gradient: Gradient {
                    GradientStop { position: 0; color: Qt.rgba(1, 1, 1, 0.08) }
                    GradientStop { position: 1; color: Qt.rgba(1, 1, 1, 0) }
                }
            }

            Item {
                id: cardRow
                x: 12
                y: 12
                width: parent.width - 24
                implicitHeight: Math.max(avatar.height, texts.implicitHeight)

                // Avatar: a picture, or initials on grey, with the app's badge
                Item {
                    id: avatar
                    width: 46
                    height: 46
                    scale: 0.6 + 0.4 * root.ease(root.step(0.3))
                    Rectangle {
                        visible: !Theme.material
                        anchors.fill: parent
                        radius: width / 2
                        color: "#8E8E93"
                        clip: true
                        Text {
                            anchors.centerIn: parent
                            visible: !root.event?.card?.avatar
                            text: root.event?.card?.initials ?? ""
                            color: "white"
                            font { family: Theme.font.family; pixelSize: 17; weight: Font.DemiBold }
                        }
                        Image {
                            anchors.fill: parent
                            visible: !!root.event?.card?.avatar
                            source: Theme.material ? "" : root.event?.card?.avatar ?? ""
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(92, 92)
                            asynchronous: true
                        }
                    }
                    // Material: the picture (or initials) in a cookie
                    Loader {
                        anchors.fill: parent
                        active: Theme.material
                        sourceComponent: Item {
                            ShapedArt {
                                anchors.fill: parent
                                url: root.event?.card?.avatar ?? ""
                                sourceSize: Qt.size(92, 92)
                                retainWhileLoading: false
                                shape: "cookie9"
                                animated: false
                                color: Theme.m3.tertiaryContainer
                            }
                            Text {
                                anchors.centerIn: parent
                                visible: !root.event?.card?.avatar
                                text: root.event?.card?.initials ?? ""
                                color: Theme.m3.onTertiaryContainer
                                font { family: Theme.font.family; pixelSize: 17; weight: Font.Medium }
                            }
                        }
                    }
                    AppIcon {
                        visible: (root.event?.card?.appIcon ?? "") !== ""
                        anchors { right: parent.right; bottom: parent.bottom; rightMargin: -3; bottomMargin: -3 }
                        size: 20
                        source: root.event?.card?.appIcon ?? ""
                    }
                }

                Column {
                    id: texts
                    anchors { left: avatar.right; leftMargin: 12; right: parent.right }
                    spacing: 2
                    Item {
                        width: parent.width
                        height: nameText.implicitHeight
                        Text {
                            id: nameText
                            width: parent.width - timeText.implicitWidth - 8
                            textFormat: Text.PlainText
                            text: root.event?.card?.name ?? ""
                            elide: Text.ElideRight
                            color: root.ink
                            font { family: Theme.font.family; pixelSize: 15; weight: Font.DemiBold }
                        }
                        Text {
                            id: timeText
                            anchors.right: parent.right
                            text: root.event?.card?.time ?? ""
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                    Text {
                        width: parent.width
                        textFormat: Text.PlainText
                        text: root.event?.card?.text ?? ""
                        wrapMode: Text.Wrap
                        maximumLineCount: 3
                        elide: Text.ElideRight
                        color: IslandInk.dim(0.85, root.inverse)
                        font { family: Theme.font.family; pixelSize: 14 }
                        opacity: root.ease(root.step(0.35))
                    }
                    // The notification's own actions (Reply, OK…), up to two
                    Row {
                        visible: buttons.count > 0
                        topPadding: 6
                        spacing: 6
                        opacity: root.ease(root.step(0.45))
                        Repeater {
                            id: buttons
                            model: (root.event?.card?.buttons ?? []).slice(0, 2)
                            Rectangle {
                                required property var modelData
                                width: buttonText.implicitWidth + 24
                                height: Theme.material ? 32 : 28
                                radius: height / 2
                                color: Theme.material ? root.tonalColor(buttonMouse) : buttonMouse.pressed ? Qt.rgba(1, 1, 1, 0.3) : buttonMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.24) : Qt.rgba(1, 1, 1, 0.16)
                                Text {
                                    id: buttonText
                                    anchors.centerIn: parent
                                    text: parent.modelData.text
                                    color: Theme.material ? IslandInk.onTonal(root.inverse) : root.ink
                                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                                }
                                MouseArea {
                                    id: buttonMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        const m = parent.modelData;
                                        root.act(() => m.invoke?.());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // The whole card opens it (under the buttons, so they get their clicks)
            MouseArea {
                z: -1
                anchors.fill: parent
                cursorShape: root.event?.onAction ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: {
                    root.act(root.event?.onAction);
                }
            }
        }
    }

    // Close, over the glyph's (or the card's) top-left corner while hovered
    Rectangle {
        id: closeButton
        readonly property real targetY: root.isCard ? cardLayout.y + card.y - 6 : (52 - glyph.height) / 2 - 6
        x: -6
        y: targetY
        z: 10
        width: 20
        height: 20
        radius: 10
        visible: opacity > 0.01
        opacity: !root.isHud && !root.isRinging && !root.leaving && (hover.hovered || closeMouse.activeFocus) ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
        color: Theme.material ? root.tonalColor(closeMouse) : closeMouse.pressed ? "#5A5A5E" : closeMouse.containsMouse ? "#48484A" : "#3A3A3C"
        border.width: Theme.material ? 0 : 1
        border.color: Qt.rgba(1, 1, 1, 0.2)
        Icon {
            anchors.centerIn: parent
            name: "x"
            bold: true
            size: 10
            color: Theme.material ? IslandInk.onTonal(root.inverse) : root.ink
        }
        MouseArea {
            id: closeMouse
            anchors { fill: parent; margins: -4 } // a 28 px target
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.leave()
        }
        Accessible.role: Accessible.Button
        Accessible.name: "Dismiss alert"
    }
}
