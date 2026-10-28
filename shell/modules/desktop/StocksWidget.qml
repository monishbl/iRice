import QtQuick
import QtQuick.Shapes
import Quickshell
import Quickshell.Io
import qs.common

// Stocks: today's price and change for your symbols (Settings → Desktop),
// with a small chart of the day. Refreshed every 15 minutes while on the
// desktop (Yahoo Finance's public data, which can be delayed).
Item {
    id: root

    property string size: "m"
    property bool preview: false
    readonly property var symbols: Config.options.desktop.stocks
    readonly property int rows: size === "l" ? 6 : size === "m" ? 3 : 2
    property var quotes: ({}) // symbol -> { s, n, p, pc, cur, c }
    property bool failed: false

    Process {
        id: fetch
        command: [decodeURIComponent(Qt.resolvedUrl("../../assets/scripts/stocks.sh").toString().replace("file://", ""))].concat(root.symbols.slice(0, 6))
        stdout: SplitParser {
            onRead: line => {
                try {
                    const q = JSON.parse(line);
                    root.quotes = Object.assign({}, root.quotes, { [q.s]: q });
                    root.failed = false;
                } catch (e) {}
            }
        }
        onExited: if (Object.keys(root.quotes).length === 0) root.failed = true
    }
    Timer {
        interval: 15 * 60000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: fetch.running = true
    }
    onSymbolsChanged: fetch.running = true

    WidgetHeader { id: header; icon: "chart-line-up"; title: "Stocks"; tint: Theme.accent }

    Column {
        anchors { left: parent.left; right: parent.right; top: header.bottom; topMargin: 10 }
        spacing: root.size === "s" ? 8 : 6
        Text {
            visible: root.failed
            width: parent.width
            wrapMode: Text.Wrap
            text: "Couldn't load prices. Check your connection."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Repeater {
            model: root.symbols.slice(0, root.rows)
            Item {
                id: row
                required property string modelData
                readonly property var q: root.quotes[modelData] ?? null
                readonly property real change: q && q.pc ? (q.p - q.pc) / q.pc : 0
                readonly property color tone: change >= 0 ? Theme.accents.green : Theme.accents.red
                // Readable on its own tint (semantic text-on-tint colours)
                readonly property color toneText: change >= 0 ? (Theme.dark ? "#4ADE80" : "#15803D") : (Theme.dark ? "#F87171" : "#B91C1C")
                width: parent.width
                height: root.size === "s" ? 30 : 32
                Column {
                    width: parent.width * (root.size === "s" ? 0.55 : 0.36)
                    anchors.verticalCenter: parent.verticalCenter
                    Text {
                        width: parent.width
                        text: row.modelData.replace(/^\^/, "")
                        elide: Text.ElideRight
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        visible: root.size !== "s"
                        text: row.q?.n ?? ""
                        elide: Text.ElideRight
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                }
                // The day so far
                Shape {
                    id: spark
                    visible: root.size !== "s" && (row.q?.c?.length ?? 0) > 1
                    x: parent.width * 0.38
                    width: parent.width * 0.3
                    height: parent.height - 8
                    anchors.verticalCenter: parent.verticalCenter
                    preferredRendererType: Shape.CurveRenderer
                    readonly property var pts: row.q?.c ?? []
                    readonly property real lo: Math.min(...pts, row.q?.pc ?? Infinity)
                    readonly property real hi: Math.max(...pts, row.q?.pc ?? -Infinity)
                    ShapePath {
                        strokeColor: row.tone
                        strokeWidth: 1.5
                        fillColor: "transparent"
                        PathPolyline {
                            path: spark.pts.map((v, i) => Qt.point(i / Math.max(1, spark.pts.length - 1) * spark.width, spark.height - (v - spark.lo) / Math.max(1e-9, spark.hi - spark.lo) * spark.height))
                        }
                    }
                }
                Column {
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    Text {
                        anchors.right: parent.right
                        text: row.q ? row.q.p.toLocaleString(Qt.locale(), "f", row.q.p < 10 ? 3 : 2) : "—"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                    Rectangle {
                        anchors.right: parent.right
                        width: chg.implicitWidth + 10
                        height: 16
                        radius: 5
                        color: row.q ? Qt.alpha(row.tone, Theme.dark ? 0.22 : 0.16) : Theme.color.fill
                        Text {
                            id: chg
                            anchors.centerIn: parent
                            text: row.q ? (row.change >= 0 ? "+" : "") + (row.change * 100).toFixed(2) + "%" : "…"
                            color: row.q ? row.toneText : Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold; features: { "tnum": 1 } }
                        }
                    }
                }
            }
        }
    }
}
