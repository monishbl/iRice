pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Pipewire

// Default output and input, with volume helpers.
Singleton {
    id: root

    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var source: Pipewire.defaultAudioSource
    readonly property real volume: sink?.audio?.volume ?? 0
    readonly property bool muted: sink?.audio?.muted ?? false
    readonly property bool micMuted: source?.audio?.muted ?? false
    readonly property real micVolume: source?.audio?.volume ?? 0
    // What a device is called ("realme TechLife Buds T100", "Built-in Audio Analog Stereo")
    function nameOf(node) {
        return node?.nickname || node?.description || node?.name || "";
    }
    readonly property string icon: muted || volume === 0 ? "speaker-x" : volume < 0.34 ? "speaker-none" : volume < 0.67 ? "speaker-low" : "speaker-high"

    // Changes made with Tessera's own sliders and buttons (`quiet`) don't pop
    // up the volume bar (Osd): you're looking at the control already. Keys,
    // the menu bar's scroll and other apps' changes still do.
    property real quietUntil: 0
    function hush() {
        quietUntil = Date.now() + 1000;
        RustUi.tell("osd", { hush: "sound" }); // (tessera-ui's OSD, when it draws it)
    }
    function setVolume(v, quiet) {
        if (quiet)
            hush();
        if (sink?.audio) {
            sink.audio.muted = false;
            sink.audio.volume = Math.max(0, Math.min(1, v));
        }
    }
    function setMicVolume(v, quiet) {
        if (quiet)
            hush();
        if (source?.audio) {
            source.audio.muted = false;
            source.audio.volume = Math.max(0, Math.min(1, v));
        }
    }
    function toggleMute(quiet) {
        if (quiet)
            hush();
        if (sink?.audio)
            sink.audio.muted = !sink.audio.muted;
    }
    function toggleMic(quiet) {
        if (quiet)
            hush();
        if (source?.audio)
            source.audio.muted = !source.audio.muted;
    }

    PwObjectTracker {
        objects: [root.sink, root.source]
    }
}
