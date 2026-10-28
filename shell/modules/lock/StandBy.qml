import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import qs.common
import qs.services
import qs.modules.desktop

// StandBy (shown on the lock screen while charging and left alone, like
// iPhone): a clock, two widgets side by side, or your photos. Swipe, scroll
// or use the arrow keys to change; anything else brings back the password
// field (the owner handles that). At night everything turns a dim red.
// Against burn-in (lock.standbyShift) the content glides up or down (always
// centred across) to a new spot each
// minute and every 15 minutes the screen rests dark for a minute (timing
// shared by every screen, from Lock.qml); the view dots only show for a
// moment after a change. Nothing else moves. A screen left out of
// lock.standbyScreens stays dark (`blank`).
Item {
    id: root

    required property var lockScope
    property bool blank: false
    // Lock → StandBy morph: the owner flies its own clock here, so this one
    // holds back until it lands (clockFade 0 .. 1)
    property real clockFade: 1 // the time
    property real dateFade: 1 // the line under it
    readonly property var cfg: Config.options.lock
    readonly property int clockWeight: cfg.standbyClockWeight === "bold" ? Font.DemiBold : cfg.standbyClockWeight === "regular" ? Font.Normal : Font.ExtraLight
    readonly property real clockOpacity: Math.max(0.2, Math.min(1, cfg.standbyClockOpacity))
    readonly property int clockSize: clockView.size
    // Where the big clock's time sits once the content has settled, in this
    // item's coordinates (the owner's morph flies to it)
    readonly property rect clockTarget: {
        const w = timeText.implicitWidth, h = timeText.implicitHeight;
        const cy = (height - clockView.implicitHeight) / 2 + dy;
        return Qt.rect((width - w) / 2 + dx, cy, w, h);
    }

    property int view: 0 // 0 clock, 1 widgets, 2 photos
    readonly property int views: photos.count > 0 ? 3 : 2
    readonly property bool night: !Daylight.day && (new Date().getHours() >= 22 || new Date().getHours() < 6)
    readonly property color ink: night ? "#FF3B30" : "#F5F5F7"
    readonly property color ink2: night ? Qt.rgba(1, 0.23, 0.19, 0.6) : Qt.rgba(1, 1, 1, 0.6)
    function step(d) {
        view = (view + d + views) % views;
        lockScope.endRest();
        dots.show();
        drift();
    }

    SystemClock { id: clock; precision: SystemClock.Minutes }
    readonly property string time: ((clock.date.getHours() + 11) % 12 + 1) + ":" + String(clock.date.getMinutes()).padStart(2, "0")

    // ── Burn-in protection ──────────────────────────────────────────────
    readonly property bool shift: cfg.standbyShift
    readonly property bool resting: lockScope.resting
    property real dx: 0
    property real dy: 0
    property bool jump: false // place without gliding (on showing)
    property real ownFx: 0
    property real ownFy: 0
    // How far the current view's content can move and stay on screen
    readonly property size room: {
        const c = view === 0 ? clockView : view === 1 ? widgetsView : null;
        if (!c)
            return Qt.size(24, 24);
        return Qt.size(Math.max(0, (width - c.width) / 2 - 48), Math.max(0, (height - c.height) / 2 - 48));
    }
    // Every screen in the same spot (lock.standbySameClock), else each its own
    function drift() {
        if (!shift) {
            dx = 0;
            dy = 0;
            return;
        }
        // Only up and down: the content always stays centred across
        let fy = lockScope.fy;
        if (!cfg.standbySameClock) {
            for (let i = 0; i < 8; i++) {
                fy = Math.random() * 2 - 1;
                if (Math.abs(fy - ownFy) > 0.5)
                    break;
            }
            ownFy = fy;
        }
        dx = 0;
        dy = fy * room.height;
    }
    Connections {
        target: root.lockScope
        function onDriftsChanged() { root.drift(); }
    }
    onVisibleChanged: {
        if (visible) {
            jump = true;
            drift();
            jump = false;
            dots.show();
        }
    }
    onShiftChanged: drift()

    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    // ── Photos (full screen; they change on their own) ──────────────────
    FolderListModel {
        id: photos
        folder: "file://" + (Config.options.desktop.photosFolder || (Quickshell.env("HOME") + "/Pictures"))
        nameFilters: ["*.jpg", "*.jpeg", "*.png", "*.webp"]
        showDirs: false
    }
    property int photoIndex: 0
    Timer {
        running: root.view === 2 && root.visible && photos.count > 1
        interval: 20000
        repeat: true
        onTriggered: root.photoIndex = Math.floor(Math.random() * photos.count)
    }
    Image {
        id: photo
        anchors.fill: parent
        visible: root.view === 2 && !root.blank
        source: root.view === 2 && photos.count > 0 ? "file://" + photos.get(root.photoIndex % photos.count, "filePath") : ""
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(root.width, root.height)
        asynchronous: true
        opacity: status === Image.Ready && !root.resting ? 1 : 0
        Behavior on opacity { OpacityAnimator { duration: 800 } }
    }

    // ── Everything that stays put otherwise: moves and rests ────────────
    Item {
        id: stage
        visible: !root.blank
        width: root.width
        height: root.height
        x: root.dx
        y: root.dy
        // Slow enough to go unnoticed; a jump while dark
        Behavior on x { enabled: !root.resting && !root.jump; XAnimator { duration: 3000; easing.type: Easing.InOutSine } }
        Behavior on y { enabled: !root.resting && !root.jump; YAnimator { duration: 3000; easing.type: Easing.InOutSine } }
        opacity: root.resting ? 0 : 1
        Behavior on opacity { OpacityAnimator { duration: 2000; easing.type: Easing.InOutQuad } }

        // Clock
        Column {
            id: clockView
            anchors.centerIn: parent
            visible: root.view === 0
            readonly property int size: Math.round(Math.min(root.height * 0.16, root.width * 0.14))
            spacing: -Math.round(size * 0.06)
            Text {
                id: timeText
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.time
                color: root.ink
                opacity: root.clockOpacity * root.clockFade
                font { family: Theme.font.family; pixelSize: clockView.size; weight: root.clockWeight; letterSpacing: -clockView.size * 0.03; features: { "tnum": 1 } }
            }
            Text {
                opacity: root.clockOpacity * root.dateFade
                anchors.horizontalCenter: parent.horizontalCenter
                text: Qt.formatDate(clock.date, "dddd d MMMM") + (Weather.ready ? "   " + Weather.temp + "° " + Weather.condition : "")
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Math.max(16, Math.round(clockView.size * 0.14)); weight: Font.Medium }
            }
        }

        // Widgets: an analog clock and the calendar
        Row {
            id: widgetsView
            anchors.centerIn: parent
            visible: root.view === 1
            spacing: root.width * 0.03
            readonly property real side: Math.min(root.height * 0.38, root.width * 0.26)
            AnalogClock {
                width: parent.side
                height: parent.side
                hours: clock.date.getHours()
                minutes: clock.date.getMinutes()
                night: true
            }
            Rectangle {
                width: parent.side
                height: parent.side
                radius: 28
                color: "#1C1C1E"
                Loader {
                    anchors.fill: parent
                    anchors.margins: 20
                    active: root.view === 1
                    source: Qt.resolvedUrl("../desktop/CalendarWidget.qml")
                    onLoaded: {
                        item.size = "l";
                        if (item.preview !== undefined)
                            item.preview = true;
                    }
                }
            }
        }

        // Photos: a small clock in the corner
        Text {
            visible: root.view === 2
            anchors { right: parent.right; bottom: parent.bottom; margins: 48 }
            text: root.time
            color: "white"
            style: Text.Raised
            styleColor: Qt.rgba(0, 0, 0, 0.5)
            font { family: Theme.font.family; pixelSize: 40; weight: Font.DemiBold; features: { "tnum": 1 } }
        }
    }

    // Night: everything a dim red (multiplied over the widgets and photos)
    Rectangle {
        anchors.fill: parent
        visible: root.night && root.view !== 0 && !root.blank
        color: "#FF2010"
        opacity: 0.55
    }
    Rectangle {
        anchors.fill: parent
        visible: root.night
        color: "black"
        opacity: 0.35
    }

    // Which view: shown for a moment after a change, then out of the way
    Row {
        id: dots
        visible: !root.blank
        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 24 }
        spacing: 8
        opacity: dotsTimer.running ? 1 : 0
        Behavior on opacity { OpacityAnimator { duration: 400 } }
        function show() {
            dotsTimer.restart();
        }
        Timer { id: dotsTimer; interval: 4000 }
        Repeater {
            model: root.views
            Rectangle {
                required property int index
                width: 7
                height: 7
                radius: 3.5
                color: root.ink
                opacity: index === root.view ? 0.9 : 0.3
            }
        }
    }
}
