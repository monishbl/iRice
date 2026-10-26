pragma Singleton
import QtQuick
import Quickshell
import qs.common
import qs.services

// What each kind of live activity says and offers, shared by the collapsed
// pill (IslandLive), the stacked rows (IslandActivity) and the open card
// (IslandFocusCard). Music's richer player lives in those views themselves.
Singleton {
    id: root

    // Seconds, only while something counts up
    SystemClock {
        id: clock
        precision: SystemClock.Seconds
        enabled: ScreenTools.recording || Calls.active
    }
    function since(t) {
        return Timers.format(Math.max(0, Math.floor((clock.date - t) / 1000)));
    }

    function icon(kind) {
        switch (kind) {
        case "recording":
            return "record";
        case "timer":
            return Timers.phase === "focus" ? "timer" : "coffee";
        case "stopwatch":
            return "timer";
        case "transfer": {
            const k = Transfers.first?.kind;
            return k === "copy" ? "copy" : k === "send" ? "paper-plane-tilt" : "download-simple";
        }
        case "call":
            return Privacy.camera ? "video-camera" : "phone";
        }
        return "music-notes";
    }

    function title(kind) {
        switch (kind) {
        case "music":
            return Island.player?.trackTitle || "Playing";
        case "timer":
            return Timers.phase === "focus" ? "Focus" : "Break";
        case "stopwatch":
            return "Stopwatch";
        case "recording":
            return "Recording";
        case "transfer": {
            const n = Transfers.items.length;
            return (Transfers.first?.name ?? "") + (n > 1 ? " and " + (n - 1) + " more" : "");
        }
        case "call":
            return Calls.call?.name ?? "Call";
        }
        return "";
    }

    // The second line in the stacked rows and the open card
    function detail(kind) {
        switch (kind) {
        case "music":
            return Island.player?.trackArtist ?? "";
        case "timer":
            return Timers.format(Timers.remaining) + " left";
        case "stopwatch":
            return Timers.format(Math.floor(Timers.swElapsed / 1000));
        case "recording":
            return since(ScreenTools.recordingStarted);
        case "transfer": {
            const f = Transfers.first;
            if (!f)
                return "";
            const what = f.kind === "copy" ? (f.cmd === "mv" ? "Moving" : "Copying") : f.kind === "send" ? (f.waiting ? "Waiting for " + f.peer : "Sending to " + f.peer) : f.kind === "receive" ? "Receiving from " + f.peer : "Downloading";
            const amount = f.total > 0 ? Transfers.size(f.done) + " of " + Transfers.size(f.total) : Transfers.size(f.done);
            return what + " · " + amount + (Transfers.speed > 0 ? " · " + Transfers.size(Transfers.speed) + "/s" : "");
        }
        case "call":
            return since(Calls.started) + (Audio.micMuted ? " · Muted" : "");
        }
        return "";
    }

    // The short text in the collapsed pill
    function time(kind) {
        switch (kind) {
        case "timer":
            return Timers.format(Timers.remaining);
        case "stopwatch":
            return Timers.format(Math.floor(Timers.swElapsed / 1000));
        case "recording":
            return since(ScreenTools.recordingStarted);
        case "transfer":
            return Transfers.progress >= 0 ? Math.round(Transfers.progress * 100) + "%" : Transfers.label(Transfers.first);
        case "call":
            return since(Calls.started);
        }
        return "";
    }

    // 0..1 for a progress bar, or -1 for none
    function progress(kind) {
        return kind === "timer" ? Timers.progress : kind === "transfer" ? Transfers.progress : -1;
    }

    // Every control a kind has, keyed by piece name: the pill shows the pieces
    // switched on in Settings, rows leave out "reset", the card shows all
    function controls(kind) {
        const p = Island.player;
        switch (kind) {
        case "music":
            return [
                { piece: "prev", icon: "skip-back", fill: true, act: () => p?.previous(), on: p?.canGoPrevious ?? false },
                { piece: "play", icon: Island.musicPlaying ? "pause" : "play", fill: true, act: () => p?.togglePlaying(), on: true },
                { piece: "next", icon: "skip-forward", fill: true, act: () => p?.next(), on: p?.canGoNext ?? false }
            ];
        case "timer":
            return [
                { piece: "reset", icon: "arrow-counter-clockwise", fill: false, act: () => Timers.reset(), on: true },
                { piece: "play", icon: Timers.running ? "pause" : "play", fill: true, act: () => Timers.toggle(), on: true },
                { piece: "skip", icon: "skip-forward", fill: true, act: () => Timers.skip(), on: true }
            ];
        case "stopwatch":
            return [
                { piece: "reset", icon: "arrow-counter-clockwise", fill: false, act: () => Timers.swReset(), on: !Timers.swRunning },
                { piece: "lap", icon: "flag", fill: false, act: () => Timers.swLap(), on: Timers.swRunning },
                { piece: "play", icon: Timers.swRunning ? "pause" : "play", fill: true, act: () => Timers.swToggle(), on: true }
            ];
        case "recording":
            return [{ piece: "stop", icon: "stop", fill: true, act: () => ScreenTools.stopRecording(), on: true }];
        case "transfer": {
            const f = Transfers.first;
            return f?.kind === "copy" || f?.kind === "send" ? [{ piece: "cancel", icon: "x", fill: false, act: () => Transfers.cancel(f), on: true }] : [{ piece: "open", icon: "folder-open", fill: false, act: () => Transfers.openDownloads(), on: true }];
        }
        case "call":
            return [
                { piece: "mute", icon: Audio.micMuted ? "microphone-slash" : "microphone", fill: true, act: () => Audio.toggleMic(), on: true },
                { piece: "open", icon: "arrow-square-out", fill: false, act: () => Calls.open(), on: !!Calls.call?.window },
                ...(Calls.call?.leave ? [{ piece: "leave", icon: "phone-x", fill: true, act: () => Calls.leave(), on: true, danger: true }] : [])
            ];
        }
        return [];
    }
}
