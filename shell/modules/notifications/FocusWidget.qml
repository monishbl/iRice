import QtQuick
import qs.common
import qs.modules.controlcenter
import qs.services

// Pomodoro ring with controls, and a stopwatch with laps.
Platter {
    id: root

    property int mode: 0 // 0 focus timer, 1 stopwatch
    implicitHeight: column.implicitHeight + 24

    Column {
        id: column
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 12 }
        spacing: 12

        Segmented {
            width: parent.width
            options: ["Focus", "Stopwatch"]
            current: root.mode
            onSelected: i => root.mode = i
        }

        // Pomodoro
        Row {
            visible: root.mode === 0
            spacing: 16

            Item {
                width: 110
                height: 110
                Canvas {
                    id: ring
                    anchors.fill: parent
                    readonly property real p: Timers.progress
                    readonly property color track: Theme.color.toggleOff
                    readonly property color fillColor: Timers.phase === "focus" ? Theme.accent : Theme.color.success
                    onPChanged: requestPaint()
                    onFillColorChanged: requestPaint()
                    onPaint: {
                        const ctx = getContext("2d");
                        ctx.reset();
                        const r = width / 2 - 6;
                        ctx.lineWidth = 8;
                        ctx.lineCap = "round";
                        ctx.strokeStyle = track;
                        ctx.beginPath();
                        ctx.arc(width / 2, height / 2, r, 0, 2 * Math.PI);
                        ctx.stroke();
                        if (p > 0) {
                            ctx.strokeStyle = fillColor;
                            ctx.beginPath();
                            ctx.arc(width / 2, height / 2, r, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * p);
                            ctx.stroke();
                        }
                    }
                }
                Column {
                    anchors.centerIn: parent
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Timers.format(Timers.remaining)
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold; features: { "tnum": 1 } }
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Timers.phase === "focus" ? "Focus" : "Break"
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                    }
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                Text {
                    text: Timers.completed + (Timers.completed === 1 ? " session" : " sessions") + " today"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Row {
                    spacing: 8
                    Repeater {
                        model: [
                            { icon: Timers.running ? "pause" : "play", primary: true, act: () => Timers.toggle() },
                            { icon: "skip-forward", primary: false, act: () => Timers.skip() },
                            { icon: "arrow-counter-clockwise", primary: false, act: () => Timers.reset() }
                        ]
                        CircleToggle {
                            required property var modelData
                            size: 36
                            icon: modelData.icon
                            checked: modelData.primary
                            onClicked: modelData.act()
                        }
                    }
                }
            }
        }

        // Stopwatch
        Column {
            visible: root.mode === 1
            width: parent.width
            spacing: 8
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Timers.formatMs(Timers.swElapsed)
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: 34; weight: Font.DemiBold; features: { "tnum": 1 } }
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 8
                Repeater {
                    model: [
                        { icon: Timers.swRunning ? "pause" : "play", primary: true, act: () => Timers.swToggle() },
                        { icon: "flag", primary: false, act: () => Timers.swLap() },
                        { icon: "arrow-counter-clockwise", primary: false, act: () => Timers.swReset() }
                    ]
                    CircleToggle {
                        required property var modelData
                        size: 36
                        icon: modelData.icon
                        checked: modelData.primary
                        onClicked: modelData.act()
                    }
                }
            }
            Repeater {
                model: Timers.laps
                Item {
                    required property real modelData
                    required property int index
                    width: parent.width
                    height: 20
                    Text { text: "Lap " + (Timers.laps.length - parent.index); color: Theme.color.secondary; font { family: Theme.font.family; pixelSize: Theme.font.small } }
                    Text { anchors.right: parent.right; text: Timers.formatMs(parent.modelData); color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } } }
                }
            }
        }
    }
}
