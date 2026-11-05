import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.services

// The login intro (Settings → Lock & Login Screen → Login intro; off unless
// you turn it on): as the desktop first appears after logging in, never on a
// shell reload, with the startup chime, in 1.3 s at most. It never stands in
// the way: its cover takes no clicks or keys, and Reduce motion skips it.
//   Glass: from black, your wallpaper comes into focus (blurred and a touch
//   larger, settling), then the cover lifts off the windows while the
//   widgets and icons fade in and the menu bar and dock settle in (LockFx,
//   as unlocking does, without its zoom: the wallpaper itself stays put)
//   Material: the Tessera mark (Material's cookie with the drop, as the boot
//   animation Bloom shows it) opens into a window onto the desktop that
//   grows, morphing to a circle, until it fills the screen
// It has to be up for the shell's first frame, before Config and Sounds
// have answered, so it reads the setting and the session's mark (Sounds:
// the startup chime's) itself. Settings' Play Intro and
//   ./dev.sh ipc call loginIntro play
// play it now, whatever the setting.
Scope {
    id: root

    // ── Whether to play: the first start in this session, the setting on ──
    FileView {
        id: settingsFile
        path: Config.dir + "/config.json"
        blockLoading: true
        printErrors: false
    }
    FileView {
        id: startedMark
        path: Sounds.startedMark
        blockLoading: true
        printErrors: false
    }
    FileView {
        id: oldStartedMark
        path: Sounds.oldStartedMark
        blockLoading: true
        printErrors: false
    }
    function wanted() {
        try {
            const c = JSON.parse(settingsFile.text() || "{}");
            return c.lock?.loginIntro === true && c.appearance?.reduceMotion !== true;
        } catch (e) {
            return false;
        }
    }
    Component.onCompleted: if (!startedMark.loaded && !oldStartedMark.loaded && !System.locked && wanted()) arm()

    // ── Its course ───────────────────────────────────────────────────────
    property bool shown: false // the cover is up
    property bool playing: false
    property bool chimed: false // the startup chime has played (or it's a preview)
    property bool pictureReady: false // (Glass: the wallpaper to bring into focus)
    property real t: 0 // ms into it
    readonly property int length: Theme.material ? 1000 : 1300

    function arm() {
        LockFx.prepare(false); // (fading in: zooming the wallpaper itself redraws every glass surface each frame)
        LockFx.intro = true;
        t = 0;
        shown = true;
        holdLimit.restart();
        // (tessera-ui draws it while it draws the lock: it brings its own picture)
        if (RustUi.drawing("lock")) {
            pictureReady = true;
            RustUi.tell("lock", { intro: "arm" });
        }
    }
    function maybePlay() {
        if (shown && !playing && chimed && (Theme.material || pictureReady))
            play();
    }
    function play() {
        holdLimit.stop();
        playing = true;
        clock.restart();
        revealAt.restart();
        RustUi.tell("lock", { intro: "play", pace: root.pace });
    }
    function finish() {
        clock.stop();
        revealAt.stop();
        holdLimit.stop();
        playing = false;
        shown = false;
        pictureReady = false;
        LockFx.intro = false;
        RustUi.tell("lock", { intro: "stop" });
        if (!System.locked)
            LockFx.rest();
    }
    function preview() {
        if (shown)
            return;
        arm();
        chimed = true;
        Qt.callLater(maybePlay);
    }

    Connections {
        target: Sounds
        function onSessionStarted() {
            root.chimed = true;
            root.maybePlay();
        }
    }
    Connections {
        target: LockFx
        function onPlayIntro() {
            root.preview();
        }
    }
    Connections {
        target: System
        function onLockedChanged() {
            if (root.shown)
                root.finish(); // (the lock plays its own)
        }
    }
    // Never held for long, chime or not
    Timer {
        id: holdLimit
        interval: 900
        onTriggered: if (root.shown && !root.playing) root.play()
    }
    // Everything moves by `t` alone, so it can be slowed as a whole
    // (TESSERA_INTRO_PACE=10: a tenth of the speed, to record it frame by frame)
    readonly property real pace: Math.max(1, Number((Quickshell.env("TESSERA_INTRO_PACE") || Quickshell.env("IRICE_INTRO_PACE"))) || 1)
    NumberAnimation {
        id: clock
        target: root
        property: "t"
        from: 0
        to: root.length
        duration: root.length * root.pace
        onFinished: root.finish()
    }
    // The desktop under the cover settles in (widgets, icons, menu bar, dock)
    Timer {
        id: revealAt
        interval: (Theme.material ? 150 : 350) * root.pace
        onTriggered: LockFx.reveal((Theme.material ? 800 : 900) * root.pace)
    }

    // (0 .. 1 between two moments, and ease-out)
    function span(a, b) {
        return Math.max(0, Math.min(1, (t - a) / (b - a)));
    }
    function out(x) {
        return 1 - Math.pow(1 - x, 3);
    }
    function inOut(x) {
        return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
    }
    // Material's expressive spring for shapes (ζ .6, k 200), from 0 to 1, ms after a
    function spring(a) {
        const s = Math.max(0, t - a) / 1000, w = Math.sqrt(200), z = 0.6, wd = w * Math.sqrt(1 - z * z);
        return 1 - Math.exp(-z * w * s) * (Math.cos(wd * s) + z / Math.sqrt(1 - z * z) * Math.sin(wd * s));
    }

    IpcHandler {
        target: "loginIntro"
        function play(): void {
            root.preview();
        }
    }

    Variants {
        // (tessera-ui's own while it draws the lock: src/surfaces/lock/intro.rs)
        model: root.shown && !RustUi.drawing("lock") ? Quickshell.screens : []

        PanelWindow {
            id: win
            required property var modelData
            screen: modelData
            WlrLayershell.namespace: "tessera:lockveil" // (unblurred, unanimated by the compositor)
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            exclusionMode: ExclusionMode.Ignore
            anchors { top: true; bottom: true; left: true; right: true }
            color: "transparent"
            mask: Region { width: 0; height: 0 } // (clicks go through)

            // ── Glass: the wallpaper comes into focus, then lifts away ──
            Loader {
                anchors.fill: parent
                active: !Theme.material
                sourceComponent: Item {
                    opacity: 1 - root.out(root.span(650, 1250))
                    layer.enabled: true // (fading as one: the black under the picture mustn't show through)
                    Rectangle {
                        anchors.fill: parent
                        color: "black"
                    }
                    Image {
                        id: picture
                        anchors.fill: parent
                        source: Wallpaper.source
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(win.modelData.width, win.modelData.height)
                        asynchronous: true
                        visible: false
                        onStatusChanged: if (status === Image.Ready) {
                            root.pictureReady = true;
                            root.maybePlay();
                        }
                    }
                    MultiEffect {
                        anchors.fill: parent
                        source: picture
                        scale: 1 + 0.06 * (1 - root.out(root.span(0, 1000))) // (settling: at rest by the time it lifts)
                        blurEnabled: true
                        blurMax: 64
                        blur: 1 - root.out(root.span(0, 900))
                        autoPaddingEnabled: false
                    }
                    Rectangle {
                        anchors.fill: parent
                        color: "black"
                        opacity: 1 - root.out(root.span(0, 450))
                    }
                }
            }

            // ── Material: the mark opens onto the desktop ───────────────
            Loader {
                anchors.fill: parent
                active: Theme.material
                sourceComponent: Item {
                    id: material
                    readonly property real mark: Math.round(0.175 * Math.min(width, height)) // (as the boot animation's)
                    readonly property real reach: 1.05 * Math.hypot(width, height)
                    readonly property real open: root.inOut(root.span(0, 900))
                    // cookie9, then softBurst, then a circle, each morph on
                    // Material's spring (driven by t, not MaterialShape's own)
                    readonly property string from: root.t < 520 ? "cookie9" : "softBurst"
                    readonly property string to: root.t < 250 ? "cookie9" : root.t < 520 ? "softBurst" : "circle"
                    readonly property real morph: root.t < 250 ? 1 : root.spring(root.t < 520 ? 250 : 520)
                    readonly property real spin: 90 * root.out(root.span(0, 900))
                    // The cover: the surface, and the mark, whose edge rings
                    // the opening as it grows and thins away
                    Item {
                        id: cover
                        anchors.fill: parent
                        Rectangle {
                            anchors.fill: parent
                            color: Qt.alpha(Theme.m3.surface, 1) // (opaque, whatever the look's transparency)
                        }
                        MaterialShape {
                            anchors.centerIn: parent
                            // (never smaller than the mark; a ring round the
                            // opening that thins away as it grows)
                            width: Math.max(material.mark, material.open * material.reach + 0.14 * material.mark * (1 - root.out(root.span(200, 900))))
                            height: width
                            animated: false
                            from: material.from
                            shownTo: material.to
                            progress: material.morph
                            spin: material.spin
                            color: Theme.m3.primary
                        }
                        Text {
                            anchors.centerIn: parent
                            text: "\ue604" // (Phosphor's filled circle-wavy: the Tessera badge)
                            color: Theme.m3.onPrimary
                            font { family: "Phosphor-Fill"; pixelSize: Math.round(material.mark * 0.42) }
                            opacity: 1 - root.out(root.span(0, 250))
                            scale: 1 - 0.4 * root.out(root.span(0, 250))
                        }
                    }
                    // The opening
                    Item {
                        id: opening
                        anchors.fill: parent
                        MaterialShape {
                            anchors.centerIn: parent
                            width: material.open * material.reach
                            height: width
                            animated: false
                            from: material.from
                            shownTo: material.to
                            progress: material.morph
                            spin: material.spin
                            color: "white"
                        }
                    }
                    // (live textures of the two: a hidden layer isn't redrawn)
                    ShaderEffectSource {
                        id: coverTexture
                        sourceItem: cover
                        hideSource: true
                        live: true
                        visible: false
                    }
                    ShaderEffectSource {
                        id: openingTexture
                        sourceItem: opening
                        hideSource: true
                        live: true
                        visible: false
                    }
                    MultiEffect {
                        anchors.fill: parent
                        source: coverTexture
                        maskEnabled: true
                        maskSource: openingTexture
                        maskInverted: true
                        maskThresholdMin: 0.5
                        maskSpreadAtMin: 1
                    }
                }
            }
        }
    }
}
