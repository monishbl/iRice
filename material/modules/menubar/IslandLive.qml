import QtQuick
import Quickshell
import Quickshell.Services.UPower
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
// Material: Android's Live Updates chip, the icon (or the art) in a primary
// circle (error for recording and other urgent ones).
Item {
    id: root

    property var activity: null
    property string part: "info" // info | controls
    property color ink: "white"
    property real maxWidth: 100000
    property bool still: false // nobody can see it (full screen, locked): animations hold
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
            readonly property bool art: root.kind === "music" && Media.artUrl(root.player) !== ""
            visible: root.on(root.kind === "music" ? "art" : "icon")
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.material ? 20 : 18
            height: width
            // Material: the circle
            Rectangle {
                visible: Theme.material && !lead.art && !callApp.visible
                anchors.fill: parent
                radius: width / 2
                color: Theme.material ? IslandInk.tint(root.activity?.color) : "transparent"
            }
            // Material: the art, round
            Loader {
                anchors.fill: parent
                active: Theme.material && lead.art
                sourceComponent: Item {
                    Image {
                        id: mArt
                        anchors.fill: parent
                        source: Media.artUrl(root.player)
                        retainWhileLoading: true
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(40, 40)
                        asynchronous: true
                    }
                    MaterialShape {
                        anchors.fill: parent
                        shape: "circle"
                        animated: false
                        color: Theme.m3.surfaceContainerHighest
                        source: mArt
                    }
                }
            }
            // Recording: a pulsing red dot (1.4 s, on a slow clock)
            Rectangle {
                visible: root.kind === "recording"
                anchors.centerIn: parent
                width: Theme.material ? 8 : 9
                height: width
                radius: width / 2
                color: Theme.material ? IslandInk.onTint(root.activity?.color) : Theme.accents.red
                opacity: 0.675 + 0.325 * Math.cos(dotClock.time * Math.PI / 0.7)
                Ticker { id: dotClock; running: root.kind === "recording" && root.part === "info" && root.visible; fps: 15 }
            }
            // A call answered in the island: its app's icon
            AppIcon {
                id: callApp
                visible: root.kind === "call" && (root.activity?.appIcon ?? "") !== ""
                anchors.centerIn: parent
                size: 16
                source: visible ? root.activity.appIcon : ""
            }
            Icon {
                anchors.centerIn: parent
                visible: root.kind !== "recording" && !callApp.visible && !lead.art
                name: root.activity?.icon ?? ""
                fill: true
                size: Theme.material ? 12 : 14
                color: Theme.material ? IslandInk.onTint(root.activity?.color) : root.activity?.color ?? root.ink
            }
        }

        // Song title, the timer's label (Focus / Break), the file or the call's app
        Marquee {
            id: title
            visible: text !== "" && (root.kind === "music" ? root.on("title") : root.kind === "timer" ? root.on("label") : root.on("name"))
            anchors.verticalCenter: parent.verticalCenter
            text: root.kind === "music" ? (root.player?.trackTitle || root.player?.identity || "") : root.kind === "screenShare" ? Privacy.screenApp : LiveInfo.title(root.kind)
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
            color: Theme.material ? (root.kind === "recording" ? Theme.m3.error : root.ink) : root.kind === "recording" ? Theme.accents.red : root.activity?.color ?? root.ink
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
            // 12 fps is plenty for bars this small (they swing slowly); held
            // while nobody can see the bar, and on battery below 30%
            Ticker { id: barClock; running: bars.on && !root.still && !(UPower.onBattery && Island.level < 0.3 && Island.hasBattery); fps: 12 }
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
                    color: Theme.material ? Theme.m3.primary : Theme.accents.green
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
                iconColor: modelData.danger ? (Theme.material ? Theme.m3.error : Theme.accents.red) : root.ink
                opacity: modelData.on ? 1 : 0.4
                enabled: modelData.on
                onClicked: modelData.act()
            }
        }
    }
}
