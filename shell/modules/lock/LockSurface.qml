import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.glass
import qs.services

// One screen of the lock (Settings → Lock Screen):
//   iOS    the date and a large clock, a row of small widgets (battery,
//          weather, next event), Now Playing with controls, recent
//          notifications, and the password field at the bottom
//   macOS  the date and a large clock up top; avatar, name and the password
//          field at the bottom (Now Playing above them while music plays)
// Cards and the field are glass, frost or solid (or follow the system), and
// the text is light, dark, or picked from the wallpaper.
WlSessionLockSurface {
    id: surface

    required property var lockScope
    // Glass host contract (see Glass.qml)
    readonly property var backdrop: backdropItem.texture
    readonly property real screenX: 0
    readonly property real screenY: 0

    readonly property var cfg: Config.options.lock
    readonly property bool ios: cfg.layout !== "macos"
    readonly property string material: cfg.material === "system" ? (Theme.glass.tier === "liquid" ? "glass" : Theme.glass.tier === "frosted" ? "frost" : "solid") : cfg.material
    readonly property bool darkInk: cfg.text === "dark" || (cfg.text === "auto" && Wallpaper.topIsLight)
    readonly property color ink: darkInk ? "#18181B" : "#FFFFFF"
    readonly property color ink2: darkInk ? Qt.rgba(0.09, 0.09, 0.11, 0.66) : Qt.rgba(1, 1, 1, 0.72)

    color: "black"

    // ── StandBy: charging and left alone on the lock screen ─────────────
    // (shared by every screen: Lock.qml)
    readonly property bool standby: lockScope.standby
    function woke() {
        lockScope.woke();
    }
    // lock.standbyScreens: a screen left out stays dark during StandBy
    readonly property bool standbyHere: cfg.standbyScreens.length === 0 || cfg.standbyScreens.includes(screen?.name ?? "")

    // Lock → StandBy (lock.standbyTransition): `sb` runs 0 → 1. morph: the
    // lock's clock flies into StandBy's spot, thinning and dimming on the
    // way (two copies cross-fade, one per weight); fade: StandBy fades in.
    readonly property string sbMode: cfg.standbyTransition
    property real sb: standby ? 1 : 0
    Behavior on sb {
        enabled: surface.sbMode !== "none"
        NumberAnimation { duration: surface.sbMode === "morph" ? 650 : 400; easing.type: Easing.InOutCubic }
    }
    readonly property bool morphOn: sbMode === "morph" && standbyHere && standbyView.view === 0
    readonly property bool morphing: morphOn && sb > 0 && sb < 1
    function ramp(a, b) { // sb mapped to 0..1 over [a, b], smoothstepped
        const t = Math.max(0, Math.min(1, (sb - a) / (b - a)));
        return t * t * (3 - 2 * t);
    }

    // Unlock (lock.unlockTransition, run by Lock.qml): the content lifts
    // away, growing a little and fading; with zoom the wallpaper leans in
    // too, matching where the desktop starts its settle (LockFx)
    readonly property bool zoomOut: cfg.unlockTransition === "zoom"
    property real exit: lockScope.leaving ? 1 : 0
    Behavior on exit { NumberAnimation { duration: surface.lockScope.leaveMs; easing.type: Easing.OutCubic } }
    // Keep the screen on while StandBy shows (like iPhone on its charger)
    IdleInhibitor {
        window: surface
        enabled: surface.standby
    }

    Backdrop {
        id: backdropItem
        screen: surface.screen
    }

    Image {
        id: wallpaper
        anchors.fill: parent
        // Unlocking it leans in to where the desktop's wallpaper starts
        // (LockFx); after a zoomed lock it settles from there
        scale: 1 + 0.05 * Math.max(surface.zoomOut ? surface.exit : 0, surface.enter)
        source: Wallpaper.source
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(width, height)
        asynchronous: true
    }
    // Frost: the wallpaper blurred once, shown through the cards
    Item {
        id: frosted
        anchors.fill: parent
        visible: false
        layer.enabled: surface.material === "frost"
        Image {
            anchors.fill: parent
            source: surface.material === "frost" ? Wallpaper.source : ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(parent.width / 4, parent.height / 4)
            asynchronous: true
            layer.enabled: true
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 1
                blurMax: 32
                saturation: 0.2
            }
        }
    }

    // Lock (lock.lockTransition): the lock covers the screen at once (Lock.qml
    // never leaves an unlocked moment for an animation), and the transition
    // plays inside it: zoom, the wallpaper settles from a little larger while
    // the clock and cards rise into place; fade, they only fade in; none,
    // everything is there at once
    readonly property string lockFx: cfg.lockTransition
    property real reveal: lockFx === "none" ? 1 : 0
    property real enter: 0
    Component.onCompleted: {
        if (lockFx === "zoom") {
            enter = 1;
            settleIn.start();
        }
        reveal = 1;
    }
    NumberAnimation { id: settleIn; target: surface; property: "enter"; to: 0; duration: 450; easing.type: Easing.OutCubic }
    Behavior on reveal {
        enabled: surface.lockFx !== "none"
        NumberAnimation { duration: Theme.motion.large; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized }
    }

    SystemClock { id: clock; precision: SystemClock.Minutes }

    // A card's background in the chosen material
    component Plate: Item {
        id: plate
        property real radius: 20
        Glass {
            visible: surface.material === "glass"
            anchors.fill: parent
            anchors.margins: -shadowMargin
            shadowMargin: 14
            radius: plate.radius
            forceLiquid: true
        }
        ClippingRectangle {
            visible: surface.material !== "glass"
            anchors.fill: parent
            radius: plate.radius
            color: surface.material === "solid" ? (surface.darkInk ? "#FFFFFF" : "#232327") : "transparent"
            border.width: 1
            border.color: surface.darkInk ? Qt.rgba(0, 0, 0, 0.08) : Qt.rgba(1, 1, 1, 0.12)
            ShaderEffectSource {
                visible: surface.material === "frost"
                anchors.fill: parent
                sourceItem: frosted
                readonly property point at: plate.mapToItem(frosted, 0, 0)
                sourceRect: Qt.rect(at.x, at.y, plate.width, plate.height)
                live: false
            }
            Rectangle {
                visible: surface.material === "frost"
                anchors.fill: parent
                color: surface.darkInk ? Qt.rgba(1, 1, 1, 0.42) : Qt.rgba(0, 0, 0, 0.28)
            }
        }
    }

    // Everything but the wallpaper and StandBy (lifts away on unlock)
    Item {
        id: content
        anchors.fill: parent
        opacity: 1 - surface.exit
        scale: surface.zoomOut ? 1 + 0.08 * surface.exit : 1
        visible: !(surface.standby && surface.sb >= 1)

        // ── Date and time ───────────────────────────────────────────────────
        Column {
            id: header
            anchors.horizontalCenter: parent.horizontalCenter
            y: parent.height * (surface.ios ? 0.07 : 0.1) + (surface.lockFx === "zoom" ? (1 - surface.reveal) * -20 : 0)
            opacity: surface.reveal
            spacing: 0
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Qt.formatDate(clock.date, "dddd d MMMM")
                color: surface.ink2
                font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold }
            }
            Text {
                id: lockClock
                anchors.horizontalCenter: parent.horizontalCenter
                // The morph to StandBy flies a copy of it (below)
                opacity: surface.morphOn && surface.sb > 0 ? 0 : 1
                // 12-hour, like the menu bar (no AM/PM at this size)
                text: ((clock.date.getHours() + 11) % 12 + 1) + ":" + String(clock.date.getMinutes()).padStart(2, "0")
                color: surface.darkInk ? Qt.rgba(0.09, 0.09, 0.11, 0.85) : Qt.rgba(1, 1, 1, 0.85)
                font { family: Theme.font.family; pixelSize: surface.ios ? 168 : 150; weight: Font.Bold; letterSpacing: -5; features: { "tnum": 1 } }
                layer.enabled: true
                layer.effect: MultiEffect {
                    shadowEnabled: !surface.darkInk
                    shadowColor: Qt.rgba(0, 0, 0, 0.3)
                    shadowBlur: 0.6
                    shadowVerticalOffset: 2
                }
            }
        }

        // ── iOS: small widgets under the clock ──────────────────────────────
        Row {
            id: complications
            visible: surface.ios
            anchors { horizontalCenter: parent.horizontalCenter; top: header.bottom; topMargin: 4 }
            opacity: surface.reveal
            spacing: 16
            readonly property var next: Calendar.upcoming(1)[0] ?? null
            Repeater {
                model: [
                    { icon: "", battery: true, text: Island.hasBattery ? Math.round(Island.level * 100) + "%" : "", on: Island.hasBattery },
                    { icon: Weather.icon, text: Weather.temp + "°", on: Weather.ready },
                    { icon: "calendar-blank", text: complications.next ? (complications.next.allDay ? "Today" : Qt.formatTime(complications.next.start, "h:mm")) : "", on: !!complications.next },
                    { icon: "list-checks", text: String(Todos.list.filter(t => !t.done).length), on: Todos.list.some(t => !t.done) }
                ].filter(c => c.on)
                Plate {
                    required property var modelData
                    width: 64
                    height: 64
                    radius: 32
                    Column {
                        anchors.centerIn: parent
                        spacing: 1
                        Icon {
                            visible: !parent.parent.modelData.battery
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: parent.parent.modelData.icon
                            fill: true
                            size: 18
                            color: surface.ink
                        }
                        BatteryGlyph {
                            visible: !!parent.parent.modelData.battery
                            anchors.horizontalCenter: parent.horizontalCenter
                            size: 11
                            level: Island.level
                            charging: Island.charging && Island.level < 1
                            plugged: Island.charging && Island.level >= 1
                            color: surface.ink
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: parent.parent.modelData.text
                            color: surface.ink
                            font { family: Theme.font.family; pixelSize: 13; weight: Font.DemiBold; features: { "tnum": 1 } }
                        }
                    }
                }
            }
        }

        // ── Now Playing and notifications ───────────────────────────────────
        Column {
            id: cards
            anchors.horizontalCenter: parent.horizontalCenter
            y: surface.ios ? complications.y + complications.height + 40 : login.y - height - 24
            width: 420
            spacing: 10
            opacity: surface.reveal

            Plate {
                id: player
                readonly property var p: Island.player
                visible: !!p && (p.trackTitle ?? "") !== ""
                width: parent.width
                height: 84
                radius: 22
                ClippingRectangle {
                    id: art
                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: 60
                    height: 60
                    radius: 12
                    color: Qt.rgba(1, 1, 1, 0.12)
                    Image {
                        anchors.fill: parent
                        source: player.p?.trackArtUrl ?? ""
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(120, 120)
                        asynchronous: true
                    }
                }
                Column {
                    anchors { left: art.right; leftMargin: 12; right: buttons.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    Text {
                        width: parent.width
                        text: player.p?.trackTitle ?? ""
                        elide: Text.ElideRight
                        color: surface.ink
                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        text: player.p?.trackArtist ?? ""
                        elide: Text.ElideRight
                        color: surface.ink2
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                }
                Row {
                    id: buttons
                    anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                    IconButton { icon: "skip-back"; fill: true; iconColor: surface.ink; width: 36; height: 36; iconSize: 18; onClicked: player.p?.previous() }
                    IconButton { icon: Island.musicPlaying ? "pause" : "play"; fill: true; iconColor: surface.ink; width: 40; height: 40; iconSize: 22; onClicked: player.p?.togglePlaying() }
                    IconButton { icon: "skip-forward"; fill: true; iconColor: surface.ink; width: 36; height: 36; iconSize: 18; onClicked: player.p?.next() }
                }
            }

            // Recent notifications (iOS layout): who and what; the text only
            // with "Show previews" on
            readonly property var recent: surface.ios ? Notifications.list.slice().reverse().slice(0, 3) : []
            Repeater {
                model: cards.recent
                Plate {
                    id: note
                    required property var modelData
                    width: cards.width
                    height: 64
                    radius: 18
                    AppIcon {
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        size: 34
                        styled: false
                        source: Notifications.iconFor(note.modelData)
                    }
                    Column {
                        anchors { left: parent.left; leftMargin: 56; right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                        Item {
                            width: parent.width
                            height: appText.implicitHeight
                            Text {
                                id: appText
                                width: parent.width - when.implicitWidth - 8
                                text: surface.cfg.previews ? (note.modelData.summary || note.modelData.appName) : (note.modelData.appName || "Notification")
                                elide: Text.ElideRight
                                color: surface.ink
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                            }
                            Text {
                                id: when
                                anchors.right: parent.right
                                text: Notifications.timeLabel(note.modelData, clock.date)
                                color: surface.ink2
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                        Text {
                            width: parent.width
                            text: surface.cfg.previews ? (note.modelData.body ?? "").replace(/<[^>]*>/g, "") : "Notification"
                            elide: Text.ElideRight
                            maximumLineCount: 1
                            color: surface.ink2
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                }
            }
            Text {
                visible: surface.ios && Notifications.list.length > 3
                anchors.horizontalCenter: parent.horizontalCenter
                text: (Notifications.list.length - 3) + " more"
                color: surface.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
        }

        // ── Sign in ─────────────────────────────────────────────────────────
        Column {
            id: login
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            anchors.bottomMargin: parent.height * (surface.ios ? 0.06 : 0.1)
            opacity: surface.reveal
            spacing: 10

            Rectangle {
                visible: !surface.ios
                anchors.horizontalCenter: parent.horizontalCenter
                width: 64
                height: 64
                radius: 32
                color: surface.darkInk ? Qt.rgba(0, 0, 0, 0.12) : Qt.rgba(1, 1, 1, 0.18)
                Icon { anchors.centerIn: parent; name: "user"; fill: true; size: 32; color: surface.ink }
            }
            Text {
                visible: !surface.ios
                anchors.horizontalCenter: parent.horizontalCenter
                text: Quickshell.env("USER") ?? ""
                color: surface.ink
                font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
            }

            Item {
                id: fieldBox
                anchors.horizontalCenter: parent.horizontalCenter
                width: 240
                height: 36

                property real shake: 0
                SequentialAnimation {
                    id: shakeAnim
                    NumberAnimation { target: fieldBox; property: "shake"; to: 10; duration: 50 }
                    NumberAnimation { target: fieldBox; property: "shake"; to: -10; duration: 80 }
                    NumberAnimation { target: fieldBox; property: "shake"; to: 6; duration: 70 }
                    NumberAnimation { target: fieldBox; property: "shake"; to: 0; duration: 60 }
                }
                Connections {
                    target: surface.lockScope
                    function onErrorChanged() {
                        if (surface.lockScope.error !== "")
                            shakeAnim.restart();
                    }
                }

                Plate {
                    id: field
                    x: fieldBox.shake
                    width: parent.width
                    height: parent.height
                    radius: 18

                    TextInput {
                        id: input
                        x: 16
                        width: parent.width - 32 - 24
                        height: parent.height
                        verticalAlignment: TextInput.AlignVCenter
                        echoMode: TextInput.Password
                        passwordCharacter: "●"
                        enabled: !surface.lockScope.busy
                        focus: true
                        color: surface.ink
                        font { family: Theme.font.family; pixelSize: Theme.font.title; letterSpacing: 1 }
                        text: surface.lockScope.password
                        onTextChanged: surface.lockScope.password = text
                        onAccepted: surface.lockScope.submit()
                        Component.onCompleted: forceActiveFocus()

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: input.text === ""
                            text: "Enter Password"
                            color: surface.ink2
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                    }
                    Icon {
                        x: parent.width - 30
                        y: 6
                        width: 24
                        height: 24
                        visible: !surface.lockScope.busy
                        name: "arrow-circle-right"
                        fill: true
                        size: 22
                        color: surface.ink
                        opacity: input.text === "" ? 0.4 : 1
                        MouseArea { anchors.fill: parent; onClicked: surface.lockScope.submit() }
                    }
                    Icon {
                        x: parent.width - 30
                        y: 6
                        width: 24
                        height: 24
                        visible: surface.lockScope.busy
                        name: "circle-notch"
                        size: 22
                        color: surface.ink
                        RotationAnimation on rotation { running: surface.lockScope.busy; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
                    }
                }
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: surface.lockScope.error
                visible: text !== ""
                color: surface.ink
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
        }
    }

    // Over everything while it shows; a key or click brings the lock back
    // (arrows, swipes and the wheel change the view instead); moving the
    // mouse does too, past a little jitter (lock.standbyWakeOnMouse)
    StandBy {
        id: standbyView
        anchors.fill: parent
        lockScope: surface.lockScope
        blank: !surface.standbyHere
        visible: opacity > 0.01
        opacity: surface.sb
        clockFade: surface.morphOn && surface.sb < 1 ? 0 : 1
        dateFade: surface.morphOn ? surface.ramp(0.6, 1) : 1
        focus: surface.standby
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Left || e.key === Qt.Key_Right) {
                standbyView.step(e.key === Qt.Key_Right ? 1 : -1);
            } else {
                surface.woke();
                input.forceActiveFocus();
            }
            e.accepted = true;
        }
        MouseArea {
            anchors.fill: parent
            enabled: surface.standby
            hoverEnabled: true
            property point from: Qt.point(-1, -1)
            onEnabledChanged: from = Qt.point(-1, -1)
            onPositionChanged: m => {
                if (!surface.cfg.standbyWakeOnMouse || surface.sb < 1)
                    return;
                if (from.x < 0) {
                    from = Qt.point(m.x, m.y);
                } else if (Math.hypot(m.x - from.x, m.y - from.y) > 24) {
                    surface.woke();
                    input.forceActiveFocus();
                }
            }
            onClicked: {
                surface.woke();
                input.forceActiveFocus();
            }
            onWheel: w => standbyView.step(w.angleDelta.x + w.angleDelta.y < 0 ? 1 : -1)
        }
    }

    // ── The morph's flying clock ────────────────────────────────────────
    // Centre and size go from the lock's clock to StandBy's; the bold copy
    // hands over to one in StandBy's weight and brightness, which ends
    // exactly where (and as) StandBy's own clock is, so nothing pops.
    readonly property point flyFrom: Qt.point(header.x + lockClock.x + lockClock.width / 2, header.y + lockClock.y + lockClock.height / 2)
    readonly property point flyTo: Qt.point(standbyView.clockTarget.x + standbyView.clockTarget.width / 2, standbyView.clockTarget.y + standbyView.clockTarget.height / 2)
    readonly property real flyX: flyFrom.x + (flyTo.x - flyFrom.x) * sb
    readonly property real flyY: flyFrom.y + (flyTo.y - flyFrom.y) * sb
    readonly property real sizeRatio: standbyView.clockSize / lockClock.font.pixelSize
    Text {
        visible: surface.morphing
        x: surface.flyX - width / 2
        y: surface.flyY - height / 2
        scale: 1 + (surface.sizeRatio - 1) * surface.sb
        opacity: 1 - surface.ramp(0.3, 0.8)
        text: lockClock.text
        color: lockClock.color
        font: lockClock.font
    }
    Text {
        visible: surface.morphing
        x: surface.flyX - width / 2
        y: surface.flyY - height / 2
        scale: 1 / surface.sizeRatio + (1 - 1 / surface.sizeRatio) * surface.sb
        opacity: surface.ramp(0.2, 0.75) * standbyView.clockOpacity
        text: lockClock.text
        color: standbyView.ink
        font { family: Theme.font.family; pixelSize: standbyView.clockSize; weight: standbyView.clockWeight; letterSpacing: -standbyView.clockSize * 0.03; features: { "tnum": 1 } }
    }
    // Any use of the lock screen keeps StandBy away
    Connections {
        target: surface.lockScope
        function onPasswordChanged() { surface.woke(); }
    }
    MouseArea {
        anchors.fill: parent
        z: -1
        hoverEnabled: true
        acceptedButtons: Qt.NoButton
        onPositionChanged: if (!surface.standby) surface.woke()
    }
}
