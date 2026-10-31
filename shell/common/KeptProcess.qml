import QtQuick
import Quickshell.Io

// A long-lived process kept running while `wanted`: started again after it
// ends (or fails to start), waiting longer each time it ends quickly (10 s,
// 20 s… up to 5 min; back to 10 s after a run of a minute or more). Turning
// `wanted` off stops it. Set `wanted`, never `running` (that is bound here).
Process {
    id: p

    property bool wanted: false
    property int firstWait: 10000
    property int maxWait: 300000

    property bool resting: false
    property int quickEnds: 0
    property real startedAt: 0

    running: wanted && !resting

    onRunningChanged: {
        if (running) {
            startedAt = Date.now();
            return;
        }
        // (ended, or never started: a missing binary emits no exited())
        if (!wanted || resting)
            return;
        if (Date.now() - startedAt >= 60000)
            quickEnds = 0;
        rest.interval = Math.min(firstWait * Math.pow(2, quickEnds), maxWait);
        quickEnds++;
        resting = true;
        rest.restart();
    }
    onWantedChanged: if (!wanted) {
        rest.stop();
        resting = false;
        quickEnds = 0;
    }

    readonly property Timer rest: Timer {
        onTriggered: p.resting = false
    }
}
