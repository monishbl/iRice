import QtQuick
import qs.common

// How far a transfer has got, as a ring around who it's with (Glass's share
// sheet and incoming card): the track in the fill colour, the done part in
// the accent, clockwise from the top.
Canvas {
    id: ring

    property real value: 0 // 0 .. 1
    property real thickness: 3
    property real inset: 2 // from the edge to the middle of the stroke

    onValueChanged: requestPaint()
    onVisibleChanged: requestPaint()
    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        ctx.lineWidth = thickness;
        ctx.lineCap = "round";
        ctx.strokeStyle = Theme.color.fill;
        ctx.beginPath();
        ctx.arc(width / 2, height / 2, width / 2 - inset, 0, 2 * Math.PI);
        ctx.stroke();
        ctx.strokeStyle = Theme.accent;
        ctx.beginPath();
        ctx.arc(width / 2, height / 2, width / 2 - inset, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * value);
        ctx.stroke();
    }
}
