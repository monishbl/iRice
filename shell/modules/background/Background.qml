import QtQuick
import QtQuick.Effects
import QtMultimedia
import Quickshell
import Quickshell.Services.UPower
import Quickshell.Wayland
import qs.common
import qs.services
import qs.glass
import qs.modules.desktop

// Wallpaper on the background layer of every screen, with the desktop widgets.
// Also renders the blurred backdrop that liquid glass refracts, once per
// wallpaper change, into a small cached image every glass window shares
// (see Backdrop.qml). This window is itself a glass host for the widgets.
// While tessera-ui draws the picture (rust.surfaces: background; its own
// window on the background layer), this one moves up to the bottom layer,
// clear, and keeps the rest: widgets, icons, the glass backdrop, the menu
// bar's sampling, and a live wallpaper's video (tessera-ui shows its poster).
Variants {
    model: Quickshell.screens

    PanelWindow {
        id: win
        required property var modelData
        screen: modelData

        // tessera-ui draws the picture, under this window
        readonly property bool rust: RustUi.drawing("background")
        WlrLayershell.namespace: "tessera:wallpaper"
        WlrLayershell.layer: rust ? WlrLayer.Bottom : WlrLayer.Background
        // A note on the desktop takes typing (focus only when clicked)
        WlrLayershell.keyboardFocus: desktopWidgets.visible && desktopWidgets.wantsKeys ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        exclusionMode: ExclusionMode.Ignore
        anchors { top: true; bottom: true; left: true; right: true }
        color: rust ? "transparent" : "black"

        // On the bottom layer this window sits over Stage Manager's strip
        // (made before it moved there): the strip's column stays the strip's
        mask: rust && StageManager.enabled && Config.options.stageManager.strip ? stripHole : null
        Region {
            id: stripHole
            width: win.width
            height: win.height
            Region {
                intersection: Intersection.Subtract
                width: 176 // (StageStrip.qml's width)
                height: win.height
            }
        }

        // tessera-ui hears the day's sunrise and sunset from here (the
        // weather service knows them): the time of day's picture, and the
        // picker's ring round it
        function tellSun() {
            if (win.screen !== Quickshell.screens[0])
                return;
            RustUi.tell("background", { sunrise: Daylight.sunrise, sunset: Daylight.sunset });
            RustUi.tell("wallpaper", { sunrise: Daylight.sunrise, sunset: Daylight.sunset });
        }
        onRustChanged: if (rust)
            Wallpaper.started = true
        Connections {
            target: RustUi
            function onDrawnChanged() {
                win.tellSun();
            }
        }
        Connections {
            target: Daylight
            function onSunriseChanged() {
                win.tellSun();
            }
            function onSunsetChanged() {
                win.tellSun();
            }
        }
        // ...and the lock's settle (LockFx: the wallpaper a touch larger,
        // easing back as the desktop is revealed)
        Connections {
            target: LockFx
            enabled: win.rust && win.screen === Quickshell.screens[0]
            function onStepped(step, ms) {
                RustUi.tell("background", { settle: step, zoom: LockFx.zoom, ms: ms });
            }
        }

        // Glass host contract (see Glass.qml)
        readonly property real screenX: 0
        readonly property real screenY: 0
        readonly property var backdrop: backdropItem.texture
        Backdrop {
            id: backdropItem
            screen: win.screen
        }

        // Two buffers: a new picture decodes in the hidden one (off the GUI
        // thread) and fades in over the old one, which keeps showing until
        // then; the old one is released afterwards. No blank frames.
        // Lock and unlock (LockFx): the wallpaper settles from a touch larger;
        // widgets and icons zoom on their own (WidgetFrame, DesktopIcons)

        Item {
            id: wallpaper
            anchors.fill: parent
            scale: LockFx.wallpaperScale
            visible: !win.rust
            // Opaque behind the pictures, so the first fade-in comes from
            // black and never shows the blur bake underneath
            Rectangle {
                anchors.fill: parent
                color: "black"
            }
            property int front: 0
            readonly property var buffers: [bufA, bufB]
            // (nothing while tessera-ui draws it: both pictures let go)
            readonly property url want: win.rust ? "" : Wallpaper.source
            readonly property bool ready: buffers[front].status === Image.Ready
            onWantChanged: win.rust ? release() : load()
            function release() {
                firstFade.stop();
                fadeIn.stop();
                bufA.source = "";
                bufB.source = "";
                bufA.opacity = 0;
                bufB.opacity = 0;
            }
            Component.onCompleted: bufA.source = want
            function load() {
                const cur = buffers[front];
                if (cur.source === want)
                    return;
                const back = buffers[1 - front];
                back.opacity = 0;
                back.z = 1;
                cur.z = 0;
                back.source = want;
            }
            function swap(img) {
                if (img === buffers[front] || img.source !== want)
                    return;
                const old = buffers[front];
                front = 1 - front;
                fadeIn.target = img;
                fadeIn.oldBuffer = old;
                fadeIn.restart();
            }
            // The first picture fades in from black (see Wallpaper.started)
            Image {
                id: bufA
                anchors.fill: parent
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(width, height)
                asynchronous: true
                cache: false
                opacity: 0
                onStatusChanged: {
                    if (status === Image.Ready) {
                        if (wallpaper.buffers[wallpaper.front] !== bufA)
                            wallpaper.swap(bufA);
                        else if (opacity < 1)
                            firstFade.start();
                        else
                            bakeTimer.restart();
                    } else if (status === Image.Error && opacity < 1) {
                        Wallpaper.started = true;
                    }
                }
            }
            Image {
                id: bufB
                anchors.fill: parent
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(width, height)
                asynchronous: true
                cache: false
                opacity: 0
                onStatusChanged: if (status === Image.Ready) wallpaper.buffers[wallpaper.front] === bufB ? bakeTimer.restart() : wallpaper.swap(bufB)
            }
            // (a screen plugged in later fades in quickly: its connect effect
            // already covers it, and with the effect off a long fade from
            // black would just look like a black screen)
            NumberAnimation {
                id: firstFade
                target: bufA
                property: "opacity"
                to: 1
                duration: Wallpaper.started ? 120 : 450
                easing.type: Easing.OutCubic
                onStarted: bakeTimer.restart()
                onFinished: Wallpaper.started = true
            }
            NumberAnimation {
                id: fadeIn
                property var oldBuffer: null
                property: "opacity"
                from: 0
                to: 1
                duration: 700
                easing.type: Easing.InOutQuad
                onStarted: bakeTimer.restart()
                // Release the old picture, unless a newer change is already
                // loading into that buffer
                onFinished: if (oldBuffer && oldBuffer !== wallpaper.buffers[wallpaper.front] && oldBuffer.source !== wallpaper.want) oldBuffer.source = ""
            }
        }

        // Live wallpaper: muted, looping; paused while windows cover the
        // desktop, the screen is locked, nobody has touched anything for two
        // minutes (the display dims or turns off), and on battery if set, so
        // it costs nothing when unseen. (Also while tessera-ui draws the
        // wallpaper: it has no video of its own, only the poster under this)
        Loader {
            anchors.fill: parent
            scale: wallpaper.scale
            active: Wallpaper.liveVideo !== ""
            sourceComponent: Item {
                id: live
                readonly property bool paused: Desktop.mostlyCovered(win.screen) || System.locked || away.isIdle || (Config.options.wallpaper.livePauseOnBattery && UPower.onBattery) || CoolDown.pauseLive
                IdleMonitor {
                    id: away
                    timeout: 120
                    respectInhibitors: true // (a video playing elsewhere keeps it going)
                }
                onPausedChanged: paused ? player.pause() : player.play()
                MediaPlayer {
                    id: player
                    source: "file://" + Wallpaper.liveVideo
                    loops: MediaPlayer.Infinite
                    videoOutput: video
                    Component.onCompleted: if (!live.paused) play()
                }
                VideoOutput {
                    id: video
                    anchors.fill: parent
                    fillMode: VideoOutput.PreserveAspectCrop
                }
            }
        }

        // Quarter-resolution blur, hidden behind the opaque wallpaper and
        // saved once (grabbing needs a rendered item; beside the screen
        // while this window is clear)
        Item {
            id: blurred
            z: -1
            x: win.rust ? -width - 64 : 0
            width: Math.max(1, Math.round(win.width / 4))
            height: Math.max(1, Math.round(win.height / 4))

            Image {
                id: small
                anchors.fill: parent
                source: Wallpaper.source
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(parent.width * 2, parent.height * 2)
                asynchronous: true
                visible: false
            }
            MultiEffect {
                anchors.fill: parent
                source: small
                autoPaddingEnabled: false
                blurEnabled: true
                blur: 1.0
                blurMax: 24
                blurMultiplier: 0.6
            }
        }

        // Is the strip under the menu bar light? (decides dark or light menu bar
        // text) Sampled from the quarter-size blur, not the full picture.
        Canvas {
            id: strip
            z: -2
            x: win.rust ? -width - 64 : 0
            width: 64
            height: 64
            property string src: ""
            onImageLoaded: requestPaint()
            onPaint: {
                if (src === "" || !isImageLoaded(src))
                    return;
                const ctx = getContext("2d");
                // Whole image squeezed into 64x64; the top two rows are its top 3%
                ctx.drawImage(src, 0, 0, 64, 64);
                const d = ctx.getImageData(0, 0, 64, 2).data;
                let sum = 0;
                for (let i = 0; i < d.length; i += 4)
                    sum += 0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2];
                Wallpaper.topIsLight = sum / (d.length / 4) / 255 > 0.6;
                unloadImage(src);
            }
        }

        Timer {
            id: bakeTimer
            interval: 300 // let the small image load and the blur render
            onTriggered: {
                if (small.status === Image.Error)
                    return; // unloadable picture: don't retry forever
                // Not while this screen's connect effect plays: grabbing and
                // saving holds up the GUI thread, which would stall it
                if (DisplayFx.runs.some(r => r.screen === win.screen?.name)) {
                    restart();
                    return;
                }
                if (small.status !== Image.Ready) {
                    restart();
                    return;
                }
                const path = Wallpaper.backdropPath(win.screen?.name ?? "default");
                blurred.grabToImage(result => {
                    if (result.saveToFile(path))
                        Wallpaper.backdropBaked(win.screen?.name ?? "default");
                    strip.src = result.url;
                    strip.loadImage(result.url);
                });
            }
        }
        onWidthChanged: bakeTimer.restart()

        // Widgets live on the first screen (one layout; macOS shows them on one display)
        DesktopWidgets {
            id: desktopWidgets
            visible: win.screen === Quickshell.screens[0] && Wallpaper.started
            enabled: visible
            screen: win.screen
        }

        // Measures app icons for squircle tiles (hidden behind the wallpaper)
        Loader {
            z: -3
            x: win.rust ? -96 : 0
            active: win.screen === Quickshell.screens[0]
            sourceComponent: IconAnalyzer {}
        }
    }
}
