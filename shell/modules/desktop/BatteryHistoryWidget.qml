import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.common

// Battery level over the last 24 hours (from UPower's history), charging
// stretches in green. Read every 5 minutes; the chart is drawn only then.
Item {
    id: root

    property string size: "m"
    property bool preview: false
    property var samples: [] // [[t, percent, charging]]
    readonly property var d: UPower.displayDevice
    readonly property real level: d ? (d.percentage > 1 ? d.percentage / 100 : d.percentage) : 0
    readonly property bool charging: d?.state === UPowerDeviceState.Charging

    Process {
        id: read
        running: true
        command: [decodeURIComponent(Qt.resolvedUrl("../../assets/scripts/battery-history.sh").toString().replace("file://", ""))]
        stdout: StdioCollector {
            onStreamFinished: {
                root.samples = text.split("\n").filter(l => l.trim()).map(l => {
                    const f = l.split(" ");
                    return [Number(f[0]), Number(f[1]), f[2] === "c"];
                });
                chart.requestPaint();
            }
        }
    }
    Timer {
        interval: 300000
        repeat: true
        running: !root.preview
        onTriggered: read.running = true
    }

    WidgetHeader { id: header; icon: root.charging ? "battery-charging" : "battery-high"; title: "Battery"; tint: root.level < 0.2 && !root.charging ? Theme.accents.red : Theme.accents.green }
    Text {
        anchors { right: parent.right; top: parent.top }
        text: Math.round(root.level * 100) + "%"
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold; features: { "tnum": 1 } }
    }

    Canvas {
        id: chart
        anchors { left: parent.left; right: parent.right; top: header.bottom; topMargin: 12; bottom: axis.top; bottomMargin: 4 }
        renderStrategy: Canvas.Cooperative
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            const w = width, h = height, now = Date.now() / 1000, from = now - 86400;
            // 0, 50 and 100% guides
            ctx.strokeStyle = Theme.color.separator;
            ctx.lineWidth = 1;
            for (const p of [0, 0.5, 1]) {
                ctx.beginPath();
                ctx.moveTo(0, Math.round(h - p * h) + 0.5);
                ctx.lineTo(w, Math.round(h - p * h) + 0.5);
                ctx.stroke();
            }
            const s = root.samples;
            if (s.length < 2)
                return;
            const X = t => (t - from) / 86400 * w, Y = p => h - p / 100 * h;
            // One filled run per charging / discharging stretch
            let i = 0;
            while (i < s.length - 1) {
                let j = i;
                while (j < s.length - 1 && s[j + 1][2] === s[i][2])
                    j++;
                const end = Math.min(j + 1, s.length - 1);
                const c = s[i][2] ? Theme.accents.green : Theme.dark ? "#A1A1AA" : "#71717A";
                ctx.beginPath();
                ctx.moveTo(X(s[i][0]), h);
                for (let k = i; k <= end; k++)
                    ctx.lineTo(X(s[k][0]), Y(s[k][1]));
                ctx.lineTo(X(s[end][0]), h);
                ctx.closePath();
                ctx.fillStyle = Qt.alpha(c, 0.28);
                ctx.fill();
                ctx.beginPath();
                for (let k = i; k <= end; k++)
                    k === i ? ctx.moveTo(X(s[k][0]), Y(s[k][1])) : ctx.lineTo(X(s[k][0]), Y(s[k][1]));
                ctx.strokeStyle = c;
                ctx.lineWidth = 1.5;
                ctx.stroke();
                i = end;
            }
        }
        Connections {
            target: Theme
            function onDarkChanged() { chart.requestPaint(); }
        }
    }
    Item {
        id: axis
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 14
        Text {
            text: root.size === "s" ? "24h" : "24 hours ago"
            color: Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
        Text {
            anchors.right: parent.right
            text: "Now"
            color: Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
        Text {
            anchors.centerIn: parent
            visible: root.samples.length < 2
            text: "No history yet"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }
}
