import QtQuick
import qs.common
import qs.services

// Quick Settings' media player (the Material look), after Android 16's: a
// secondaryContainer card with the album art cut into a cookie that relaxes
// into a circle when paused, the app, title and artist, a play button that
// morphs (round while paused, squared off while playing), and the position
// as a wavy line that lies flat when paused; skip buttons either side. The
// cast button shows when there's a TV to cast to (the Cast page). Same
// player and actions as Control Center's Now Playing (services/Media).
Item {
    id: root

    property bool active: true // false in Edit controls: shown, not used
    property bool shown: true // the panel is open (the position ticks only then)
    property var player: Media.player // (the carousel gives each card its own)
    // Its forms, by the room it has (Edit controls: 4×1 or 2×2):
    //   wide     the art, the text and play on top, then previous, the
    //            position and next under the text
    //   slim     one row (4×1 in a short grid row): art, text, then previous,
    //            play and next
    //   compact  half width (2×2): art and text on top, previous, play and
    //            next along the bottom
    readonly property bool slim: height < 100
    readonly property bool compact: !slim && width < 300
    readonly property bool playing: player?.isPlaying ?? false
    readonly property color ink: Theme.m3.onSecondaryContainer

    implicitHeight: 136

    // The position: players don't announce it as it moves, so it's read
    // once a second while playing and the panel is open
    property real position: player?.position ?? 0
    Timer {
        running: root.shown && root.visible && root.playing
        interval: 1000
        repeat: true
        onTriggered: root.position = root.player?.position ?? 0
    }
    onShownChanged: position = player?.position ?? 0
    onPlayerChanged: position = player?.position ?? 0
    readonly property real length: (player?.lengthSupported ?? false) ? (player?.length ?? 0) : 0
    readonly property real progress: length > 0 ? Math.max(0, Math.min(1, position / length)) : 0

    Rectangle {
        anchors.fill: parent
        radius: 28
        color: Theme.m3.secondaryContainer
    }

    // Album art in a cookie (a circle while paused); a note without art
    Item {
        id: art
        x: root.slim ? 12 : 16
        y: root.slim ? (root.height - height) / 2 : 16
        width: root.slim ? root.height - 24 : root.compact ? 56 : root.height - 32
        height: width
        Image {
            id: artImage
            anchors.fill: parent
            source: Media.artUrl(root.player)
            retainWhileLoading: true
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(192, 192)
            asynchronous: true
        }
        MaterialShape {
            anchors.fill: parent
            shape: root.playing ? "cookie9" : "circle"
            color: Theme.m3.tertiaryContainer
            source: artImage.status === Image.Ready && artImage.source != "" ? artImage : null
        }
        Icon {
            anchors.centerIn: parent
            visible: artImage.status !== Image.Ready || artImage.source == ""
            name: "music-notes"
            fill: true
            size: 32
            color: Theme.m3.onTertiaryContainer
        }
    }

    // The app, title and artist
    Column {
        anchors { left: art.right; leftMargin: root.compact ? 12 : 16; right: root.slim ? controls.left : root.compact ? parent.right : play.left; rightMargin: root.compact ? 16 : 12 }
        y: root.slim ? (root.height - height) / 2 : 16
        spacing: 0
        Text {
            width: parent.width
            visible: text !== ""
            text: root.player?.identity ?? ""
            elide: Text.ElideRight
            color: Qt.alpha(root.ink, 0.72)
            font { family: Theme.font.family; pixelSize: 12; weight: Font.Medium }
        }
        Text {
            width: parent.width
            textFormat: Text.PlainText
            text: root.player?.trackTitle || "Not playing"
            elide: Text.ElideRight
            color: root.ink
            font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
        }
        Text {
            width: parent.width
            visible: text !== ""
            textFormat: Text.PlainText
            text: root.player?.trackArtist ?? ""
            elide: Text.ElideRight
            color: Qt.alpha(root.ink, 0.8)
            font { family: Theme.font.family; pixelSize: 14 }
        }
    }

    // Play / pause: round while paused, squared off while playing; a press
    // squares it further (fast spatial spring)
    Rectangle {
        id: play
        x: root.compact ? (root.width - width) / 2 : root.slim ? controls.x + (controls.width - width) / 2 : root.width - width - 16
        y: root.compact ? controls.y + (controls.height - height) / 2 : root.slim ? (root.height - height) / 2 : 16
        width: 48
        height: 48
        readonly property bool can: root.active && (root.player?.canTogglePlaying ?? false)
        transform: pressFx1.effects
        ElasticControl { id: pressFx1; pressed: playMouse.pressed }
        property real corner: playMouse.pressed ? 10 : root.playing ? 16 : 24
        Behavior on corner {
            SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.2 }
        }
        radius: corner
        color: can ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, playMouse.pressed ? 0.1 : playMouse.containsMouse ? 0.08 : 0)) : Qt.alpha(Theme.m3.onSurface, 0.12)
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        activeFocusOnTab: can
        Keys.onPressed: event => {
            if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                root.player?.togglePlaying();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: root.playing ? "Pause" : "Play"
        Accessible.onPressAction: root.player?.togglePlaying()
        FocusRing {
            radius: play.radius + 5
        }
        Icon {
            anchors.centerIn: parent
            name: root.playing ? "pause" : "play"
            fill: true
            size: 22
            color: play.can ? Theme.m3.onPrimary : Qt.alpha(Theme.m3.onSurface, 0.38)
        }
        MouseArea {
            id: playMouse
            anchors.fill: parent
            enabled: play.can
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.player?.togglePlaying()
        }
    }

    // Previous · position · next (· cast)
    Item {
        id: controls
        // (slim and compact: previous and next either side of play)
        x: root.slim ? root.width - 12 - width : root.compact ? 12 : art.x + art.width + 8
        width: root.slim ? 40 + 4 + 48 + 4 + 40 : root.compact ? root.width - 24 : root.width - x - 8
        y: root.compact ? root.height - height - 12 : root.slim ? (root.height - height) / 2 : art.y + art.height - height + 4
        height: 40
        enabled: root.active && !!root.player

        IconButton {
            id: prev
            anchors.verticalCenter: parent.verticalCenter
            width: 40
            height: 40
            icon: "skip-back"
            fill: true
            iconSize: 20
            iconColor: root.ink
            tooltip: "Previous"
            opacity: enabled ? 1 : 0.38
            enabled: root.player?.canGoPrevious ?? false
            onClicked: root.player?.previous()
        }
        // (seeks on a click, where the player allows it)
        Item {
            id: bar
            visible: !root.slim && !root.compact
            anchors { left: prev.right; leftMargin: 4; right: next.left; rightMargin: 4; verticalCenter: parent.verticalCenter }
            height: 20
            WavyProgress {
                visible: root.playing && root.length > 0
                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                value: root.progress
                color: Theme.m3.primary
                trackColor: Qt.alpha(root.ink, 0.18)
            }
            // Paused (or no length): the line lies flat
            Item {
                visible: !root.playing || root.length <= 0
                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                height: 4
                Rectangle {
                    width: Math.max(height, parent.width * root.progress)
                    visible: root.length > 0
                    height: 4
                    radius: 2
                    color: Theme.m3.primary
                }
                Rectangle {
                    x: root.length > 0 ? parent.width * root.progress + 4 : 0
                    width: Math.max(0, parent.width - x)
                    height: 4
                    radius: 2
                    color: Qt.alpha(root.ink, 0.18)
                }
            }
            MouseArea {
                anchors { fill: parent; topMargin: -6; bottomMargin: -6 }
                enabled: (root.player?.canSeek ?? false) && root.length > 0
                cursorShape: Qt.PointingHandCursor
                onClicked: m => {
                    const v = Math.max(0, Math.min(1, m.x / width)) * root.length;
                    root.player.position = v; // absolute, so a stale position can't skew it
                    root.position = v;
                }
            }
        }
        IconButton {
            id: next
            anchors { right: cast.visible ? cast.left : parent.right; verticalCenter: parent.verticalCenter }
            width: 40
            height: 40
            icon: "skip-forward"
            fill: true
            iconSize: 20
            iconColor: root.ink
            tooltip: "Next"
            opacity: enabled ? 1 : 0.38
            enabled: root.player?.canGoNext ?? false
            onClicked: root.player?.next()
        }
        // To a Chromecast or Google TV: the Cast page
        IconButton {
            id: cast
            visible: (Cast.devices.length > 0 || Cast.active) && !root.slim && !root.compact
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            width: visible ? 40 : 0
            height: 40
            icon: "screencast"
            fill: Cast.active
            iconSize: 18
            iconColor: Cast.active ? Theme.m3.primary : root.ink
            tooltip: Cast.active ? "Casting to " + (Cast.device?.name ?? "") : "Cast to a TV"
            onClicked: Panels.ccPage = "cast"
        }
    }
}
