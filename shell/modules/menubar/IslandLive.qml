import QtQuick
import Quickshell
import qs.common
import qs.services

// One side of a live activity in the collapsed island.
//   part "info":     album art + song title (scrolling when long) + three
//                    playing bars (only while playing), or the icon, label
//                    and time of a timer / stopwatch / recording / transfer / call
//   part "controls": previous / play-pause / next, or the activity's controls
// (What each kind says and does: LiveInfo.)
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
            // Recording: a pulsing red dot (1.4 s, on a slow clock)
            Rectangle {
                visible: root.kind === "recording"
                anchors.centerIn: parent
                width: 9
                height: 9
                radius: 4.5
                color: Theme.accents.red
                opacity: 0.675 + 0.325 * Math.cos(dotClock.time * Math.PI / 0.7)
                Ticker { id: dotClock; running: root.kind === "recording" && root.part === "info" && root.visible; fps: 15 }
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

        // Song title, the timer's label (Focus / Break), the file or the call's app
        Marquee {
            id: title
            visible: text !== "" && (root.kind === "music" ? root.on("title") : root.kind === "timer" ? root.on("label") : root.on("name"))
            anchors.verticalCenter: parent.verticalCenter
            text: root.kind === "music" ? (root.player?.trackTitle || root.player?.identity || "") : LiveInfo.title(root.kind)
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
            text: LiveInfo.time(root.kind)
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
            // 30 fps is plenty for bars this small (half the redraws of 60)
            Ticker { id: barClock; running: bars.on; fps: 30 }
            Repeater {
                model: 3
                Rectangle {
                    id: bar
                    required property int index
                    anchors.verticalCenter: parent.verticalCenter
                    width: 2.5
                    radius: 1.25
                    // each bar swings between its own low and high, out of step
                    readonly property real lo: [5, 4, 4][index]
                    readonly property real hi: [11, 10, 13][index]
                    readonly property real period: [0.70, 0.66, 0.74][index]
                    height: lo + (hi - lo) * (0.5 - 0.5 * Math.cos(2 * Math.PI * barClock.time / period + index * 1.9))
                    color: Theme.accents.green
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
            model: LiveInfo.controls(root.kind).filter(b => root.on(b.piece))
            IconButton {
                required property var modelData
                width: 24
                height: 22
                icon: modelData.icon
                fill: modelData.fill
                iconSize: 13
                iconColor: modelData.danger ? Theme.accents.red : root.ink
                opacity: modelData.on ? 1 : 0.4
                enabled: modelData.on
                onClicked: modelData.act()
            }
        }
    }
}
