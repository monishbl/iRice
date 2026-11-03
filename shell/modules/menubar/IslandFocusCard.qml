import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.modules.captions
import qs.services

// The open island's view of one live activity.
//   music      a mini player: big art, title and artist, a seek bar, and
//              volume / previous / play-pause / next / Cast / open-the-player
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
    implicitHeight: kind === "music" ? (overlay ? 200 : 132) : kind === "captions" ? captionsCard.implicitHeight : 80

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
    // Pick a Chromecast or Google TV for what's playing (Control Center's
    // Cast page)
    function openCast() {
        Panels.show("controlCenter", Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name));
        Panels.ccPage = "cast";
    }
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
            Audio.toggleMute(true);
        }
    }
    function nudgeVolume(d) {
        setVolume(volume + d);
    }
    function setVolume(v) {
        v = Math.max(0, Math.min(1, v));
        if (ownVolume) {
            player.volume = v;
        } else {
            Audio.setVolume(v, true);
            if (Audio.muted && v > 0)
                Audio.toggleMute(true);
        }
    }

    component Ctl: IconButton {
        width: 36
        height: 36
        radius: height / 2 // (round on the island, like its controls)
        fill: true
        iconSize: 18
        iconColor: root.ink
    }

    // The volume, on the left of the controls: a click on the speaker mutes;
    // the pointer on it opens a slider beside it (the player's own volume
    // when it has one, else the system's). The wheel works over both.
    component Volume: Item {
        id: vol
        readonly property bool open: volHover.hovered || volDrag.pressed
        width: open ? 104 : 36
        height: 36
        Behavior on width { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
        HoverHandler { id: volHover }
        WheelHandler { onWheel: e => root.nudgeVolume(e.angleDelta.y > 0 ? 0.05 : -0.05) }
        Rectangle {
            anchors.fill: parent
            radius: height / 2
            color: Qt.rgba(1, 1, 1, vol.open ? 0.12 : 0)
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        }
        Ctl {
            id: speaker
            icon: root.muted ? "speaker-x" : root.volume < 0.4 ? "speaker-low" : "speaker-high"
            fill: !vol.open
            color: "transparent" // (the pill around it shows the hover)
            tooltip: root.muted ? "Unmute" : "Mute"
            onClicked: root.toggleMute()
        }
        Item {
            anchors { left: speaker.right; leftMargin: 2; right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
            height: 20
            visible: opacity > 0.01
            opacity: Math.max(0, (vol.width - 60) / 44)
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                height: 4
                radius: 2
                color: Qt.rgba(1, 1, 1, 0.25)
                Rectangle {
                    width: parent.width * (root.muted ? 0 : root.volume)
                    height: parent.height
                    radius: 2
                    color: root.ink
                }
            }
            MouseArea {
                id: volDrag
                anchors { fill: parent; topMargin: -8; bottomMargin: -8 }
                cursorShape: Qt.PointingHandCursor
                preventStealing: true
                onPressed: m => root.setVolume(m.x / width)
                onPositionChanged: m => {
                    if (pressed)
                        root.setVolume(m.x / width);
                }
            }
            Accessible.role: Accessible.Slider
            Accessible.name: "Volume"
        }
    }
    // Open the player's window: pointed at, it widens to the left to say
    // which app (cut short to the room there is)
    component OpenApp: Item {
        id: oa
        property real maxWidth: 140
        readonly property bool open: oaHover.hovered && name.text !== ""
        width: open ? Math.min(maxWidth, name.implicitWidth + 32 + 10) : 36
        height: 36
        Behavior on width { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
        HoverHandler { id: oaHover }
        Rectangle {
            anchors.fill: parent
            radius: height / 2
            color: Qt.rgba(1, 1, 1, oaTap.pressed ? 0.2 : oaHover.hovered ? 0.12 : 0)
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        }
        Text {
            id: name
            anchors { left: parent.left; leftMargin: 10; right: openIcon.left; verticalCenter: parent.verticalCenter }
            // (the app's own name, "Zen Browser", over what the player calls
            // itself, "Mozilla zen")
            text: Apps.entryFor(root.player?.desktopEntry ?? "")?.name || root.player?.identity || ""
            elide: Text.ElideRight
            opacity: Math.max(0, (oa.width - 52) / 30)
            visible: opacity > 0.01
            color: root.ink
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        }
        Ctl {
            id: openIcon
            anchors.right: parent.right
            width: oa.open ? 32 : 36
            icon: "arrow-square-out"
            fill: false
            iconSize: 17
            color: "transparent" // (the pill shows the hover: one round shape)
            tooltip: "Open " + (name.text || "the player")
            onClicked: root.openPlayer()
        }
        // (the name is part of the button too)
        TapHandler {
            id: oaTap
            onTapped: root.openPlayer()
        }
    }
    // Under the song: the volume on the left, previous / play / next in the
    // middle, then Cast and open the player's window on the right
    component Transport: Item {
        property int playIcon: 24
        height: 44
        Volume {
            anchors { left: parent.left; verticalCenter: parent.verticalCenter }
        }
        Row {
            anchors.centerIn: parent
            spacing: 10
            Ctl { icon: "skip-back"; enabled: root.player?.canGoPrevious ?? false; opacity: enabled ? 1 : 0.4; onClicked: root.player?.previous() }
            Ctl { icon: Island.musicPlaying ? "pause" : "play"; iconSize: parent.parent.playIcon; width: 44; onClicked: root.player?.togglePlaying() }
            Ctl { icon: "skip-forward"; enabled: root.player?.canGoNext ?? false; opacity: enabled ? 1 : 0.4; onClicked: root.player?.next() }
        }
        Row {
            id: trailing
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: 4
            Ctl { id: castBtn; visible: Cast.devices.length > 0 || Cast.active; icon: "screencast"; fill: Cast.active; iconSize: 17; tooltip: Cast.active ? "Casting to " + (Cast.device?.name ?? "a TV") : "Cast to a TV"; onClicked: root.openCast() }
            // (as wide as the room right of play / next allows)
            OpenApp { maxWidth: parent.parent.width / 2 - 68 - 6 - (castBtn.visible ? castBtn.width + 4 : 0) }
        }
    }

    // ── Music, controls over the art ────────────────────────────────────
    // How the art shows (island.playerArt): island, the art is the island's
    // background, fading like its black (IslandView's islandArt, not here);
    // faded, the art here with its edges fading out (common/edgefade.frag).
    // (The small player, art on the left, is playerLayout "side", below.)
    readonly property string artStyle: Config.options.island.playerArt
    Item {
        visible: root.kind === "music" && root.overlay
        anchors.fill: parent
        Item {
            anchors.fill: parent
            visible: root.artStyle === "faded"
            Image {
                anchors.fill: parent
                source: Media.artUrl(root.player) // (kept loaded, at a fixed size: no reload and flicker on every open)
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(640, 640)
                asynchronous: true
                retainWhileLoading: true
                layer.enabled: true
                layer.effect: ShaderEffect {
                    property vector4d fade: Qt.vector4d(0.28, 0.22, 0.28, 0.6)
                    fragmentShader: Qt.resolvedUrl("../../common/edgefade.frag.qsb")
                }
            }
        }
        Icon {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -30
            visible: Media.artUrl(root.player) === ""
            name: "music-notes"
            size: 40
            color: root.ink2
        }
        Column {
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 12 }
            spacing: 6
            Column {
                id: titleCol
                width: parent.width
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
                    textFormat: Text.PlainText
                    text: [root.player?.trackArtist, root.player?.trackAlbum].filter(x => !!x).join(" · ")
                    elide: Text.ElideRight
                    color: Qt.rgba(1, 1, 1, 0.75)
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
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
            Transport {
                width: parent.width
            }
        }
    }

    // ── Music, art beside the controls ──────────────────────────────────
    Item {
        visible: root.kind === "music" && !root.overlay
        anchors.fill: parent

        // Nested in the island's corner: its radius less the island's padding
        Item {
            id: art
            width: 72
            height: 72
            readonly property real radius: Theme.radius.tile - 12
            Squircle {
                anchors.fill: parent
                radius: art.radius
                color: Qt.rgba(1, 1, 1, 0.1)
            }
            Image {
                anchors.fill: parent
                source: Media.artUrl(root.player)
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(144, 144)
                asynchronous: true
                retainWhileLoading: true
                layer.enabled: true // (always: switching the mask on when it loaded showed it square for a frame)
                layer.effect: MultiEffect {
                    maskEnabled: true
                    maskSource: artMask
                    maskThresholdMin: 0.5
                    maskSpreadAtMin: 1
                }
            }
            Squircle {
                id: artMask
                anchors.fill: parent
                radius: art.radius
                color: "black"
                visible: false
                layer.enabled: true
            }
            Icon {
                anchors.centerIn: parent
                visible: Media.artUrl(root.player) === ""
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
                textFormat: Text.PlainText
                text: root.player?.trackTitle || "Not Playing"
                elide: Text.ElideRight
                color: root.ink
                font { family: Theme.font.family; pixelSize: 16; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                textFormat: Text.PlainText
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

        Transport {
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
            playIcon: 22
        }
    }

    // ── Live captions: the last words, up to four lines, and Off ────────
    Item {
        visible: root.kind === "captions"
        anchors.fill: parent
        Column {
            id: captionsCard
            width: parent.width - offBtn.width - 8
            spacing: 4
            Text {
                text: "Live captions"
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
            }
            CaptionLines {
                width: parent.width
                ink: root.ink
                size: Theme.font.body
                lines: 4
            }
        }
        Ctl {
            id: offBtn
            anchors { right: parent.right; top: parent.top }
            icon: "x"
            fill: false
            tooltip: "Turn off live captions"
            onClicked: Captions.cfg.enabled = false
        }
    }

    // ── Timer, stopwatch, recording, transfer, call ─────────────────────
    Item {
        visible: root.kind !== "music" && root.kind !== "captions" && root.kind !== ""
        anchors.fill: parent

        Rectangle {
            id: badge
            width: 44
            height: 44
            radius: 22
            color: Qt.alpha(root.activity?.color ?? root.ink, 0.22)
            // A call answered in the island: its app's icon
            AppIcon {
                id: badgeApp
                visible: root.kind === "call" && (root.activity?.appIcon ?? "") !== ""
                anchors.centerIn: parent
                size: 28
                source: visible ? root.activity.appIcon : ""
            }
            Icon {
                anchors.centerIn: parent
                visible: !badgeApp.visible
                name: LiveInfo.icon(root.kind)
                fill: true
                size: 22
                color: root.activity?.color ?? root.ink
            }
        }
        // Timer, stopwatch, recording: a small caption over a big number.
        // Transfer and call: the file / app over what's happening.
        readonly property bool wordy: root.kind === "transfer" || root.kind === "call" || root.kind === "screenShare" || root.kind === "sysupdate"
        Column {
            anchors { left: badge.right; leftMargin: 14; right: buttons.left; rightMargin: 10; verticalCenter: badge.verticalCenter }
            spacing: 0
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: parent.parent.wordy ? LiveInfo.title(root.kind) : root.kind === "timer" ? LiveInfo.title("timer") + (Timers.running ? "" : " · paused") : root.kind === "stopwatch" ? "Stopwatch" + (Timers.laps.length ? " · " + Timers.laps.length + (Timers.laps.length === 1 ? " lap" : " laps") : "") : LiveInfo.title("recording") + (Recorder.cfg.mic !== "" && Recorder.micMuted ? " · mic off" : "")
                color: parent.parent.wordy ? root.ink : root.ink2
                font { family: Theme.font.family; pixelSize: parent.parent.wordy ? Theme.font.body : Theme.font.small; weight: parent.parent.wordy ? Font.DemiBold : Font.Medium }
            }
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: parent.parent.wordy ? LiveInfo.detail(root.kind) : root.kind === "stopwatch" ? Timers.formatMs(Timers.swElapsed) : LiveInfo.time(root.kind)
                color: root.kind === "recording" ? (Recorder.paused ? root.ink2 : Theme.accents.red) : parent.parent.wordy ? root.ink2 : root.ink
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
                    iconColor: modelData.danger ? Theme.accents.red : root.ink
                    enabled: modelData.on
                    opacity: enabled ? 1 : 0.4
                    onClicked: modelData.act()
                }
            }
        }
        // Recording with a microphone: its level (grey while muted)
        Item {
            id: micMeter
            readonly property bool active: root.visible && root.kind === "recording" && Recorder.cfg.mic !== "" && Recorder.micNode !== null
            visible: active
            onActiveChanged: Recorder.meterUsers += active ? 1 : -1
            Component.onDestruction: if (active) Recorder.meterUsers--
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; bottomMargin: 8 }
            height: 4
            Rectangle {
                anchors.fill: parent
                radius: 2
                color: Qt.rgba(1, 1, 1, 0.16)
            }
            Rectangle {
                width: parent.width * (Recorder.micMuted ? 0 : Math.min(1, Math.sqrt(Recorder.micLevel)))
                height: parent.height
                radius: 2
                color: Theme.accents.green
                Behavior on width { NumberAnimation { duration: 80 } }
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
