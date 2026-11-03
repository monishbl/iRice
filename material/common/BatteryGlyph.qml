import QtQuick
import "material-symbols.js" as Symbols

// The battery in the Material look, as Android 16/17 draws it: a rounded
// horizontal block, the charge filling it from the left over a faint
// empty part, the percentage inside in bold (written, not cut out), a
// small terminal, and a bolt beside it while charging (a plug when on power
// but not charging). Green while charging, yellow in battery saver, red at
// 20% and below. One Canvas: repaints only when something changes.
//   BatteryGlyph { level: 0.64; charging: true; color: bar.fg }
Canvas {
    id: root

    property real level: 1 // 0 .. 1
    property bool charging: false
    property bool plugged: false // on power, not charging (full, or holding)
    property bool lowPower: false
    property bool showPercent: false // the number inside
    property color color: "white"
    property real size: 12 // the block's height; everything else follows

    readonly property real blockW: Math.round(size * (showPercent ? 2.3 : 1.9))
    readonly property real nubW: Math.max(2, Math.round(size * 0.16))
    readonly property color green: Theme.dark ? "#6DD58C" : "#1E8E3E"
    readonly property color fillColor: lowPower ? Theme.accents.yellow : charging || plugged ? green : level <= 0.2 ? Theme.color.danger : color
    // The number: dark on a light fill, light on a dark one
    readonly property color textColor: (0.2126 * fillColor.r + 0.7152 * fillColor.g + 0.0722 * fillColor.b) > 0.55 ? "#1A1C1E" : "#FFFFFF"
    readonly property string mark: charging ? "lightning" : plugged ? "plug" : ""
    readonly property real markSize: Math.round(size * 1.15)

    implicitWidth: blockW + 1 + nubW + (mark !== "" ? 2 + markSize : 0)
    implicitHeight: Math.max(Math.round(size), mark !== "" ? markSize : 0)

    onLevelChanged: requestPaint()
    onChargingChanged: requestPaint()
    onPluggedChanged: requestPaint()
    onLowPowerChanged: requestPaint()
    onShowPercentChanged: requestPaint()
    onColorChanged: requestPaint()
    onFillColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    // The fonts load a moment after the shell starts: paint again then
    readonly property string family: Theme.font.family
    onFamilyChanged: {
        requestPaint();
        fontSettle.restart();
    }
    Timer {
        id: fontSettle
        interval: 600
        onTriggered: root.requestPaint()
    }
    Component.onCompleted: fontSettle.restart()

    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        const h = Math.round(size), y0 = Math.round((height - h) / 2), w = blockW, r = Math.round(h * 0.36);
        const block = () => {
            ctx.beginPath();
            ctx.roundedRect(0, y0, w, h, r, r);
        };
        // The empty part, then the charge, inside the block
        ctx.save();
        block();
        ctx.clip();
        ctx.fillStyle = Qt.alpha(color, 0.3);
        ctx.fillRect(0, y0, w, h);
        ctx.fillStyle = fillColor;
        ctx.fillRect(0, y0, Math.round(w * Math.max(0, Math.min(1, level))), h);
        ctx.restore();
        // The terminal
        const nh = Math.round(h * 0.42);
        ctx.fillStyle = Qt.alpha(color, 0.3);
        ctx.beginPath();
        ctx.roundedRect(w + 1, y0 + Math.round((h - nh) / 2), nubW, nh, nubW / 2, nubW / 2);
        ctx.fill();

        const cy = height / 2;
        ctx.textBaseline = "middle";
        if (showPercent) {
            // (as large as fits)
            const text = String(Math.round(level * 100));
            let px = Math.round(h * 0.78);
            ctx.font = "bold " + px + "px '" + family + "'";
            const room = w - 4;
            const tw = ctx.measureText(text).width;
            if (tw > room) {
                px = Math.max(6, Math.floor(px * room / tw));
                ctx.font = "bold " + px + "px '" + family + "'";
            }
            ctx.textAlign = "center";
            ctx.fillStyle = textColor;
            ctx.fillText(text, w / 2, cy + 0.5);
        }
        if (mark !== "") {
            ctx.textAlign = "center";
            ctx.font = markSize + "px 'iRice Symbols'";
            ctx.fillStyle = charging || plugged ? fillColor : color;
            ctx.fillText(Symbols.glyphs[mark] ?? "", w + 1 + nubW + 2 + markSize / 2, cy);
        }
    }
}
