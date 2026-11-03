import QtQuick
import qs.common
import qs.services

// The speed test, for a menu or popover (the Wi-Fi menu, the network graph):
//   SpeedTestCard { width: parent.width }
// Idle: the last result and a "Test speed" button. Running: the live speed,
// the phase and a Cancel button; download, upload and ping fill in as they're
// measured. Done: the three numbers, latency under load, the server. Nothing
// runs until the button is clicked.
//   compact: only the numbers (no server, no responsiveness line)
//   settingsLink: a "Details" link to Settings → Network
Column {
    id: root

    property bool compact: false
    property bool settingsLink: true

    readonly property bool running: SpeedTest.running
    readonly property bool hasRun: SpeedTest.stage !== "idle" && SpeedTest.stage !== "error"
    // Numbers to show: this run's, or the last saved result
    readonly property var shown: hasRun ? SpeedTest : SpeedTest.last
    readonly property string phaseText: ({ latency: "Measuring ping…", download: "Testing download…", upload: "Testing upload…" })[SpeedTest.stage] ?? ""

    function ago(ms) {
        const m = Math.round((Date.now() - ms) / 60000);
        return m < 1 ? "just now" : m < 60 ? m + " min ago" : m < 1440 ? Math.round(m / 60) + " h ago" : Math.round(m / 1440) + " d ago";
    }

    spacing: 10
    Component.onCompleted: SpeedTest.probe()

    // Title, and the one action
    Item {
        width: parent.width
        height: 28
        Icon {
            id: gauge
            anchors.verticalCenter: parent.verticalCenter
            name: "gauge"
            size: 16
            color: Theme.color.secondary
        }
        Column {
            anchors { left: gauge.right; leftMargin: 8; right: action.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
            Text {
                width: parent.width
                text: "Speed test"
                elide: Text.ElideRight
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                visible: text !== ""
                text: root.running ? root.phaseText : SpeedTest.stage === "done" ? "Just now · " + SpeedTest.formatBytes(SpeedTest.usedBytes) + " used" : SpeedTest.last && SpeedTest.stage !== "error" ? "Last test " + root.ago(SpeedTest.last.at) : ""
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        Rectangle {
            id: action
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            // Material: an extra-small button, filled primary to start, tonal
            // to cancel, with state layers; its corners tighten while pressed
            readonly property color mBase: !Theme.material ? "transparent" : root.running ? Theme.m3.secondaryContainer : Theme.m3.primary
            readonly property color mInk: !Theme.material ? "transparent" : root.running ? Theme.m3.onSecondaryContainer : Theme.m3.onPrimary
            width: actionText.implicitWidth + (Theme.material ? 32 : 24)
            height: Theme.material ? 32 : 28
            // (Material's press swell; Glass shrinks by scale instead)
            transform: Theme.material ? pressFx1.effects : []
            ElasticControl { id: pressFx1; enabled: Theme.material; pressed: Theme.material && actionMouse.pressed }
            radius: Theme.material ? (actionMouse.pressed ? 8 : height / 2) : 14
            Behavior on radius {
                enabled: Theme.material
                MaterialSpring { speed: "fast"; epsilon: 0.25 }
            }
            activeFocusOnTab: true
            color: Theme.material ? Qt.tint(mBase, Qt.alpha(mInk, actionMouse.pressed ? 0.1 : actionMouse.containsMouse ? 0.08 : 0))
                : root.running ? (actionMouse.pressed ? Theme.color.fillPressed : actionMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill)
                : (actionMouse.pressed ? Qt.darker(Theme.accent, 1.2) : actionMouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent)
            scale: actionMouse.pressed && !Theme.material ? 0.97 : 1
            Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            Text {
                id: actionText
                anchors.centerIn: parent
                text: root.running ? "Cancel" : SpeedTest.stage === "done" || SpeedTest.stage === "error" ? "Test again" : "Test speed"
                color: Theme.material ? action.mInk : root.running ? Theme.color.label : Theme.color.onAccent
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
            }
            Rectangle { // focus ring (Material: 3 px in secondary, 2 px out)
                anchors { fill: parent; margins: Theme.material ? -5 : -3 }
                visible: action.activeFocus
                radius: height / 2
                color: "transparent"
                border.width: Theme.material ? 3 : 2
                border.color: Theme.material ? Theme.m3.secondary : Theme.accent
            }
            function press() {
                if (root.running)
                    SpeedTest.cancel();
                else
                    SpeedTest.start();
            }
            Keys.onSpacePressed: press()
            Keys.onReturnPressed: press()
            MouseArea {
                id: actionMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: action.press()
            }
        }
    }

    // Live: the speed right now, and how far through the phase
    Column {
        visible: root.running
        width: parent.width
        spacing: 6
        Text {
            text: SpeedTest.stage === "latency" ? SpeedTest.formatMs(SpeedTest.ping) : SpeedTest.format(SpeedTest.live)
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.display; weight: Font.DemiBold; features: { "tnum": 1 } }
        }
        Rectangle {
            visible: !Theme.material
            width: parent.width
            height: 4
            radius: 2
            color: Theme.color.fill
            Rectangle {
                width: parent.width * Math.max(0.02, Math.min(1, SpeedTest.progress))
                height: parent.height
                radius: 2
                color: Theme.accent
                Behavior on width { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }
            }
        }
        // (Material: its wavy progress bar)
        Loader {
            active: Theme.material
            visible: active
            width: parent.width
            sourceComponent: WavyProgress {
                value: Math.max(0.02, Math.min(1, SpeedTest.progress))
            }
        }
    }

    // The numbers
    Item {
        visible: root.shown !== null && SpeedTest.stage !== "error"
        width: parent.width
        height: stats.implicitHeight
        Grid {
            id: stats
            width: parent.width
            columns: 3
            readonly property real cell: width / 3
            Repeater {
                model: [
                    { label: "Download", icon: "arrow-down", value: SpeedTest.format(root.shown?.download ?? -1), active: SpeedTest.stage === "download" },
                    { label: "Upload", icon: "arrow-up", value: SpeedTest.format(root.shown?.upload ?? -1), active: SpeedTest.stage === "upload" },
                    { label: "Ping", icon: "timer", value: SpeedTest.formatMs(root.shown?.ping ?? -1), active: SpeedTest.stage === "latency" }
                ]
                Column {
                    required property var modelData
                    width: stats.cell
                    spacing: 2
                    Row {
                        spacing: 4
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: parent.parent.modelData.icon
                            bold: true
                            size: 11
                            color: parent.parent.modelData.active ? Theme.accent : Theme.color.tertiary
                        }
                        Text {
                            text: parent.parent.modelData.label
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                    Text {
                        width: parent.width - 4
                        text: parent.modelData.value
                        elide: Text.ElideRight
                        color: parent.modelData.value === "—" ? Theme.color.tertiary : Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold; features: { "tnum": 1 } }
                    }
                }
            }
        }
    }

    // Responsiveness and the server
    Text {
        visible: !root.compact && root.shown !== null && SpeedTest.stage !== "error" && text !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: {
            const s = root.shown;
            if (!s)
                return "";
            const parts = [];
            if ((s.jitter ?? -1) >= 0)
                parts.push("Jitter " + SpeedTest.formatMs(s.jitter));
            if ((s.loadedDown ?? -1) >= 0 || (s.loadedUp ?? -1) >= 0)
                parts.push("Under load " + SpeedTest.formatMs(s.loadedDown ?? -1) + " ↓ " + SpeedTest.formatMs(s.loadedUp ?? -1) + " ↑");
            return parts.join(" · ");
        }
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
    }
    Text {
        visible: !root.compact && !root.running && SpeedTest.stage !== "error" && text !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: root.hasRun ? SpeedTest.verdict(SpeedTest) : SpeedTest.verdict(SpeedTest.last)
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
    Text {
        visible: !root.compact && text !== "" && SpeedTest.stage !== "error"
        width: parent.width
        elide: Text.ElideRight
        text: {
            if (root.hasRun) {
                const srv = [SpeedTest.server.name, SpeedTest.server.location].filter(x => x).join(", ");
                return [srv, SpeedTest.client.isp].filter(x => x).join(" · ");
            }
            return [SpeedTest.last?.server, SpeedTest.last?.isp].filter(x => x).join(" · ");
        }
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }

    // Something went wrong: what, and how to fix it
    Text {
        visible: SpeedTest.stage === "error"
        width: parent.width
        wrapMode: Text.Wrap
        text: SpeedTest.error
        color: Theme.color.danger
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Text {
        visible: root.settingsLink
        text: "Speed test details…"
        color: linkMouse.containsMouse ? Theme.accent : Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small; underline: linkMouse.containsMouse }
        MouseArea {
            id: linkMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: Panels.openSettings("network")
        }
    }
}
