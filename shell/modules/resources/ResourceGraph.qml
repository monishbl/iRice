import QtQuick
import qs.common

// A history graph for Resources' stats (CPU, memory, network…): the last
// minute, newest at the right edge. Drawn as an area (a smooth line with a
// soft fill), bars or a line (Settings → Desktop → Resource graphs). Repaints
// only when its values change, so at most once a second.
//   ResourceGraph { values: Resources.cpuHistory; color: Theme.accents.blue }
Canvas {
    id: root

    property var values: []
    property int slots: 60 // a full minute spans the width
    property real maximum: 1 // the graph's top; 0 scales to the largest value
    property color color: Theme.accent
    property string graphStyle: Config.options.resources.graphStyle
    property real lineWidth: 1.5

    // Network has no fixed top: a little room above the peak, never so
    // little that idle noise fills the graph
    readonly property real ceiling: maximum > 0 ? maximum : Math.max(65536, Math.max(...values) * 1.15)

    onValuesChanged: requestPaint()
    onGraphStyleChanged: requestPaint()
    onColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    // A hidden canvas doesn't paint: catch up when shown
    onVisibleChanged: if (visible) requestPaint()

    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        const n = Math.min(values.length, slots);
        if (n === 0 || width <= 0 || height <= 0)
            return;
        const vals = values.slice(values.length - n);
        const frac = v => Math.max(0, Math.min(1, (v || 0) / ceiling));

        if (graphStyle === "bars") {
            const w = width / slots;
            const gap = w >= 4 ? 1 : 0;
            ctx.fillStyle = color;
            for (let i = 0; i < n; i++) {
                const h = Math.max(1, frac(vals[i]) * height);
                ctx.fillRect(width - (n - i) * w + gap / 2, height - h, w - gap, h);
            }
            return;
        }

        if (n < 2)
            return;
        const step = width / (slots - 1);
        const pad = lineWidth / 2;
        const pts = vals.map((v, i) => [width - (n - 1 - i) * step, height - pad - frac(v) * (height - 2 * pad)]);
        // Smooth: a curve through each segment's midpoint
        const trace = () => {
            ctx.moveTo(pts[0][0], pts[0][1]);
            for (let i = 1; i < n; i++) {
                const mx = (pts[i - 1][0] + pts[i][0]) / 2, my = (pts[i - 1][1] + pts[i][1]) / 2;
                ctx.quadraticCurveTo(pts[i - 1][0], pts[i - 1][1], mx, my);
            }
            ctx.lineTo(pts[n - 1][0], pts[n - 1][1]);
        };
        if (graphStyle !== "line") {
            ctx.beginPath();
            trace();
            ctx.lineTo(pts[n - 1][0], height);
            ctx.lineTo(pts[0][0], height);
            ctx.closePath();
            ctx.globalAlpha = 0.18;
            ctx.fillStyle = color;
            ctx.fill();
            ctx.globalAlpha = 1;
        }
        ctx.beginPath();
        trace();
        ctx.lineWidth = lineWidth;
        ctx.lineJoin = "round";
        ctx.lineCap = "round";
        ctx.strokeStyle = color;
        ctx.stroke();
    }
}
