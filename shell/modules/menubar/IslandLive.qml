import QtQuick
import Quickshell
import qs.common
import qs.services

// One side of a live activity in the collapsed island.
//   part "info":     album art + song title (scrolling when long) + three
//                    playing bars (only while playing), or the icon, label
//                    and time of a timer / stopwatch / recording
//   part "controls": previous / play-pause / next, or the timer's controls
// Each piece can be switched off (Settings → Menu bar, see Island.shows).
// `maxWidth` limits the info side (the title gives way first).
Item {
    id: root

    property var activity: null
    property string part: "info" // info | controls
    property color ink: "white"
    property real maxWidth: 100000
    readonly property string kind: activity?.kind ?? ""
    readonly property var player: Island.player
    function on(piece) {
        return Island.shows(kind, piece);
    }

    implicitWidth: part === "info" ? info.implicitWidth : controls.implicitWidth
    implicitHeight: 22

    SystemClock { id: clock; precision: SystemClock.Seconds; enabled: root.kind === "recording" }
    function timeText() {
        return kind === "timer" ? Timers.format(Timers.remaining)
            : kind === "stopwatch" ? Timers.format(Math.floor(Timers.swElapsed / 1000))
            : kind === "recording" ? Timers.format(Math.floor((clock.date - ScreenTools.recordingStarted) / 1000)) : "";
    }

    // ── Info ────────────────────────────────────────────────────────────
    Row {
        id: info
        visible: root.part === "info"
        anchors.verticalCenter: parent.verticalCenter
        spacing: 7

        // Album art, or the activity's icon
        Item {
            id: lead
            visible: root.on(root.kind === "music" ? "art" : "icon")
            anchors.verticalCenter: parent.verticalCenter
            width: 18
            height: 18
            Rectangle {
                anchors.fill: parent
                radius: 5
                clip: true
                color: Qt.alpha(root.ink, 0.15)
                visible: root.kind === "music" && (root.player?.trackArtUrl ?? "") !== ""
                Image {
                    anchors.fill: parent
                    source: root.kind === "music" ? (root.player?.trackArtUrl ?? "") : ""
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: Qt.size(36, 36)
                    asynchronous: true
                }
            }
            // Recording: a pulsing red dot
            Rectangle {
                visible: root.kind === "recording"
                anchors.centerIn: parent
                width: 9
                height: 9
                radius: 4.5
                color: Theme.accents.red
                SequentialAnimation on opacity {
                    running: root.kind === "recording" && root.part === "info" && root.visible
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.35; duration: 700; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
                }
            }
            Icon {
                anchors.centerIn: parent
                visible: root.kind !== "recording" && !(root.kind === "music" && (root.player?.trackArtUrl ?? "") !== "")
                name: root.activity?.icon ?? ""
                fill: true
                size: 14
                color: root.activity?.color ?? root.ink
            }
        }

        // Song title, or the timer's label (Focus / Break)
        Marquee {
            id: title
            visible: text !== "" && (root.kind === "music" ? root.on("title") : root.kind === "timer" && root.on("label"))
            anchors.verticalCenter: parent.verticalCenter
            text: root.kind === "music" ? (root.player?.trackTitle || root.player?.identity || "") : root.kind === "timer" ? (Timers.phase === "focus" ? "Focus" : "Break") : ""
            scroll: root.kind === "music" && root.on("scroll")
            color: root.ink
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            // What's left after the other pieces
            maxWidth: Math.max(0, Math.min(root.kind === "music" ? 132 : 90, root.maxWidth - (lead.visible ? lead.width + info.spacing : 0) - (time.visible ? time.implicitWidth + info.spacing : 0) - (bars.on ? bars.implicitWidth + info.spacing : 0)))
        }
        Text {
            id: time
            anchors.verticalCenter: parent.verticalCenter
            visible: root.kind !== "music" && root.on("time")
            text: root.timeText()
            color: root.kind === "recording" ? Theme.accents.red : root.activity?.color ?? root.ink
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold; features: { "tnum": 1 } }
        }

        // Three bars that dance while playing; gone when paused
        Row {
            id: bars
            readonly property bool on: root.kind === "music" && Island.musicPlaying && root.on("bars") && root.part === "info" && root.visible
            anchors.verticalCenter: parent.verticalCenter
            visible: opacity > 0.01
            opacity: on ? 1 : 0
            width: on ? implicitWidth : 0
            Behavior on opacity { NumberAnimation { duration: 200 } }
            spacing: 2
            Repeater {
                model: 3
                Rectangle {
                    id: bar
                    required property int index
                    anchors.verticalCenter: parent.verticalCenter
                    width: 2.5
                    radius: 1.25
                    height: 4
                    color: Theme.accents.green
                    SequentialAnimation on height {
                        running: bars.on
                        loops: Animation.Infinite
                        NumberAnimation { to: [11, 7, 13][bar.index]; duration: [380, 300, 440][bar.index]; easing.type: Easing.InOutSine }
                        NumberAnimation { to: [5, 10, 4][bar.index]; duration: [320, 360, 300][bar.index]; easing.type: Easing.InOutSine }
                    }
                }
            }
        }
    }

    // ── Controls ────────────────────────────────────────────────────────
    Row {
        id: controls
        visible: root.part === "controls"
        anchors.verticalCenter: parent.verticalCenter
        spacing: 0
        Repeater {
            model: (root.kind === "music" ? [
                { piece: "prev", icon: "skip-back", act: () => root.player?.previous(), on: root.player?.canGoPrevious ?? false },
                { piece: "play", icon: Island.musicPlaying ? "pause" : "play", act: () => root.player?.togglePlaying(), on: true },
                { piece: "next", icon: "skip-forward", act: () => root.player?.next(), on: root.player?.canGoNext ?? false }
            ] : root.kind === "timer" ? [
                { piece: "play", icon: Timers.running ? "pause" : "play", act: () => Timers.toggle(), on: true },
                { piece: "skip", icon: "skip-forward", act: () => Timers.skip(), on: true }
            ] : root.kind === "stopwatch" ? [
                { piece: "play", icon: Timers.swRunning ? "pause" : "play", act: () => Timers.swToggle(), on: true },
                { piece: "reset", icon: "arrow-counter-clockwise", act: () => Timers.swReset(), on: !Timers.swRunning }
            ] : root.kind === "recording" ? [
                { piece: "stop", icon: "stop", act: () => ScreenTools.stopRecording(), on: true }
            ] : []).filter(b => root.on(b.piece))
            IconButton {
                required property var modelData
                width: 24
                height: 22
                icon: modelData.icon
                fill: true
                iconSize: 13
                iconColor: root.ink
                opacity: modelData.on ? 1 : 0.4
                enabled: modelData.on
                onClicked: modelData.act()
            }
        }
    }
}
