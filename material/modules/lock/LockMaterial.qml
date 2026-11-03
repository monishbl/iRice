import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.services

// One screen of the lock in the Material look, as Android 16's lock screen
// (LockSurface loads it in place of the Glass content; the wallpaper,
// StandBy and every rule about locking stay LockSurface's and Lock.qml's):
//   ios    the big clock (hours over minutes) with the date, weather and a
//          smartspace line (next event, open to-dos) under it; on a wide
//          screen, like a Pixel tablet, notifications and Now Playing in a
//          column on the right; the battery in the top corner
//   macos  the clock centred; avatar, name and the field at the bottom, Now
//          Playing above them
// Notifications are a shelf of app icons, or compact cards with "Show
// previews" on. The field is a pill under the avatar (a cookie); the
// fingerprint reader is a round target under it that pulses as it listens,
// shakes red on a miss and morphs to a check on a match; face unlock spins
// Material's loading indicator in the lock icon at the top while it looks.
// Entrance (lock.lockTransition): zoom is Android's light reveal, a circle
// opening from the centre while the clock thickens from its thin
// always-on weight; fade only fades. Unlock (lock.unlockTransition): the top
// lifts and the bottom drops as they fade (zoom), or they only fade.
Item {
    id: root

    required property var host // the LockSurface
    readonly property var lockScope: host.lockScope
    readonly property var cfg: host.cfg
    readonly property var m3: Theme.m3
    readonly property bool ios: host.ios
    // Where notifications and Now Playing go (lock.notifications):
    //   shelf   under the clock: the apps' icons and the newest as a card;
    //           Now Playing as a slim player over the account pill
    //   column  beside the clock, like a tablet: the player and the cards
    //   icons   under the clock, only the apps' icons; the player as shelf
    readonly property string style: cfg.notifications ?? "shelf"
    // (column needs a wide screen; on a narrow one it's shelf)
    readonly property bool split: style === "column" && width > height

    // Ink over the wallpaper: Material's fixed tones (the same in light and
    // dark), dark on a light wallpaper (lock.text)
    readonly property bool darkInk: host.darkInk
    readonly property color clockInk: darkInk ? m3.onPrimaryFixed : m3.primaryFixed
    readonly property color clockInk2: darkInk ? m3.onPrimaryFixedVariant : m3.primaryFixedDim
    readonly property color ink: darkInk ? m3.onSecondaryFixed : m3.secondaryFixed
    readonly property color ink2: darkInk ? m3.onSecondaryFixedVariant : m3.secondaryFixedDim

    // The clock, as the owner's lock → StandBy morph needs it
    readonly property int clockSize: Math.round(split ? Math.min(height * 0.2, width * 0.12) : Math.min(height * (ios ? 0.17 : 0.15), width * 0.2))
    readonly property real clockWeight: cfg.clockWeight ?? 640
    readonly property real clockWidth: cfg.clockWidth ?? 112
    readonly property rect clockRect: Qt.rect(header.x + clock.x, header.y + clock.y, clock.width, clock.height)
    // Only the large stacked clock flies into StandBy (LockSurface fades
    // the rest)
    readonly property bool morphable: clock.face === "stacked" && smallT === 0

    // The clock stays where it is (lock.clockSize large, or small in the
    // top corner); notifications and the player find room around it
    readonly property bool hasContent: media.visible || cards.recent.length > 0
    readonly property bool compact: cfg.clockSize === "small"
    property real smallT: compact ? 1 : 0
    Behavior on smallT {
        SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.002 }
    }
    // (the small clock and what's under it keep to the left edge)
    readonly property bool leftAligned: split || smallT > 0.5
    readonly property date now: wallClock.date

    function focusField() {
        input.forceActiveFocus();
    }
    Component.onCompleted: input.forceActiveFocus()

    SystemClock { id: wallClock; precision: SystemClock.Minutes }

    // ── Motion ──────────────────────────────────────────────────────────
    readonly property real reveal: host.reveal
    readonly property bool zoomIn: host.lockFx === "zoom"
    readonly property real rise: zoomIn ? (1 - reveal) * 32 : 0
    readonly property real part: host.zoomOut ? height * 0.05 * host.exit : 0

    // Everything (hidden under StandBy once it's fully up)
    Item {
        id: content
        anchors.fill: parent
        opacity: 1 - root.host.exit
        visible: !(root.host.standby && root.host.sb >= 1)

        // ── Lock icon (the face camera's loading indicator) ─────────────
        Item {
            id: lockIcon
            x: (root.width - width) / 2
            y: 24 - root.part
            width: 40
            height: 40
            opacity: root.reveal
            Icon {
                anchors.centerIn: parent
                visible: !root.lockScope.faceLooking
                name: "lock"
                fill: true
                size: 22
                color: root.ink
            }
            Spinner {
                anchors.centerIn: parent
                visible: root.lockScope.faceLooking
                size: 36
                color: root.ink
            }
            Accessible.role: Accessible.StaticText
            Accessible.name: root.lockScope.faceLooking ? "Looking for your face" : "Locked"
        }

        // ── Battery, in the corner like Android's status bar ────────────
        Row {
            visible: root.ios && Island.hasBattery
            anchors { right: parent.right; rightMargin: 24 }
            y: 30 - root.part
            spacing: 4
            opacity: root.reveal
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: Island.charging ? "battery-charging" : Island.level <= 0.2 ? "battery-low" : "battery-high"
                fill: true
                size: 18
                color: root.ink
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Math.round(Island.level * 100) + "%"
                color: root.ink
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
            }
        }

        // ── Clock, date and smartspace ──────────────────────────────────
        Column {
            id: header
            // (large: centred, or on the left of a wide screen; small: the
            // top left corner)
            readonly property real largeX: root.split ? Math.round(root.width * 0.08) : Math.round((root.width - width) / 2)
            readonly property real largeY: Math.round(root.height * (root.split ? 0.14 : root.ios ? 0.09 : 0.11))
            x: largeX + (Math.max(48, Math.round(root.width * 0.04)) - largeX) * root.smallT
            y: largeY + (72 - largeY) * root.smallT + root.rise - root.part
            spacing: 20 - 8 * root.smallT
            opacity: root.reveal

            LockClock {
                id: clock
                anchors.horizontalCenter: root.leftAligned ? undefined : parent.horizontalCenter
                width: implicitWidth
                height: implicitHeight
                face: root.cfg.clockFace ?? "stacked"
                time: wallClock.date
                size: root.clockSize
                smallSize: Math.round(Math.min(72, root.height * 0.07))
                small: root.smallT
                // (the light reveal: thin, as if waking from always-on)
                weight: root.zoomIn ? 160 + (root.clockWeight - 160) * root.reveal : root.clockWeight
                widthAxis: root.clockWidth
                round: root.cfg.clockRound ?? true
                color: root.clockInk
                color2: root.clockInk2
                color3: root.darkInk ? root.m3.onTertiaryFixed : root.m3.tertiaryFixed
                align: root.leftAligned ? Text.AlignLeft : Text.AlignHCenter
                ticks: !root.host.standby
                // The morph to StandBy flies a copy of it (LockSurface)
                opacity: root.host.morphOn && root.host.sb > 0 ? 0 : 1
            }

            Column {
                anchors.horizontalCenter: root.leftAligned ? undefined : parent.horizontalCenter
                x: root.leftAligned ? 4 : 0
                spacing: 8
                // Date and weather
                Row {
                    anchors.horizontalCenter: root.leftAligned ? undefined : parent.horizontalCenter
                    spacing: 12
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: Qt.formatDate(wallClock.date, "dddd d MMMM")
                        color: root.ink
                        font { family: Theme.font.family; pixelSize: 18; weight: Font.Medium }
                    }
                    Row {
                        visible: root.ios && Weather.ready
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: Weather.icon
                            fill: true
                            size: 20
                            color: root.ink
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: Weather.temp + "°"
                            color: root.ink
                            font { family: Theme.font.family; pixelSize: 18; weight: Font.Medium; features: { "tnum": 1 } }
                        }
                    }
                }
                // Smartspace: the next event and the open to-dos
                Row {
                    id: smartspace
                    visible: root.ios && (next !== null || todo > 0)
                    anchors.horizontalCenter: root.leftAligned ? undefined : parent.horizontalCenter
                    spacing: 16
                    readonly property var next: Calendar.upcoming(1)[0] ?? null
                    readonly property int todo: Todos.list.filter(t => !t.done).length
                    Row {
                        visible: smartspace.next !== null
                        spacing: 6
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: "calendar-blank"
                            fill: true
                            size: 18
                            color: root.ink2
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.min(implicitWidth, 320)
                            elide: Text.ElideRight
                            text: !smartspace.next ? "" : (smartspace.next.title || "Event") + " · " + (smartspace.next.allDay ? "Today" : Qt.formatTime(smartspace.next.start, "h:mm AP"))
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
                        }
                    }
                    Row {
                        visible: smartspace.todo > 0
                        spacing: 6
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: "list-checks"
                            size: 18
                            color: root.ink2
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: smartspace.todo + (smartspace.todo === 1 ? " to-do" : " to-dos")
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
                        }
                    }
                }
            }
        }

        // ── Now Playing ─────────────────────────────────────────────────
        // Its players in Material's carousel: beside the clock (column), or
        // a slim card over the account pill
        MediaCarousel {
            id: media
            visible: !!Media.player && (Media.player.trackTitle ?? "") !== ""
            width: cards.width
            height: 80
            x: cards.x
            y: root.split ? header.y : unlock.y - height - 20
            peekRadius: Theme.radius.tile
            opacity: root.reveal
            card: Component {
                LockPlayer {}
            }
        }

        // ── Notifications ───────────────────────────────────────────────
        Column {
            id: cards
            width: root.split ? Math.min(380, root.width * 0.3) : 380
            x: root.split ? Math.round(root.width * 0.72 - width / 2) : Math.round((root.width - width) / 2)
            // (moving with the clock on unlock)
            y: root.split ? header.y + (media.visible ? media.height + 12 : 0) : header.y + header.height + 32
            spacing: 12
            opacity: root.reveal

            // Newest first: who sent them as a shelf of app icons (shelf,
            // icons), and compact cards in a segmented group (corners 24
            // outside, 6 inside, 2 apart): the newest (shelf) or the latest
            // four (column). Without "Show previews", a card says only
            // which app it's from.
            readonly property var recent: Notifications.list.slice().reverse()
            readonly property var apps: {
                const seen = {}, out = [];
                for (let i = 0; i < recent.length && out.length < 6; i++) {
                    const n = recent[i];
                    const key = n.appName || n.desktopEntry || String(n.id);
                    if (seen[key])
                        continue;
                    seen[key] = true;
                    out.push(n);
                }
                return out;
            }

            Rectangle {
                id: shelf
                visible: !root.split && cards.recent.length > 0
                x: root.split ? 0 : (parent.width - width) / 2
                width: shelfRow.implicitWidth + 16
                height: 52
                radius: height / 2
                color: root.m3.surfaceContainerHigh
                Accessible.role: Accessible.StaticText
                Accessible.name: cards.recent.length + (cards.recent.length === 1 ? " notification" : " notifications")
                Row {
                    id: shelfRow
                    anchors.centerIn: parent
                    spacing: 4
                    Repeater {
                        model: shelf.visible ? cards.apps : []
                        Rectangle {
                            id: shelfIcon
                            required property var modelData
                            width: 40
                            height: 40
                            radius: 20
                            color: root.m3.surfaceContainerHighest
                            AppIcon {
                                anchors.centerIn: parent
                                size: 26
                                styled: false
                                source: Notifications.iconFor(shelfIcon.modelData)
                            }
                        }
                    }
                    Text {
                        visible: cards.recent.length > cards.apps.length
                        anchors.verticalCenter: parent.verticalCenter
                        leftPadding: 6
                        rightPadding: 8
                        text: "+" + (cards.recent.length - cards.apps.length)
                        color: root.m3.onSurfaceVariant
                        font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                }
            }

            Column {
                id: noteList
                visible: root.style !== "icons" && cards.recent.length > 0
                width: parent.width
                spacing: 2
                readonly property var shown: visible ? cards.recent.slice(0, root.split ? 4 : 1) : []
                Repeater {
                    model: noteList.shown
                    Rectangle {
                        id: note
                        required property var modelData
                        required property int index
                        readonly property int count: noteList.shown.length
                        width: parent.width
                        height: 72
                        color: root.m3.surfaceContainerHigh
                        topLeftRadius: index === 0 ? 24 : 6
                        topRightRadius: index === 0 ? 24 : 6
                        bottomLeftRadius: index === count - 1 ? 24 : 6
                        bottomRightRadius: index === count - 1 ? 24 : 6
                        Rectangle {
                            id: noteIcon
                            x: 16
                            anchors.verticalCenter: parent.verticalCenter
                            width: 40
                            height: 40
                            radius: 20
                            color: root.m3.surfaceContainerHighest
                            AppIcon {
                                anchors.centerIn: parent
                                size: 26
                                styled: false
                                source: Notifications.iconFor(note.modelData)
                            }
                        }
                        Column {
                            anchors { left: noteIcon.right; leftMargin: 12; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                            spacing: 2
                            Item {
                                width: parent.width
                                height: noteTitle.implicitHeight
                                Text {
                                    id: noteTitle
                                    width: parent.width - noteWhen.implicitWidth - 8
                                    text: (root.cfg.previews ? note.modelData.summary : "") || note.modelData.appName || "Notification"
                                    elide: Text.ElideRight
                                    color: root.m3.onSurface
                                    font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
                                }
                                Text {
                                    id: noteWhen
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: Notifications.timeLabel(note.modelData, wallClock.date)
                                    color: root.m3.onSurfaceVariant
                                    font { family: Theme.font.family; pixelSize: 12 }
                                }
                            }
                            Text {
                                width: parent.width
                                text: root.cfg.previews ? (note.modelData.body ?? "").replace(/<[^>]*>/g, "") || (note.modelData.appName ?? "") : "New notification"
                                elide: Text.ElideRight
                                maximumLineCount: 1
                                color: root.m3.onSurfaceVariant
                                font { family: Theme.font.family; pixelSize: 14 }
                            }
                        }
                    }
                }
            }
            Text {
                visible: root.split && cards.recent.length > 4
                x: 8
                text: (cards.recent.length - 4) + " more"
                color: root.ink
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
        }

        // ── Unlock: one pill ────────────────────────────────────────────
        // At rest: your picture, your name and, with a fingerprint reader
        // set up, its glyph at the end. A key or a mouse move springs it
        // open into the password field (the key typed is kept: the field
        // always has the keyboard); the glyph stays at the end, pulsing
        // while the reader listens, shaking on a miss, a check on a match.
        // 10 s untouched with nothing typed: back to the pill.
        Column {
            id: unlock
            x: Math.round((root.width - width) / 2)
            y: root.height - height - Math.round(root.height * 0.06) + root.part
            spacing: 12
            opacity: root.reveal

            // What went wrong and how to fix it, above the pill as Android's
            // bouncer has it, in error on a container so it reads on any
            // wallpaper (the pill stays put)
            Rectangle {
                visible: root.lockScope.error !== ""
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(errorRow.implicitWidth, 460) + 32
                height: errorRow.height + 20
                radius: errorText.lineCount > 1 ? 20 : height / 2
                color: root.m3.surfaceContainerHigh
                Row {
                    id: errorRow
                    anchors.centerIn: parent
                    spacing: 8
                    Icon {
                        name: "warning-circle"
                        fill: true
                        size: 18
                        color: root.m3.error
                    }
                    Text {
                        id: errorText
                        width: Math.min(implicitWidth, 460 - 26)
                        wrapMode: Text.WordWrap
                        text: root.lockScope.error
                        color: root.m3.error
                        font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
                    }
                }
            }

            Item {
                id: fieldBox
                anchors.horizontalCenter: parent.horizontalCenter
                readonly property bool fingered: Fingerprint.lockScreen || finger.matched
                // (closed: as wide as the name; open: the field's width)
                readonly property real restWidth: 8 + 40 + 12 + Math.min(nameText.implicitWidth, 240) + (fingered ? 12 + 40 + 8 : 24)
                width: Math.round(restWidth + (360 - restWidth) * root.openT)
                height: 56

                property real shake: 0
                // (a loose spring, ζ 0.15, k 1500: a quick decaying shake)
                SpringAnimation {
                    id: shakeAnim
                    target: fieldBox
                    property: "shake"
                    from: 18
                    to: 0
                    spring: 24
                    damping: 0.19
                    epsilon: 0.2
                }

                Rectangle {
                    id: pill
                    x: fieldBox.shake
                    width: parent.width
                    height: parent.height
                    radius: height / 2
                    color: root.m3.surfaceContainerHigh
                    border.width: root.open && (input.activeFocus || root.lockScope.error !== "") ? 2 : 0
                    border.color: root.lockScope.error !== "" ? root.m3.error : root.m3.primary

                    // Your picture (~/.face, or AccountsService's) in a cookie
                    Item {
                        id: avatar
                        x: 8
                        anchors.verticalCenter: parent.verticalCenter
                        width: 40
                        height: 40
                        Image {
                            id: avatarImage
                            anchors.fill: parent
                            source: root.avatarPath !== "" ? "file://" + root.avatarPath : ""
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(120, 120)
                            asynchronous: true
                        }
                        MaterialShape {
                            anchors.fill: parent
                            shape: "cookie9"
                            color: root.m3.primaryContainer
                            source: avatarImage.status === Image.Ready && avatarImage.source != "" ? avatarImage : null
                            // (a turn while the password is checked)
                            spin: root.lockScope.busy ? 40 : 0
                            Behavior on spin { SpringAnimation { spring: 3.2; damping: 0.27; epsilon: 0.05 } }
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: avatarImage.status !== Image.Ready || avatarImage.source == ""
                            name: "user"
                            fill: true
                            size: 18
                            color: root.m3.onPrimaryContainer
                        }
                    }

                    // Your name, closed; the field, open
                    Text {
                        id: nameText
                        x: avatar.x + avatar.width + 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(implicitWidth, 240)
                        elide: Text.ElideRight
                        text: root.fullName || (Quickshell.env("USER") ?? "")
                        color: root.m3.onSurface
                        opacity: 1 - root.openT
                        visible: opacity > 0.01
                        font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
                    }
                    TextInput {
                        id: input
                        x: avatar.x + avatar.width + 12
                        width: Math.max(0, (send.visible ? send.x : fingerSlot.x) - x - 8)
                        height: parent.height
                        verticalAlignment: TextInput.AlignVCenter
                        echoMode: TextInput.Password
                        passwordCharacter: "•"
                        enabled: !root.lockScope.busy
                        focus: true
                        clip: true
                        opacity: root.openT
                        color: root.m3.onSurface
                        selectionColor: root.m3.primary
                        selectedTextColor: root.m3.onPrimary
                        font { family: Theme.font.family; pixelSize: 18; letterSpacing: 2 }
                        text: root.lockScope.password
                        onTextChanged: {
                            root.lockScope.password = text;
                            root.wake();
                        }
                        onAccepted: root.lockScope.submit()
                        Keys.onPressed: e => {
                            root.wake();
                            e.accepted = false;
                        }
                        Accessible.name: "Password"

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: input.text === ""
                            text: "Enter password"
                            color: root.m3.onSurfaceVariant
                            font { family: Theme.font.family; pixelSize: 16 }
                        }
                    }

                    // Send: a circle, filled once there's something to send
                    Item {
                        id: send
                        x: fingerSlot.x - (fieldBox.fingered ? 4 : 0) - width
                        anchors.verticalCenter: parent.verticalCenter
                        width: 40
                        height: 40
                        readonly property bool ready: input.text !== ""
                        visible: root.openT > 0.5 && !root.lockScope.busy
                        opacity: ready ? 1 : 0.6
                        transform: sendPress.effects
                        ElasticControl { id: sendPress; pressed: sendMouse.pressed }
                        MaterialShape {
                            anchors.fill: parent
                            shape: "circle"
                            color: send.ready ? (sendMouse.containsMouse ? Qt.tint(root.m3.primary, Qt.alpha(root.m3.onPrimary, sendMouse.pressed ? 0.1 : 0.08)) : root.m3.primary) : Qt.alpha(root.m3.onSurface, 0.12)
                        }
                        Icon {
                            anchors.centerIn: parent
                            name: "arrow-right"
                            size: 20
                            color: send.ready ? root.m3.onPrimary : Qt.alpha(root.m3.onSurface, 0.38)
                        }
                        MouseArea {
                            id: sendMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: send.ready ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: root.lockScope.submit()
                        }
                        Accessible.role: Accessible.Button
                        Accessible.name: "Unlock"
                        Accessible.onPressAction: root.lockScope.submit()
                    }
                    Spinner {
                        x: send.x
                        anchors.verticalCenter: parent.verticalCenter
                        visible: root.lockScope.busy
                        size: 40
                    }

                    // The fingerprint glyph at the end (its place even while
                    // the reader isn't listening yet)
                    Item {
                        id: fingerSlot
                        x: pill.width - 8 - width
                        anchors.verticalCenter: parent.verticalCenter
                        width: fieldBox.fingered ? 40 : 0
                        height: 40
                        Item {
                            id: finger
                            anchors.fill: parent
                            visible: fieldBox.fingered
                            // The match (or any unlock) holds the check while the lock fades
                            property bool matched: false
                            readonly property string phase: matched || root.lockScope.leaving ? "match" : root.lockScope.fingerState
                            property real pulse: 1
                            property real shake: 0
                            scale: pulse
                            Accessible.role: Accessible.StaticText
                            Accessible.name: root.lockScope.fingerText || "Fingerprint"

                            onPhaseChanged: {
                                if (CoolDown.motionScale <= 0)
                                    return;
                                if (phase === "waiting") {
                                    pulseAnim.restart();
                                } else if (phase === "miss") {
                                    fingerShake.restart();
                                } else if (phase === "match") {
                                    pulseAnim.restart();
                                    ripple.restart();
                                }
                            }
                            // Listening: a subtle spring out and back
                            SpringAnimation {
                                id: pulseAnim
                                target: finger
                                property: "pulse"
                                from: 1.15
                                to: 1
                                spring: 6.08
                                damping: 0.5
                                epsilon: 0.002
                            }
                            SpringAnimation {
                                id: fingerShake
                                target: finger
                                property: "shake"
                                from: 8
                                to: 0
                                spring: 24
                                damping: 0.19
                                epsilon: 0.2
                            }
                            // Android's ripple from the sensor on a match
                            Rectangle {
                                id: rippleRing
                                anchors.centerIn: parent
                                property real grow: 0
                                width: 40 + grow * 240
                                height: width
                                radius: width / 2
                                color: "transparent"
                                border.width: 2
                                border.color: root.m3.primary
                                opacity: grow > 0 ? (1 - grow) * 0.8 : 0
                                NumberAnimation {
                                    id: ripple
                                    target: rippleRing
                                    property: "grow"
                                    from: 0
                                    to: 1
                                    duration: 600
                                    easing.type: Easing.OutCubic
                                }
                            }
                            MaterialShape {
                                id: fingerShape
                                x: finger.shake
                                width: parent.width
                                height: parent.height
                                shape: finger.phase === "match" ? "cookie9" : "circle"
                                color: finger.phase === "match" ? root.m3.primaryContainer : finger.phase === "miss" ? root.m3.errorContainer : finger.phase === "waiting" ? root.m3.secondaryContainer : "transparent"
                                Behavior on color { ColorAnimation { duration: 150 } }
                            }
                            Icon {
                                anchors.centerIn: fingerShape
                                name: finger.phase === "match" ? "check" : "fingerprint"
                                size: 22
                                color: finger.phase === "match" ? root.m3.onPrimaryContainer : finger.phase === "miss" ? root.m3.onErrorContainer : root.m3.primary
                                Behavior on color { ColorAnimation { duration: 150 } }
                            }
                        }
                    }

                    // A click on the closed pill opens it
                    MouseArea {
                        anchors.fill: parent
                        visible: !root.open
                        cursorShape: Qt.IBeamCursor
                        onClicked: {
                            root.wake();
                            input.forceActiveFocus();
                        }
                    }
                }
            }

            // What the fingerprint reader or the face camera wants (Lock.qml),
            // under the pill (its line kept while either is set up, so the
            // pill doesn't jump; blank while the reader just listens: the
            // glyph says that)
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 6
                height: 20
                readonly property bool isFace: root.lockScope.faceText !== "" && root.lockScope.fingerText === ""
                readonly property bool hasText: (root.lockScope.fingerText !== "" && root.lockScope.fingerState !== "waiting") || root.lockScope.faceText !== ""
                visible: hasText || Fingerprint.lockScreen || Face.lockScreen
                opacity: hasText ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 150 } }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: parent.isFace ? "smiley" : "fingerprint"
                    size: 16
                    color: root.ink
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.lockScope.fingerText || root.lockScope.faceText
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
                }
            }
        }
    }

    // The pill opens on a key or a mouse move and closes 10 s after the
    // last one, unless something's typed (or being checked, or wrong)
    property bool typing: false
    readonly property bool open: typing || input.text !== "" || lockScope.busy || lockScope.error !== ""
    property real openT: open ? 1 : 0
    Behavior on openT {
        SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.002 }
    }
    function wake() {
        typing = true;
        idle.restart();
    }
    Timer {
        id: idle
        interval: 10000
        onTriggered: root.typing = false
    }
    HoverHandler {
        // (the first position after the lock appears isn't a move)
        property point last: Qt.point(-1, -1)
        onPointChanged: {
            const p = point.position;
            if (last.x >= 0 && (Math.abs(p.x - last.x) > 2 || Math.abs(p.y - last.y) > 2))
                root.wake();
            last = p;
        }
    }
    // Your name as the account has it (its full name), else the login
    property string fullName: ""
    Process {
        running: true
        command: ["sh", "-c", "getent passwd \"$USER\" | cut -d: -f5 | cut -d, -f1"]
        stdout: StdioCollector {
            onStreamFinished: root.fullName = text.trim()
        }
    }

    // Now Playing on the lock screen: art in a cookie, a play button that
    // squares off while playing, the wavy timeline (one per player: the
    // carousel widens the chosen one in place)
    component LockPlayer: Rectangle {
                id: lp
                property var player
                readonly property var p: player
                readonly property bool playingNow: p?.isPlaying ?? false
                readonly property bool timeline: (p?.lengthSupported ?? false) && (p?.length ?? 0) > 0
                radius: Theme.radius.tile
                color: root.m3.surfaceContainerHigh

                Timer {
                    interval: 1000
                    repeat: true
                    running: lp.visible && lp.timeline && lp.playingNow && root.visible
                    onTriggered: lp.p?.positionChanged()
                }

                // The art in a cookie while it plays, a circle when paused
                Item {
                    id: art
                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: 56
                    height: 56
                    Image {
                        id: artImage
                        anchors.fill: parent
                        source: Media.artUrl(lp.p)
                        retainWhileLoading: true
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(112, 112)
                        asynchronous: true
                    }
                    MaterialShape {
                        anchors.fill: parent
                        shape: lp.playingNow ? "cookie9" : "circle"
                        color: root.m3.surfaceContainerHighest
                        source: artImage.status === Image.Ready && artImage.source != "" ? artImage : null
                    }
                    Icon {
                        anchors.centerIn: parent
                        visible: artImage.status !== Image.Ready || artImage.source == ""
                        name: "music-notes"
                        fill: true
                        size: 28
                        color: root.m3.onSurfaceVariant
                    }
                }
                Column {
                    anchors { left: art.right; leftMargin: 12; right: controls.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Text {
                        width: parent.width
                        text: lp.p?.trackTitle ?? ""
                        elide: Text.ElideRight
                        color: root.m3.onSurface
                        font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
                    }
                    Text {
                        width: parent.width
                        text: lp.p?.trackArtist ?? ""
                        elide: Text.ElideRight
                        color: root.m3.onSurfaceVariant
                        font { family: Theme.font.family; pixelSize: 13 }
                    }
                    // The timeline: wavy while it plays, flat when paused (so
                    // a paused track on the lock screen draws nothing)
                    Item {
                        id: timelineBox
                        visible: lp.timeline
                        width: parent.width
                        height: 18
                        readonly property real done: lp.timeline ? Math.max(0, Math.min(1, (lp.p?.position ?? 0) / lp.p.length)) : 0
                        WavyProgress {
                            visible: lp.playingNow
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width
                            value: timelineBox.done
                        }
                        Rectangle {
                            visible: !lp.playingNow
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width
                            height: 4
                            radius: 2
                            color: root.m3.secondaryContainer
                            Rectangle {
                                width: parent.width * timelineBox.done
                                height: parent.height
                                radius: 2
                                color: root.m3.primary
                            }
                        }
                    }
                }
                Row {
                    id: controls
                    anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    IconButton {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 36
                        height: 36
                        icon: "skip-back"
                        fill: true
                        iconSize: 20
                        iconColor: root.m3.onSurfaceVariant
                        tooltip: "Previous track"
                        onClicked: lp.p?.previous()
                    }
                    Item {
                        id: play
                        anchors.verticalCenter: parent.verticalCenter
                        width: 44
                        height: 44
                        transform: pressFx1.effects
                        ElasticControl { id: pressFx1; pressed: playMouse.pressed }
                        // (Material: grows on press: the ElasticControl above)
                        Behavior on scale { SpringAnimation { spring: 12.8; damping: 0.54; epsilon: 0.005 } }
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: lp.playingNow ? "Pause" : "Play"
                        Accessible.onPressAction: lp.p?.togglePlaying()
                        Keys.onPressed: e => {
                            if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                                lp.p?.togglePlaying();
                                e.accepted = true;
                            }
                        }
                        // Selected (playing) squares off, as Material's toggles do
                        MaterialShape {
                            anchors.fill: parent
                            shape: lp.playingNow ? "square" : "circle"
                            color: playMouse.containsMouse ? Qt.tint(root.m3.primary, Qt.alpha(root.m3.onPrimary, playMouse.pressed ? 0.1 : 0.08)) : root.m3.primary
                        }
                        Icon {
                            anchors.centerIn: parent
                            name: lp.playingNow ? "pause" : "play"
                            fill: true
                            size: 22
                            color: root.m3.onPrimary
                        }
                        FocusRing {
                            radius: play.width / 2 + 4
                        }
                        MouseArea {
                            id: playMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: lp.p?.togglePlaying()
                        }
                    }
                    IconButton {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 36
                        height: 36
                        icon: "skip-forward"
                        fill: true
                        iconSize: 20
                        iconColor: root.m3.onSurfaceVariant
                        tooltip: "Next track"
                        onClicked: lp.p?.next()
                    }
                }
            }

    // A picture for the avatar, if there is one
    property string avatarPath: ""
    Process {
        running: true
        command: ["sh", "-c", "for f in \"$HOME/.face\" \"$HOME/.face.icon\" \"/var/lib/AccountsService/icons/$USER\"; do [ -s \"$f\" ] && [ -r \"$f\" ] && { echo \"$f\"; exit 0; }; done; true"]
        stdout: StdioCollector {
            onStreamFinished: root.avatarPath = text.trim()
        }
    }

    Connections {
        target: root.lockScope
        function onErrorChanged() {
            if (root.lockScope.error !== "")
                shakeAnim.restart();
        }
        function onFingerStateChanged() {
            if (root.lockScope.fingerState === "match")
                finger.matched = true;
        }
        function onBusyChanged() {
            // (back from checking the password: type again straight away)
            if (!root.lockScope.busy && !root.host.standby)
                input.forceActiveFocus();
        }
    }

    // ── The light reveal (lock.lockTransition zoom) ─────────────────────
    // The screen's black, with a circle opening from the centre: a ring
    // whose hole grows (a plain rectangle's border, nothing per pixel)
    Rectangle {
        readonly property real reach: Math.hypot(root.width, root.height)
        readonly property real hole: root.reveal * reach / 2 * 1.04
        visible: root.zoomIn && root.reveal < 1
        width: (hole + reach) * 2
        height: width
        x: (root.width - width) / 2
        y: (root.height - height) / 2
        radius: width / 2
        color: "transparent"
        border.width: reach
        border.color: root.host.color
    }
}
