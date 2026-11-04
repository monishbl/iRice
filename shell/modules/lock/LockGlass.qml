import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Widgets
import qs.common
import qs.glass
import qs.services

// One screen of the lock in the Glass look (LockSurface loads it; the
// wallpaper, StandBy and every rule about locking stay LockSurface's and
// Lock.qml's):
//   iOS    the date and a large clock, a row of small widgets (battery,
//          weather, next event), Now Playing with controls, recent
//          notifications, and your picture, name and the password field at
//          the bottom
//   macOS  the date and a large clock up top; your picture, name and the
//          password field at the bottom (Now Playing above them while music
//          plays)
// Cards and the field are glass, frost or solid (or follow the system), and
// the text is light, dark, or picked from the wallpaper.
// Everything here lifts away on unlock.
Item {
    id: root

    required property var host // the LockSurface
    // Notifications and Now Playing (lock.notifications): shelf (the apps'
    // icons and the newest under the clock, the player over the sign-in),
    // column (beside the clock, on a wide screen) or icons (only the icons)
    readonly property string style: host.cfg.notifications ?? "shelf"
    readonly property bool split: style === "column" && width > height
    readonly property color ink: host.darkInk ? "#18181B" : "#FFFFFF"
    readonly property color ink2: host.darkInk ? Qt.rgba(0.09, 0.09, 0.11, 0.66) : Qt.rgba(1, 1, 1, 0.72)

    // The clock, as the owner's lock → StandBy morph needs it
    readonly property Item bigClock: lockClock
    readonly property point clockCentre: Qt.point(header.x + lockClock.x + lockClock.width / 2, header.y + lockClock.y + lockClock.height / 2)
    function focusField() {
        input.forceActiveFocus();
    }

    opacity: 1 - host.exit
    scale: host.zoomOut ? 1 + 0.08 * host.exit : 1
    visible: !(host.standby && host.sb >= 1)

    SystemClock { id: clock; precision: SystemClock.Minutes }

    // A card's background in the chosen material
    component Plate: Item {
        id: plate
        property real radius: 20
        Glass {
            visible: root.host.material === "glass"
            anchors.fill: parent
            anchors.margins: -shadowMargin
            shadowMargin: 14
            radius: plate.radius
            forceLiquid: true
        }
        ClippingRectangle {
            visible: root.host.material !== "glass"
            anchors.fill: parent
            radius: plate.radius
            color: root.host.material === "solid" ? (root.host.darkInk ? "#FFFFFF" : "#232327") : "transparent"
            border.width: 1
            border.color: root.host.darkInk ? Qt.rgba(0, 0, 0, 0.08) : Qt.rgba(1, 1, 1, 0.12)
            ShaderEffectSource {
                visible: root.host.material === "frost"
                anchors.fill: parent
                sourceItem: root.host.frost
                readonly property point at: plate.mapToItem(root.host.frost, 0, 0)
                sourceRect: Qt.rect(at.x, at.y, plate.width, plate.height)
                live: false
            }
            Rectangle {
                visible: root.host.material === "frost"
                anchors.fill: parent
                color: root.host.darkInk ? Qt.rgba(1, 1, 1, 0.42) : Qt.rgba(0, 0, 0, 0.28)
            }
        }
    }

    // ── Date and time ───────────────────────────────────────────────────
    Column {
        id: header
        anchors.horizontalCenter: parent.horizontalCenter
        y: parent.height * (host.ios ? 0.07 : 0.1) + (host.lockFx === "zoom" ? (1 - host.reveal) * -20 : 0)
        opacity: host.reveal
        spacing: 0
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDate(clock.date, "dddd d MMMM")
            color: root.ink2
            font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold }
        }
        Text {
            id: lockClock
            anchors.horizontalCenter: parent.horizontalCenter
            // The morph to StandBy flies a copy of it (below)
            opacity: host.morphOn && host.sb > 0 ? 0 : 1
            // 12-hour, like the menu bar (no AM/PM at this size)
            text: TimeFormat.hour(clock.date) + ":" + String(clock.date.getMinutes()).padStart(2, "0")
            color: host.darkInk ? Qt.rgba(0.09, 0.09, 0.11, 0.85) : Qt.rgba(1, 1, 1, 0.85)
            font { family: Theme.font.family; pixelSize: host.ios ? 168 : 150; weight: Font.Bold; letterSpacing: -5; features: { "tnum": 1 } }
            layer.enabled: true
            layer.effect: MultiEffect {
                shadowEnabled: !host.darkInk
                shadowColor: Qt.rgba(0, 0, 0, 0.3)
                shadowBlur: 0.6
                shadowVerticalOffset: 2
            }
        }
    }

    // ── iOS: small widgets under the clock ──────────────────────────────
    Row {
        id: complications
        visible: host.ios
        anchors { horizontalCenter: parent.horizontalCenter; top: header.bottom; topMargin: 4 }
        opacity: host.reveal
        spacing: 16
        readonly property var next: Calendar.upcoming(1)[0] ?? null
        Repeater {
            model: [
                { icon: "", battery: true, text: Island.hasBattery ? Math.round(Island.level * 100) + "%" : "", on: Island.hasBattery },
                { icon: Weather.icon, text: Weather.temp + "°", on: Weather.ready },
                { icon: "calendar-blank", text: complications.next ? (complications.next.allDay ? "Today" : Qt.formatTime(complications.next.start, TimeFormat.hmShort)) : "", on: !!complications.next },
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
                        color: root.ink
                    }
                    BatteryGlyph {
                        visible: !!parent.parent.modelData.battery
                        anchors.horizontalCenter: parent.horizontalCenter
                        size: 11
                        level: Island.level
                        charging: Island.charging && Island.level < 1
                        plugged: Island.charging && Island.level >= 1
                        color: root.ink
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: parent.parent.modelData.text
                        color: root.ink
                        font { family: Theme.font.family; pixelSize: 13; weight: Font.DemiBold; features: { "tnum": 1 } }
                    }
                }
            }
        }
    }

    // ── Notifications ───────────────────────────────────────────────────
    // Now Playing: in the column beside the clock, or over the sign-in
    Plate {
        id: player
        readonly property var p: Island.player
        visible: !!p && (p.trackTitle ?? "") !== ""
        width: cards.width
        x: cards.x
        y: root.split ? header.y : login.y - height - 24
        opacity: host.reveal
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
                source: Media.artUrl(player.p)
                retainWhileLoading: true
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(120, 120)
                asynchronous: true
            }
        }
        Column {
            anchors { left: art.right; leftMargin: 12; right: buttons.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
            Text {
                width: parent.width
                textFormat: Text.PlainText
                text: player.p?.trackTitle ?? ""
                elide: Text.ElideRight
                color: root.ink
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                textFormat: Text.PlainText
                text: player.p?.trackArtist ?? ""
                elide: Text.ElideRight
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
        Row {
            id: buttons
            anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
            IconButton { icon: "skip-back"; fill: true; iconColor: root.ink; width: 36; height: 36; iconSize: 18; onClicked: player.p?.previous() }
            IconButton { icon: Island.musicPlaying ? "pause" : "play"; fill: true; iconColor: root.ink; width: 40; height: 40; iconSize: 22; onClicked: player.p?.togglePlaying() }
            IconButton { icon: "skip-forward"; fill: true; iconColor: root.ink; width: 36; height: 36; iconSize: 18; onClicked: player.p?.next() }
        }
    }

    Column {
        id: cards
        x: root.split ? Math.round(parent.width * 0.74 - width / 2) : Math.round((parent.width - width) / 2)
        y: root.split ? header.y + (player.visible ? player.height + 10 : 0) : (host.ios ? complications.y + complications.height : header.y + header.height) + 40
        width: root.split ? Math.min(420, parent.width * 0.3) : 420
        spacing: 10
        opacity: host.reveal

        // Who sent them, as a row of app icons (shelf, icons)
        Plate {
            id: shelf
            visible: !root.split && cards.all.length > 0
            anchors.horizontalCenter: parent.horizontalCenter
            width: shelfRow.implicitWidth + 20
            height: 48
            radius: 24
            Accessible.role: Accessible.StaticText
            Accessible.name: cards.all.length + (cards.all.length === 1 ? " notification" : " notifications")
            Row {
                id: shelfRow
                anchors.centerIn: parent
                spacing: 6
                Repeater {
                    model: shelf.visible ? cards.apps : []
                    AppIcon {
                        required property var modelData
                        anchors.verticalCenter: parent.verticalCenter
                        size: 30
                        styled: false
                        source: Notifications.iconFor(modelData)
                    }
                }
                Text {
                    visible: cards.all.length > cards.apps.length
                    anchors.verticalCenter: parent.verticalCenter
                    leftPadding: 4
                    text: "+" + (cards.all.length - cards.apps.length)
                    color: root.ink2
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
            }
        }
        readonly property var all: Notifications.list.slice().reverse()
        readonly property var apps: {
            const seen = {}, out = [];
            for (let i = 0; i < all.length && out.length < 6; i++) {
                const key = all[i].appName || all[i].desktopEntry || String(all[i].id);
                if (!seen[key]) {
                    seen[key] = true;
                    out.push(all[i]);
                }
            }
            return out;
        }

        // The newest (shelf) or the latest three (column): who and what;
        // the text only with "Show previews" on
        readonly property var recent: root.style === "icons" ? [] : all.slice(0, root.split ? 3 : 1)
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
                            textFormat: Text.PlainText
                            text: host.cfg.previews ? (note.modelData.summary || note.modelData.appName) : (note.modelData.appName || "Notification")
                            elide: Text.ElideRight
                            color: root.ink
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                        }
                        Text {
                            id: when
                            anchors.right: parent.right
                            text: Notifications.timeLabel(note.modelData, clock.date)
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                    Text {
                        width: parent.width
                        text: host.cfg.previews ? (note.modelData.body ?? "").replace(/<[^>]*>/g, "") : "Notification"
                        elide: Text.ElideRight
                        maximumLineCount: 1
                        color: root.ink2
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                }
            }
        }
        Text {
            visible: root.split && Notifications.list.length > 3
            anchors.horizontalCenter: parent.horizontalCenter
            text: (Notifications.list.length - 3) + " more"
            color: root.ink2
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        }
    }

    // ── Sign in ─────────────────────────────────────────────────────────
    Column {
        id: login
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: parent.height * (host.ios ? 0.06 : 0.1)
        opacity: host.reveal
        spacing: 10

        // You: your picture (or the person glyph) and your full name, in
        // both layouts
        Avatar {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 64
            height: 64
            color: host.darkInk ? Qt.rgba(0, 0, 0, 0.12) : Qt.rgba(1, 1, 1, 0.18)
            iconColor: root.ink
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(implicitWidth, 360)
            elide: Text.ElideRight
            textFormat: Text.PlainText
            text: Account.name
            color: root.ink
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
                target: host.lockScope
                function onErrorChanged() {
                    if (host.lockScope.error !== "")
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
                    enabled: !host.lockScope.busy
                    focus: true
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: Theme.font.title; letterSpacing: 1 }
                    text: host.lockScope.password
                    onTextChanged: host.lockScope.password = text
                    onAccepted: host.lockScope.submit()
                    Component.onCompleted: forceActiveFocus()

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: input.text === ""
                        // (a finger works too: the fingerprint in the field says so)
                        text: host.lockScope.fingerState !== "" ? "Fingerprint or Password" : "Enter Password"
                        color: root.ink2
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                }
                // The reader, in the field while it's empty (as on a Mac);
                // typing brings the arrow back
                readonly property bool fingerInField: host.lockScope.fingerState !== "" && input.text === "" && !host.lockScope.busy
                FingerGlyph {
                    x: parent.width - 30
                    y: 6
                    width: 24
                    height: 24
                    visible: parent.fingerInField
                    phase: host.lockScope.fingerState
                    size: 22
                    color: root.ink
                }
                Icon {
                    x: parent.width - 30
                    y: 6
                    width: 24
                    height: 24
                    visible: !host.lockScope.busy && !parent.fingerInField
                    name: "arrow-circle-right"
                    fill: true
                    size: 22
                    color: root.ink
                    opacity: input.text === "" ? 0.4 : 1
                    MouseArea { anchors.fill: parent; onClicked: host.lockScope.submit() }
                }
                Icon {
                    x: parent.width - 30
                    y: 6
                    width: 24
                    height: 24
                    visible: host.lockScope.busy
                    name: "circle-notch"
                    size: 22
                    color: root.ink
                    RotationAnimation on rotation { running: host.lockScope.busy; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
                }
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: host.lockScope.error
            visible: text !== ""
            color: root.ink
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        }
        // What the fingerprint reader or the face camera wants
        // (Lock.qml), under a wrong-password message too: the finger
        // still works then
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 6
            readonly property bool isFace: host.lockScope.faceText !== "" && host.lockScope.fingerText === ""
            visible: host.lockScope.fingerText !== "" || host.lockScope.faceText !== ""
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: parent.isFace ? "smiley" : "fingerprint"
                size: 14
                color: root.ink
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: host.lockScope.fingerText || host.lockScope.faceText
                color: root.ink
                opacity: 0.85
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
        }
    }
}
