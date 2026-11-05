import QtQuick
import qs.common
import qs.services

// While tessera-ui draws live captions (rust.surfaces: captions), what it
// shows reaches it here, a line on its stdin (RustUi.tell): where they go,
// the screen, the words (as CaptionsWindow and CaptionLines have them), the
// bar's kept place and the playing window's. What it asks (RustUi's
// "captions" call): turn them off (the × button), keep the bar's place.
// (native/tessera-ui/src/surfaces/captions reads what this sends)
Item {
    id: root

    // CaptionsWindow's own, passed in
    property bool shown: false
    property string screen: ""
    property string latest: ""
    property string previous: ""
    property bool hint: false
    property bool dim: false
    property var videoRect: ({ x: 0, y: 0, w: 1280, h: 800 })

    readonly property bool here: RustUi.drawing("captions")
    // CaptionLines' words (the video's): why there are none yet, else the words
    readonly property string linesHint: Captions.error !== "" ? Captions.error
        : Captions.latest !== "" ? ""
        : !Captions.ready ? "Live captions need setting up: Settings → Accessibility"
        : "Captions appear here when something speaks"
    readonly property var feed: ({
            on: root.shown,
            placement: Captions.placement,
            screen: root.screen,
            latest: root.latest,
            previous: root.previous,
            hint: root.hint,
            dim: root.dim,
            bar: { x: Captions.cfg.barX ?? -1, y: Captions.cfg.barY ?? -1 },
            video: root.videoRect,
            lines: { hint: root.linesHint, previous: Captions.previous, latest: Captions.latest }
        })
    function send() {
        RustUi.tell("captions", { state: root.feed });
    }
    onFeedChanged: if (here) send()
    onHereChanged: if (here) send()
    Connections {
        target: RustUi
        function onStarted() {
            root.send();
        }
        function onDrawnChanged() {
            if (root.here)
                root.send();
        }
        function onFeedCall(surface, what, a) {
            if (surface !== "captions")
                return;
            if (what === "off") {
                Captions.cfg.enabled = false;
            } else if (what === "place" && typeof a[0] === "number" && typeof a[1] === "number") {
                Captions.cfg.barX = Math.max(0, Math.round(a[0]));
                Captions.cfg.barY = Math.max(0, Math.round(a[1]));
            }
        }
    }
}
