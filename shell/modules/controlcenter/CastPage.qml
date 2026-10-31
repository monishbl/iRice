import QtQuick
import Quickshell
import qs.common
import qs.services

// Cast what's playing to a Chromecast or Google TV (services/Cast): the
// devices on this network, and the one it's playing on with Stop
DetailPage {
    id: root

    title: "Cast"

    // What would go: what's playing, or why it can't
    Text {
        width: parent.width
        leftPadding: 6
        rightPadding: 6
        bottomPadding: 4
        wrapMode: Text.WordWrap
        text: Cast.active ? "Playing on " + (Cast.device?.name ?? "") + ": " + Cast.title
            : !Media.player ? "Play something first, then choose where it goes."
            : Cast.canCastPlaying ? "Cast “" + (Media.player.trackTitle || Media.player.identity) + "” to:"
            : "“" + (Media.player.identity ?? "This app") + "” doesn't say what it's playing, so it can't be cast. Cast a video or song from Files instead."
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Repeater {
        model: ScriptModel { values: Cast.devices }
        ListRow {
            required property var modelData
            readonly property bool here: Cast.active && Cast.device?.id === modelData.id
            icon: Cast.iconFor(modelData)
            highlighted: here
            title: modelData.name
            status: here ? (Cast.state_ === "playing" ? "Playing" : Cast.state_ === "paused" ? "Paused" : "Connecting…") : modelData.model
            trailingIcon: here ? "stop" : ""
            busy: here && (Cast.state_ === "connecting" || Cast.state_ === "loading")
            enabled: here || (Cast.canCastPlaying && !!Media.player)
            opacity: enabled ? 1 : 0.5
            onClicked: here ? Cast.stop() : Cast.castPlaying(modelData)
        }
    }

    // Network discovery off (avahi): nothing can be found
    ListRow {
        visible: Needs.checked && !Needs.avahi
        icon: "wifi-high"
        title: Needs.installing || Health.fixing === "avahi" ? "Turning on…" : "Turn on network discovery"
        status: "TVs are found with avahi, which isn't running"
        busy: Needs.installing || Health.fixing === "avahi"
        onClicked: Needs.turnOnAvahi()
    }

    // None found
    Text {
        visible: Cast.devices.length === 0 && (!Needs.checked || Needs.avahi)
        width: parent.width
        leftPadding: 6
        rightPadding: 6
        topPadding: 6
        wrapMode: Text.WordWrap
        text: Tools.has("avahi-browse") ? "No Chromecast or Google TV on this network. Check that it's on and on the same Wi-Fi." : "Finding TVs needs avahi (avahi-browse)."
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
}
