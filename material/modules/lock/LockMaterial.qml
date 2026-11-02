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
    // A wide screen splits like a tablet: clock left, cards right
    readonly property bool split: ios && width > height * 1.2

    // Ink over the wallpaper: Material's fixed tones (the same in light and
    // dark), dark on a light wallpaper (lock.text)
    readonly property bool darkInk: host.darkInk
    readonly property color clockInk: darkInk ? m3.onPrimaryFixed : m3.primaryFixed
    readonly property color clockInk2: darkInk ? m3.onPrimaryFixedVariant : m3.primaryFixedDim
    readonly property color ink: darkInk ? m3.onSecondaryFixed : m3.secondaryFixed
    readonly property color ink2: darkInk ? m3.onSecondaryFixedVariant : m3.secondaryFixedDim

    // The clock, as the owner's lock → StandBy morph needs it
    readonly property int clockSize: Math.round(split ? Math.min(height * 0.27, width * 0.16) : Math.min(height * (ios ? 0.24 : 0.22), width * 0.3))
    readonly property real clockWeight: 640
    readonly property real clockWidth: 112
    readonly property rect clockRect: Qt.rect(header.x + clock.x, header.y + clock.y, clock.width, clock.height)
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
            x: root.split ? Math.round(root.width * 0.08) : Math.round((root.width - width) / 2)
            y: Math.round(root.height * (root.split ? 0.14 : root.ios ? 0.09 : 0.11)) + root.rise - root.part
            spacing: 20
            opacity: root.reveal

            MaterialClock {
                id: clock
                anchors.horizontalCenter: root.split ? undefined : parent.horizontalCenter
                width: implicitWidth
                height: implicitHeight
                time: wallClock.date
                size: root.clockSize
                // (the light reveal: thin, as if waking from always-on)
                weight: root.zoomIn ? 160 + (root.clockWeight - 160) * root.reveal : root.clockWeight
                widthAxis: root.clockWidth
                color: root.clockInk
                color2: root.clockInk2
                align: root.split ? Text.AlignLeft : Text.AlignHCenter
                ticks: !root.host.standby
                // The morph to StandBy flies a copy of it (LockSurface)
                opacity: root.host.morphOn && root.host.sb > 0 ? 0 : 1
            }

            Column {
                anchors.horizontalCenter: root.split ? undefined : parent.horizontalCenter
                x: root.split ? 4 : 0
                spacing: 8
                // Date and weather
                Row {
                    anchors.horizontalCenter: root.split ? undefined : parent.horizontalCenter
                    spacing: 12
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: Qt.formatDate(wallClock.date, "dddd d MMMM")
                        color: root.ink
                        font { family: Theme.font.family; pixelSize: 20; weight: Font.Medium }
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
                            font { family: Theme.font.family; pixelSize: 20; weight: Font.Medium; features: { "tnum": 1 } }
                        }
                    }
                }
                // Smartspace: the next event and the open to-dos
                Row {
                    id: smartspace
                    visible: root.ios && (next !== null || todo > 0)
                    anchors.horizontalCenter: root.split ? undefined : parent.horizontalCenter
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

        // ── Now Playing and notifications ───────────────────────────────
        Column {
            id: cards
            width: root.split ? Math.min(420, root.width * 0.3) : 420
            x: root.split ? Math.round(root.width * 0.72 - width / 2) : Math.round((root.width - width) / 2)
            // (moving with the clock, or in macos with the field, on unlock)
            y: root.split ? header.y : root.ios ? header.y + header.height + 32 : unlock.y - height - 32
            spacing: 12
            opacity: root.reveal

            // Now Playing: art in a cookie, a play button that squares off
            // while playing, the wavy timeline
            Rectangle {
                id: media
                readonly property var p: Island.player
                readonly property bool timeline: (p?.lengthSupported ?? false) && (p?.length ?? 0) > 0
                visible: !!p && (p.trackTitle ?? "") !== ""
                width: parent.width
                height: 112
                radius: Theme.radius.tile
                color: root.m3.surfaceContainerHigh

                Timer {
                    interval: 1000
                    repeat: true
                    running: media.visible && media.timeline && Island.musicPlaying && root.visible
                    onTriggered: media.p?.positionChanged()
                }

                // The art in a cookie while it plays, a circle when paused
                Item {
                    id: art
                    x: 16
                    anchors.verticalCenter: parent.verticalCenter
                    width: 80
                    height: 80
                    Image {
                        id: artImage
                        anchors.fill: parent
                        source: Media.artUrl(media.p)
                        retainWhileLoading: true
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(160, 160)
                        asynchronous: true
                    }
                    MaterialShape {
                        anchors.fill: parent
                        shape: Island.musicPlaying ? "cookie9" : "circle"
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
                    anchors { left: art.right; leftMargin: 16; right: controls.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Text {
                        width: parent.width
                        text: media.p?.trackTitle ?? ""
                        elide: Text.ElideRight
                        color: root.m3.onSurface
                        font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
                    }
                    Text {
                        width: parent.width
                        text: media.p?.trackArtist ?? ""
                        elide: Text.ElideRight
                        color: root.m3.onSurfaceVariant
                        font { family: Theme.font.family; pixelSize: 14 }
                    }
                    // The timeline: wavy while it plays, flat when paused (so
                    // a paused track on the lock screen draws nothing)
                    Item {
                        id: timelineBox
                        visible: media.timeline
                        width: parent.width
                        height: 18
                        readonly property real done: media.timeline ? Math.max(0, Math.min(1, (media.p?.position ?? 0) / media.p.length)) : 0
                        WavyProgress {
                            visible: Island.musicPlaying
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width
                            value: timelineBox.done
                        }
                        Rectangle {
                            visible: !Island.musicPlaying
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
                        width: 40
                        height: 40
                        icon: "skip-back"
                        fill: true
                        iconSize: 22
                        iconColor: root.m3.onSurfaceVariant
                        tooltip: "Previous track"
                        onClicked: media.p?.previous()
                    }
                    Item {
                        id: play
                        anchors.verticalCenter: parent.verticalCenter
                        width: 56
                        height: 56
                        scale: playMouse.pressed ? 0.92 : 1
                        Behavior on scale { SpringAnimation { spring: 12.8; damping: 0.54; epsilon: 0.005 } }
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: Island.musicPlaying ? "Pause" : "Play"
                        Accessible.onPressAction: media.p?.togglePlaying()
                        Keys.onPressed: e => {
                            if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                                media.p?.togglePlaying();
                                e.accepted = true;
                            }
                        }
                        // Selected (playing) squares off, as Material's toggles do
                        MaterialShape {
                            anchors.fill: parent
                            shape: Island.musicPlaying ? "square" : "circle"
                            color: playMouse.containsMouse ? Qt.tint(root.m3.primary, Qt.alpha(root.m3.onPrimary, playMouse.pressed ? 0.1 : 0.08)) : root.m3.primary
                        }
                        Icon {
                            anchors.centerIn: parent
                            name: Island.musicPlaying ? "pause" : "play"
                            fill: true
                            size: 26
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
                            onClicked: media.p?.togglePlaying()
                        }
                    }
                    IconButton {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 40
                        height: 40
                        icon: "skip-forward"
                        fill: true
                        iconSize: 22
                        iconColor: root.m3.onSurfaceVariant
                        tooltip: "Next track"
                        onClicked: media.p?.next()
                    }
                }
            }

            // Notifications (ios layout), newest first: who sent them as a
            // shelf of app icons, or with "Show previews" on, compact cards
            // in a segmented group (corners 24 outside, 6 inside, 2 apart)
            readonly property var recent: root.ios ? Notifications.list.slice().reverse() : []
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
                visible: !root.cfg.previews && cards.recent.length > 0
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
                visible: root.cfg.previews && cards.recent.length > 0
                width: parent.width
                spacing: 2
                readonly property var shown: visible ? cards.recent.slice(0, 3) : []
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
                                    text: note.modelData.summary || note.modelData.appName || "Notification"
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
                                text: (note.modelData.body ?? "").replace(/<[^>]*>/g, "") || (note.modelData.appName ?? "")
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
                visible: root.cfg.previews && cards.recent.length > 3
                x: root.split ? 8 : (parent.width - width) / 2
                text: (cards.recent.length - 3) + " more"
                color: root.ink
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
        }

        // ── Unlock ──────────────────────────────────────────────────────
        Column {
            id: unlock
            x: Math.round((root.width - width) / 2)
            y: root.height - height - Math.round(root.height * (root.ios ? 0.05 : 0.08)) + root.part
            spacing: 16
            opacity: root.reveal

            // The avatar (~/.face, or AccountsService's picture) in a cookie
            Column {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 8
                Item {
                    id: avatar
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: root.ios ? 64 : 96
                    height: width
                    Image {
                        id: avatarImage
                        anchors.fill: parent
                        source: root.avatarPath !== "" ? "file://" + root.avatarPath : ""
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(192, 192)
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
                        size: Math.round(avatar.width * 0.45)
                        color: root.m3.onPrimaryContainer
                    }
                }
                Text {
                    visible: !root.ios
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Quickshell.env("USER") ?? ""
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: 22; weight: Font.Medium }
                }
            }

            // What went wrong and how to fix it, above the field as Android's
            // bouncer has it, in error on a container so it reads on any
            // wallpaper (the field stays put)
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
            // The field: a filled pill, outlined in primary while typing (in
            // error after a wrong password), the send button squaring off
            // once there's something to send
            Item {
                id: fieldBox
                anchors.horizontalCenter: parent.horizontalCenter
                width: 340
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
                    border.width: input.activeFocus || root.lockScope.error !== "" ? 2 : 0
                    border.color: root.lockScope.error !== "" ? root.m3.error : root.m3.primary

                    TextInput {
                        id: input
                        x: 24
                        width: parent.width - 24 - 60
                        height: parent.height
                        verticalAlignment: TextInput.AlignVCenter
                        echoMode: TextInput.Password
                        passwordCharacter: "•"
                        enabled: !root.lockScope.busy
                        focus: true
                        color: root.m3.onSurface
                        selectionColor: root.m3.primary
                        selectedTextColor: root.m3.onPrimary
                        font { family: Theme.font.family; pixelSize: 18; letterSpacing: 2 }
                        text: root.lockScope.password
                        onTextChanged: root.lockScope.password = text
                        onAccepted: root.lockScope.submit()
                        Accessible.name: "Password"

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: input.text === ""
                            // (a finger works too: the target under the field says so)
                            text: root.lockScope.fingerState !== "" ? "Fingerprint or password" : "Enter password"
                            color: root.m3.onSurfaceVariant
                            font { family: Theme.font.family; pixelSize: 16 }
                        }
                    }

                    Item {
                        id: send
                        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        width: 40
                        height: 40
                        readonly property bool ready: input.text !== ""
                        visible: !root.lockScope.busy
                        MaterialShape {
                            anchors.fill: parent
                            shape: send.ready ? "square" : "circle"
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
                        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        visible: root.lockScope.busy
                        size: 40
                    }
                }
            }

            // What the fingerprint reader or the face camera wants (Lock.qml),
            // under a wrong-password message too: the finger still works then
            // (its line kept while either is set up, so the field doesn't jump)
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 6
                height: 20
                readonly property bool isFace: root.lockScope.faceText !== "" && root.lockScope.fingerText === ""
                readonly property bool hasText: root.lockScope.fingerText !== "" || root.lockScope.faceText !== ""
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

            // The fingerprint target
            Item {
                id: finger
                anchors.horizontalCenter: parent.horizontalCenter
                width: 72
                height: 72
                // (its place kept while the reader is set up; it shows while
                // the reader listens)
                visible: Fingerprint.lockScreen || matched
                readonly property bool shown: root.lockScope.fingerState !== "" || matched || root.lockScope.leaving
                opacity: shown ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 200 } }
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
                // Listening: a subtle spring out and back (Material's default
                // spatial spring, ζ 0.8, k 380)
                SpringAnimation {
                    id: pulseAnim
                    target: finger
                    property: "pulse"
                    from: 1.1
                    to: 1
                    spring: 6.08
                    damping: 0.5
                    epsilon: 0.002
                }
                SpringAnimation {
                    id: fingerShake
                    target: finger
                    property: "shake"
                    from: 10
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
                    width: 72 + grow * 360
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
                    color: finger.phase === "match" ? root.m3.primaryContainer : finger.phase === "miss" ? root.m3.errorContainer : root.m3.surfaceContainerHigh
                    Behavior on color { ColorAnimation { duration: 150 } }
                }
                Icon {
                    anchors.centerIn: fingerShape
                    name: finger.phase === "match" ? "check" : "fingerprint"
                    size: 32
                    color: finger.phase === "match" ? root.m3.onPrimaryContainer : finger.phase === "miss" ? root.m3.onErrorContainer : root.m3.primary
                    Behavior on color { ColorAnimation { duration: 150 } }
                }
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
