import QtQuick

// One rounded screen corner (black, concave). Flip with atRight / atBottom.
Canvas {
    id: root

    property bool atRight: false
    property bool atBottom: false
    property int radius: 12

    width: radius
    height: radius
    transform: Scale {
        origin.x: root.radius / 2
        origin.y: root.radius / 2
        xScale: root.atRight ? -1 : 1
        yScale: root.atBottom ? -1 : 1
    }
    onPaint: {
        const ctx = getContext("2d");
        const r = width;
        ctx.reset();
        ctx.fillStyle = "black";
        ctx.beginPath();
        ctx.moveTo(0, 0);
        ctx.lineTo(r, 0);
        ctx.arc(r, r, r, -Math.PI / 2, Math.PI, true);
        ctx.closePath();
        ctx.fill();
    }
    onWidthChanged: requestPaint()
}
