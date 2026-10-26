pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs.common

// Apps playing sound: their PipeWire playback streams, the output each one
// plays to, and moving a stream to another output.
//
// Quickshell's Pipewire API can read links but can't relink a stream (it only
// sets the default sink), so moving goes through pipewire-pulse:
// `pactl move-sink-input <index> <sink>`. A sink-input's index isn't the
// PipeWire node id, so the script finds it by the node's object.id property.
// Without pactl the output chooser is hidden.
Singleton {
    id: root

    // Short event sounds (the shell's own clicks and alerts) come and go too
    // fast to mix
    readonly property var eventPlayers: ["pw-play", "pw-cat", "paplay", "aplay", "canberra-gtk-play"]

    readonly property var outputs: Pipewire.nodes.values.filter(n => n.isSink && !n.isStream && n.audio)
    // Every playback stream; track these (PwObjectTracker) so volume, mute
    // and the app details stay live
    readonly property var allStreams: Pipewire.nodes.values.filter(n => n.isSink && n.isStream && n.audio)
    readonly property var streams: allStreams.filter(n => !eventPlayers.includes(n.properties?.["application.process.binary"] ?? ""))

    readonly property bool canMove: Tools.has("pactl")

    function name(node) {
        const p = node?.properties ?? {};
        // The app's own name when it's an installed app ("Discord", not its
        // audio engine's "WEBRTC VoiceEngine")
        const entry = Apps.entryFor(String(p["application.icon-name"] || p["application.id"] || p["application.process.binary"] || p["application.name"] || "").toLowerCase());
        return entry?.name || p["application.name"] || node?.description || p["media.name"] || node?.name || "App";
    }

    function icon(node) {
        const p = node?.properties ?? {};
        return Apps.iconFor(p["application.icon-name"] || p["application.id"] || p["application.process.binary"] || (p["application.name"] ?? "").toLowerCase());
    }

    function outputName(node) {
        return node?.description || node?.nickname || node?.name || "";
    }

    function outputIcon(node) {
        return outputName(node).toLowerCase().includes("headphone") ? "headphones" : "speaker-hifi";
    }

    // The output device a stream is linked to (null while it isn't linked)
    function outputOf(stream) {
        for (const g of Pipewire.linkGroups.values)
            if (g.source === stream && g.target && g.target.isSink && !g.target.isStream)
                return g.target;
        return null;
    }

    // Moves a stream to another output. Fire and forget: the caller watches
    // outputOf() change and treats no change after a few seconds as failure.
    function move(stream, sink) {
        if (!stream || !sink || !canMove)
            return;
        Quickshell.execDetached(["sh", "-c", "idx=$(LC_ALL=C pactl list sink-inputs | awk -v id=\"$1\" '/^Sink Input #/ { n = substr($3, 2) } $1 == \"object.id\" && $3 == \"\\\"\" id \"\\\"\" { print n; exit }'); [ -n \"$idx\" ] && pactl move-sink-input \"$idx\" \"$2\"", "sh", String(stream.id), sink.name]);
    }
}
