import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.services

// The open island's view of one live activity.
//   music      a mini player: big art, title and artist, a seek bar, and
//              open-the-player / previous / play-pause / next / volume
//   timer      the time left, the phase, progress, play-pause / skip / reset
//   stopwatch  the time, laps, lap / play-pause / reset
//   recording  how long, and stop
//   transfer   the file, how far and how fast, open Downloads / cancel a copy
//   call       the app, how long, mute / open / leave
Item {
    id: root

    property var activity: null
    readonly property string kind: activity?.kind ?? ""
    readonly property var player: Island.player
    readonly property color ink: "#F5F5F7"
    readonly property color ink2: Qt.rgba(1, 1, 1, 0.6)

    readonly property bool overlay: Config.options.island.playerLayout !== "side"
    // Hundredths shown: ask for the fast stopwatch tick while visible
    readonly property bool fineClock: visible && Timers.swRunning && kind === "stopwatch"
    onFineClockChanged: Timers.swFine = Math.max(0, Timers.swFine + (fineClock ? 1 : -1))
    Component.onDestruction: if (fineClock) Timers.swFine = Math.max(0, Timers.swFine - 1)
    implicitHeight: kind === "music" ? (overlay ? 200 : 132) : 80

    // MPRIS doesn't push the position; ask for it while visible
    Timer {
        running: root.visible && root.kind === "music" && Island.musicPlaying
        repeat: true
        interval: 1000
        onTriggered: root.player?.positionChanged()
    }

    function fmt(s) {
        s = Math.max(0, Math.floor(s || 0));
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
    }
    // Bring the player's window forward (or its app)
    function openPlayer() {
        const p = player;
        if (!p)
            return;
        if (p.canRaise) {
            p.raise();
        } else {
            const want = (p.desktopEntry || p.identity || "").toLowerCase();
            const t = Hyprland.toplevels.values.find(w => (w.lastIpcObject?.class ?? "").toLowerCase().includes(want));
            if (t)
                Hyprland.dispatch("hl.dsp.focus({ window = \"address:0x" + t.address.replace(/^0x/, "") + "\" })");
        }
        Panels.close();
    }
    // The player's own volume when it has one, else the system output
    property real savedVolume: 1
    readonly property bool ownVolume: player?.volumeSupported ?? false
    readonly property real volume: ownVolume ? player.volume : Audio.volume
    readonly property bool muted: ownVolume ? player.volume < 0.01 : Audio.muted
    function toggleMute() {
        if (ownVolume) {
            if (player.volume > 0.01) {
                savedVolume = player.volume;
                player.volume = 0;
            } else {
                player.volume = savedVolume || 0.6;
            }
        } else {
            Audio.toggleMute();
        }
    }
    function nudgeVolume(d) {
        if (ownVolume)
            player.volume = Math.max(0, Math.min(1, player.volume + d));
        else
            Audio.setVolume(Math.max(0, Math.min(1, Audio.volume + d)));
    }

    component Ctl: IconButton {
        width: 36
        height: 36
        fill: true
        iconSize: 18
        iconColor: root.ink
    }

    // ── Music, controls over the art ────────────────────────────────────
    Rectangle {
        visible: root.kind === "music" && root.overlay
        anchors.fill: parent
        radius: 18
        color: Qt.rgba(1, 1, 1, 0.08)
        clip: true
        Image {
            anchors.fill: parent
            source: root.player?.trackArtUrl ?? ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(parent.width * 1.5, parent.height * 1.5)
            asynchronous: true
        }
        Icon {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -30
            visible: !(root.player?.trackArtUrl)
            name: "music-notes"
            size: 40
            color: root.ink2
        }
        // Darkens toward the bottom so the controls read on any art
        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.05) }
                GradientStop { position: 0.4; color: Qt.rgba(0, 0, 0, 0.2) }
                GradientStop { position: 1; color: Qt.rgba(0, 0, 0, 0.78) }
            }
        }
        Column {
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 12 }
            spacing: 6
            Item {
                width: parent.width
                height: titleCol.height
                Column {
                    id: titleCol
                    width: parent.width - openBtn.width - 4
                    spacing: 0
                    Marquee {
                        text: root.player?.trackTitle || "Not Playing"
                        maxWidth: titleCol.width
                        scroll: Island.shows("music", "scroll")
                        color: root.ink
                        font { family: Theme.font.family; pixelSize: 16; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        text: [root.player?.trackArtist, root.player?.trackAlbum].filter(x => !!x).join(" · ")
                        elide: Text.ElideRight
                        color: Qt.rgba(1, 1, 1, 0.75)
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                }
                Ctl {
                    id: openBtn
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    width: 32
                    height: 32
                    icon: "arrow-square-out"
                    fill: false
                    iconSize: 16
                    onClicked: root.openPlayer()
                }
            }
            Item {
                visible: (root.player?.lengthSupported ?? false) && (root.player?.length ?? 0) > 0
                width: parent.width
                height: 14
                readonly property real frac: Math.max(0, Math.min(1, (root.player?.position ?? 0) / Math.max(1, root.player?.length ?? 1)))
                Text {
                    id: oPos
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.fmt(root.player?.position)
                    color: Qt.rgba(1, 1, 1, 0.75)
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                }
                Text {
                    id: oLen
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    text: "-" + root.fmt((root.player?.length ?? 0) - (root.player?.position ?? 0))
                    color: Qt.rgba(1, 1, 1, 0.75)
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                }
                Rectangle {
                    anchors { left: oPos.right; leftMargin: 8; right: oLen.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    height: oSeek.containsMouse ? 6 : 4
                    radius: height / 2
                    color: Qt.rgba(1, 1, 1, 0.25)
                    Behavior on height { NumberAnimation { duration: Theme.motion.hover } }
                    Rectangle {
                        width: parent.width * parent.parent.frac
                        height: parent.height
                        radius: parent.radius
                        color: root.ink
                    }
                    MouseArea {
                        id: oSeek
                        anchors { fill: parent; topMargin: -6; bottomMargin: -6 }
                        hoverEnabled: true
                        enabled: root.player?.canSeek ?? false
                        cursorShape: Qt.PointingHandCursor
                        onClicked: m => {
                            root.player.position = m.x / width * root.player.length;
                            root.player.positionChanged();
                        }
                    }
                }
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Ctl { icon: "skip-back"; enabled: root.player?.canGoPrevious ?? false; opacity: enabled ? 1 : 0.4; onClicked: root.player?.previous() }
                Ctl { icon: Island.musicPlaying ? "pause" : "play"; iconSize: 24; width: 44; onClicked: root.player?.togglePlaying() }
                Ctl { icon: "skip-forward"; enabled: root.player?.canGoNext ?? false; opacity: enabled ? 1 : 0.4; onClicked: root.player?.next() }
                Ctl {
                    icon: root.muted ? "speaker-x" : root.volume < 0.4 ? "speaker-low" : "speaker-high"
                    onClicked: root.toggleMute()
                    WheelHandler { onWheel: e => root.nudgeVolume(e.angleDelta.y > 0 ? 0.05 : -0.05) }
                    Rectangle {
                        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 3 }
                        width: 18
                        height: 2
                        radius: 1
                        color: Qt.rgba(1, 1, 1, 0.25)
                        Rectangle {
                            width: parent.width * (root.muted ? 0 : root.volume)
                            height: parent.height
                            radius: 1
                            color: root.ink
                        }
                    }
                }
            }
        }
    }

    // ── Music, art beside the controls ──────────────────────────────────
    Item {
        visible: root.kind === "music" && !root.overlay
        anchors.fill: parent

        Rectangle {
            id: art
            width: 72
            height: 72
            radius: 14
            color: Qt.rgba(1, 1, 1, 0.1)
            clip: true
            Image {
                anchors.fill: parent
                source: root.player?.trackArtUrl ?? ""
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(144, 144)
                asynchronous: true
            }
            Icon {
                anchors.centerIn: parent
                visible: !(root.player?.trackArtUrl)
                name: "music-notes"
                size: 28
                color: root.ink2
            }
        }
        Column {
            anchors { left: art.right; leftMargin: 14; right: parent.right; verticalCenter: art.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                text: root.player?.trackTitle || "Not Playing"
                elide: Text.ElideRight
                color: root.ink
                font { family: Theme.font.family; pixelSize: 16; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                text: [root.player?.trackArtist, root.player?.trackAlbum].filter(x => !!x).join(" · ")
                elide: Text.ElideRight
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
            Text {
                width: parent.width
                text: root.player?.identity ?? ""
                elide: Text.ElideRight
                color: Qt.rgba(1, 1, 1, 0.4)
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }

        // Seek bar
        Item {
            id: seek
            visible: (root.player?.lengthSupported ?? false) && (root.player?.length ?? 0) > 0
            anchors { left: parent.left; right: parent.right; top: art.bottom; topMargin: 10 }
            height: 14
            readonly property real frac: Math.max(0, Math.min(1, (root.player?.position ?? 0) / Math.max(1, root.player?.length ?? 1)))
            Text {
                id: posText
                anchors.verticalCenter: parent.verticalCenter
                text: root.fmt(root.player?.position)
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
            }
            Text {
                id: lenText
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                text: "-" + root.fmt((root.player?.length ?? 0) - (root.player?.position ?? 0))
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
            }
            Rectangle {
                id: track
                anchors { left: posText.right; leftMargin: 8; right: lenText.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                height: seekMouse.containsMouse ? 6 : 4
                radius: height / 2
                color: Qt.rgba(1, 1, 1, 0.18)
                Behavior on height { NumberAnimation { duration: Theme.motion.hover } }
                Rectangle {
                    width: parent.width * seek.frac
                    height: parent.height
                    radius: parent.radius
                    color: root.ink
                }
                MouseArea {
                    id: seekMouse
                    anchors { fill: parent; topMargin: -6; bottomMargin: -6 }
                    hoverEnabled: true
                    enabled: root.player?.canSeek ?? false
                    cursorShape: Qt.PointingHandCursor
                    onClicked: m => {
                        root.player.position = m.x / width * root.player.length;
                        root.player.positionChanged();
                    }
                }
            }
        }

        Row {
            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom }
            spacing: 6
            Ctl { icon: "arrow-square-out"; fill: false; iconSize: 17; onClicked: root.openPlayer() }
            Ctl { icon: "skip-back"; enabled: root.player?.canGoPrevious ?? false; opacity: enabled ? 1 : 0.4; onClicked: root.player?.previous() }
            Ctl { icon: Island.musicPlaying ? "pause" : "play"; iconSize: 22; width: 44; onClicked: root.player?.togglePlaying() }
            Ctl { icon: "skip-forward"; enabled: root.player?.canGoNext ?? false; opacity: enabled ? 1 : 0.4; onClicked: root.player?.next() }
            Ctl {
                id: vol
                icon: root.muted ? "speaker-x" : root.volume < 0.4 ? "speaker-low" : "speaker-high"
                onClicked: root.toggleMute()
                WheelHandler { onWheel: e => root.nudgeVolume(e.angleDelta.y > 0 ? 0.05 : -0.05) }
                // The level, as a thin arc under the icon
                Rectangle {
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 3 }
                    width: 18
                    height: 2
                    radius: 1
                    color: Qt.rgba(1, 1, 1, 0.2)
                    Rectangle {
                        width: parent.width * (root.muted ? 0 : root.volume)
                        height: parent.height
                        radius: 1
                        color: root.ink
                    }
                }
            }
        }
    }

    // ── Timer, stopwatch, recording, transfer, call ─────────────────────
    Item {
        visible: root.kind !== "music" && root.kind !== ""
        anchors.fill: parent

        Rectangle {
            id: badge
            width: 44
            height: 44
            radius: 22
            color: Qt.alpha(root.activity?.color ?? root.ink, 0.22)
            Icon {
                anchors.centerIn: parent
                name: LiveInfo.icon(root.kind)
                fill: true
                size: 22
                color: root.activity?.color ?? root.ink
            }
        }
        // Timer, stopwatch, recording: a small caption over a big number.
        // Transfer and call: the file / app over what's happening.
        readonly property bool wordy: root.kind === "transfer" || root.kind === "call"
        Column {
            anchors { left: badge.right; leftMargin: 14; right: buttons.left; rightMargin: 10; verticalCenter: badge.verticalCenter }
            spacing: 0
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: parent.parent.wordy ? LiveInfo.title(root.kind) : root.kind === "timer" ? LiveInfo.title("timer") + (Timers.running ? "" : " · paused") : root.kind === "stopwatch" ? "Stopwatch" + (Timers.laps.length ? " · " + Timers.laps.length + (Timers.laps.length === 1 ? " lap" : " laps") : "") : "Recording"
                color: parent.parent.wordy ? root.ink : root.ink2
                font { family: Theme.font.family; pixelSize: parent.parent.wordy ? Theme.font.body : Theme.font.small; weight: parent.parent.wordy ? Font.DemiBold : Font.Medium }
            }
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: parent.parent.wordy ? LiveInfo.detail(root.kind) : root.kind === "stopwatch" ? Timers.formatMs(Timers.swElapsed) : LiveInfo.time(root.kind)
                color: root.kind === "recording" ? Theme.accents.red : parent.parent.wordy ? root.ink2 : root.ink
                font { family: Theme.font.family; pixelSize: parent.parent.wordy ? Theme.font.small : 24; weight: parent.parent.wordy ? Font.Normal : Font.DemiBold; features: { "tnum": 1 } }
            }
        }
        Row {
            id: buttons
            anchors { right: parent.right; verticalCenter: badge.verticalCenter }
            spacing: 4
            Repeater {
                model: LiveInfo.controls(root.kind)
                Ctl {
                    required property var modelData
                    icon: modelData.icon
                    enabled: modelData.on
                    opacity: enabled ? 1 : 0.4
                    onClicked: modelData.act()
                }
            }
        }
        // Timer and transfer progress
        Rectangle {
            visible: LiveInfo.progress(root.kind) >= 0
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; bottomMargin: 8 }
            height: 4
            radius: 2
            color: Qt.rgba(1, 1, 1, 0.16)
            Rectangle {
                width: parent.width * Math.max(0, LiveInfo.progress(root.kind))
                height: parent.height
                radius: 2
                color: root.activity?.color ?? root.ink
                Behavior on width { NumberAnimation { duration: 900 } }
            }
        }
    }
}
