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
// Against burn-in (lock.standbyShift) the content glides to a new spot each
// minute and every 15 minutes the screen rests dark for a minute; the view
// dots only show for a moment after a change. Nothing else moves.
Item {
    id: root

    property int view: 0 // 0 clock, 1 widgets, 2 photos
    readonly property int views: photos.count > 0 ? 3 : 2
    readonly property bool night: !Daylight.day && (new Date().getHours() >= 22 || new Date().getHours() < 6)
    readonly property color ink: night ? "#FF3B30" : "#F5F5F7"
    readonly property color ink2: night ? Qt.rgba(1, 0.23, 0.19, 0.6) : Qt.rgba(1, 1, 1, 0.6)
    function step(d) {
        view = (view + d + views) % views;
        resting = false;
        dots.show();
        drift();
    }

    SystemClock { id: clock; precision: SystemClock.Minutes }
    readonly property string time: ((clock.date.getHours() + 11) % 12 + 1) + ":" + String(clock.date.getMinutes()).padStart(2, "0")

    // ── Burn-in protection ──────────────────────────────────────────────
    readonly property bool shift: Config.options.lock.standbyShift
    property real dx: 0
    property real dy: 0
    property bool resting: false
    property int minutes: 0
    // How far the current view's content can move and stay on screen
    readonly property size room: {
        const c = view === 0 ? clockView : view === 1 ? widgetsView : null;
        if (!c)
            return Qt.size(24, 24);
        return Qt.size(Math.max(0, (width - c.width) / 2 - 48), Math.max(0, (height - c.height) / 2 - 48));
    }
    function drift() {
        if (!shift) {
            dx = 0;
            dy = 0;
            return;
        }
        // A new spot well away from the last one, so no pixel holds the same
        // colour for long
        let x = 0, y = 0;
        for (let i = 0; i < 6; i++) {
            x = (Math.random() * 2 - 1) * room.width;
            y = (Math.random() * 2 - 1) * room.height;
            if (Math.hypot(x - dx, y - dy) > Math.min(room.width, room.height) * 0.5)
                break;
        }
        dx = x;
        dy = y;
    }
    Connections {
        target: clock
        function onDateChanged() {
            if (!root.visible || !root.shift || root.resting)
                return;
            if (++root.minutes >= 15) {
                root.minutes = 0;
                root.resting = true;
                rest.restart();
            } else {
                root.drift();
            }
        }
    }
    Timer {
        id: rest
        interval: 60000
        onTriggered: {
            root.drift(); // moves while dark, so it comes back somewhere new
            root.resting = false;
        }
    }
    onVisibleChanged: {
        minutes = 0;
        resting = false;
        rest.stop();
        if (visible) {
            drift();
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
        visible: root.view === 2
        source: root.view === 2 && photos.count > 0 ? "file://" + photos.get(root.photoIndex % photos.count, "filePath") : ""
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(root.width, root.height)
        asynchronous: true
        opacity: status === Image.Ready && !root.resting ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 800 } }
    }

    // ── Everything that stays put otherwise: moves and rests ────────────
    Item {
        id: stage
        width: root.width
        height: root.height
        x: root.dx
        y: root.dy
        // Slow enough to go unnoticed; a jump while dark
        Behavior on x { enabled: !root.resting; NumberAnimation { duration: 3000; easing.type: Easing.InOutSine } }
        Behavior on y { enabled: !root.resting; NumberAnimation { duration: 3000; easing.type: Easing.InOutSine } }
        opacity: root.resting ? 0 : 1
        Behavior on opacity { NumberAnimation { duration: 2000; easing.type: Easing.InOutQuad } }

        // Clock
        Column {
            id: clockView
            anchors.centerIn: parent
            visible: root.view === 0
            readonly property int size: Math.round(Math.min(root.height * 0.16, root.width * 0.14))
            spacing: -Math.round(size * 0.06)
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.time
                color: root.ink
                font { family: Theme.font.family; pixelSize: clockView.size; weight: Font.DemiBold; letterSpacing: -clockView.size * 0.03; features: { "tnum": 1 } }
            }
            Text {
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
        visible: root.night && root.view !== 0
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
        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 24 }
        spacing: 8
        opacity: dotsTimer.running ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 400 } }
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
