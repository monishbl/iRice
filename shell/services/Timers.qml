pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Pomodoro (focus / break cycles) and a stopwatch. Keeps running with the
// panels closed; announces phase changes with a notification.
Singleton {
    id: root

    property int focusMinutes: 25
    property int breakMinutes: 5
    property int longBreakMinutes: 15
    property string phase: "focus" // focus | break | longBreak
    property bool running: false
    property int remaining: focusMinutes * 60
    property int completed: 0 // focus sessions finished

    readonly property int phaseLength: (phase === "focus" ? focusMinutes : phase === "break" ? breakMinutes : longBreakMinutes) * 60
    readonly property real progress: 1 - remaining / Math.max(1, phaseLength)

    property bool swRunning: false
    property real swStarted: 0
    property real swAccum: 0
    property real swNow: 0
    readonly property real swElapsed: swAccum + (swRunning ? swNow - swStarted : 0)
    property var laps: []

    function toggle() {
        running = !running;
    }
    function reset() {
        running = false;
        phase = "focus";
        remaining = focusMinutes * 60;
    }
    function skip() {
        advance();
    }
    // A phase ended (by time or Skip; not by Reset). The island announces it
    // when it can; otherwise a notification does.
    signal phaseEnded(string next)
    function advance() {
        const island = Island.enabled && Island.wants("timers");
        if (phase === "focus") {
            completed++;
            phase = completed % 4 === 0 ? "longBreak" : "break";
            if (!island)
                Quickshell.execDetached(["notify-send", "-a", "Focus", "Time for a break", (phase === "longBreak" ? longBreakMinutes : breakMinutes) + " minutes. Stand up and stretch."]);
        } else {
            phase = "focus";
            if (!island)
                Quickshell.execDetached(["notify-send", "-a", "Focus", "Break's over", focusMinutes + " minutes of focus starting now."]);
        }
        remaining = phaseLength;
        phaseEnded(phase);
    }

    function swToggle() {
        if (swRunning) {
            swAccum += Date.now() - swStarted;
            swRunning = false;
        } else {
            swStarted = Date.now();
            swNow = swStarted;
            swRunning = true;
        }
    }
    function swLap() {
        laps = [swElapsed].concat(laps).slice(0, 20);
    }
    function swReset() {
        swRunning = false;
        swAccum = 0;
        laps = [];
    }

    function format(seconds) {
        const s = Math.max(0, Math.floor(seconds));
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
    }
    function formatMs(ms) {
        const cs = Math.floor(ms / 10);
        return Math.floor(cs / 6000) + ":" + String(Math.floor(cs / 100) % 60).padStart(2, "0") + "." + String(cs % 100).padStart(2, "0");
    }

    Timer {
        interval: 1000
        running: root.running
        repeat: true
        onTriggered: {
            root.remaining--;
            if (root.remaining <= 0)
                root.advance();
        }
    }
    Timer {
        interval: 47
        running: root.swRunning
        repeat: true
        onTriggered: root.swNow = Date.now()
    }

    // qs ipc call timer toggle | skip | reset | stopwatch | stopwatchReset
    IpcHandler {
        target: "timer"
        function toggle(): void { root.toggle(); }
        function skip(): void { root.skip(); }
        function reset(): void { root.reset(); }
        function stopwatch(): void { root.swToggle(); }
        function stopwatchReset(): void { root.swReset(); }
    }
}
