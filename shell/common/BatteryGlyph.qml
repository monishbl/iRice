import QtQuick
import "phosphor.js" as Phosphor
import "material-symbols.js" as Symbols

// The battery, drawn like iOS 27: a solid capsule with no outline. The charge
// fills it from the left in the icon's colour; the rest is the same colour,
// faint. Red at 20% and below, yellow in Low Power. Charging cuts a bolt out
// of it (a plug when it's on power but not charging), and the percentage can
// be cut out of it too. One Canvas: repaints only when something changes.
// Material: as Android 16/17 draws it, a rounded horizontal block, the
// percentage inside in bold (written, not cut out), a small terminal, and a
// bolt beside it while charging (a plug when on power but not charging);
// green while charging, yellow in battery saver, red at 20% and below.
//   BatteryGlyph { level: 0.64; charging: true; color: bar.fg }
Canvas {
    id: root

    property real level: 1 // 0 .. 1
    property bool charging: false
    property bool plugged: false // on power, not charging (full, or holding)
    property bool lowPower: false
    property bool showPercent: false // the number inside (Glass: cut out)
    property color color: "white"
    property real size: 12 // the capsule's (block's) height; everything else follows

    readonly property real capsuleW: Math.round(size * (showPercent ? 2.3 : Theme.material ? 1.9 : 2.05))
    readonly property real nubW: Math.max(2, Math.round(size * (Theme.material ? 0.16 : 0.17)))
    readonly property color green: Theme.dark ? "#6DD58C" : "#1E8E3E"
    readonly property color fillColor: lowPower ? Theme.accents.yellow : Theme.material && (charging || plugged) ? green : !charging && !plugged && level <= 0.2 ? Theme.color.danger : color
    readonly property string mark: charging ? "lightning" : plugged ? "plug" : ""
    readonly property real markSize: Math.round(size * (Theme.material ? 1.15 : 1.3)) // (Glass: a little taller than the capsule, like iOS)
    // With the number inside, the mark goes beside the battery instead
    // (Material: always beside)
    readonly property bool markBeside: mark !== "" && (showPercent || Theme.material)
    readonly property real besideSize: Theme.material ? markSize : Math.round(size * 0.95)

    implicitWidth: capsuleW + 1 + nubW + (markBeside ? 2 + besideSize : 0)
    implicitHeight: Theme.material ? Math.max(Math.round(size), mark !== "" ? markSize : 0) : mark !== "" && !markBeside ? markSize : Math.round(size)

    onLevelChanged: requestPaint()
    onChargingChanged: requestPaint()
    onPluggedChanged: requestPaint()
    onLowPowerChanged: requestPaint()
    onShowPercentChanged: requestPaint()
    onColorChanged: requestPaint()
    onFillColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    // (Material: its fonts load a moment after the shell starts; paint again then)
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
    Component.onCompleted: if (Theme.material) fontSettle.restart()

    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        const h = Math.round(size), y0 = Math.round((height - h) / 2), w = capsuleW, r = Math.round(h * (Theme.material ? 0.36 : 0.32));
        const faint = Qt.alpha(color, Theme.material ? 0.3 : 0.35);
        const capsule = () => {
            ctx.beginPath();
            ctx.roundedRect(0, y0, w, h, r, r);
        };
        // Empty part, then the charge, clipped to the capsule
        ctx.save();
        capsule();
        ctx.clip();
        ctx.fillStyle = faint;
        ctx.fillRect(0, y0, w, h);
        ctx.fillStyle = fillColor;
        ctx.fillRect(0, y0, Math.round(w * Math.max(0, Math.min(1, level))), h);
        ctx.restore();
        // The terminal nub
        const nh = Math.round(h * (Theme.material ? 0.42 : 0.38));
        ctx.fillStyle = faint;
        ctx.beginPath();
        ctx.roundedRect(w + 1, y0 + Math.round((h - nh) / 2), nubW, nh, nubW / 2, nubW / 2);
        ctx.fill();

        const cx = w / 2, cy = height / 2;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        if (Theme.material) {
            if (showPercent) {
                // (as large as fits; dark on a light fill, light on a dark one)
                const text = String(Math.round(level * 100));
                let px = Math.round(h * 0.78);
                ctx.font = "bold " + px + "px '" + family + "'";
                const room = w - 4;
                const tw = ctx.measureText(text).width;
                if (tw > room) {
                    px = Math.max(6, Math.floor(px * room / tw));
                    ctx.font = "bold " + px + "px '" + family + "'";
                }
                ctx.fillStyle = (0.2126 * fillColor.r + 0.7152 * fillColor.g + 0.0722 * fillColor.b) > 0.55 ? "#1A1C1E" : "#FFFFFF";
                ctx.fillText(text, cx, cy + 0.5);
            }
            if (mark !== "") {
                ctx.font = markSize + "px 'iRice Symbols'";
                ctx.fillStyle = charging || plugged ? fillColor : color;
                ctx.fillText(Symbols.glyphs[mark] ?? "", w + 1 + nubW + 2 + markSize / 2, cy);
            }
        } else if (showPercent) {
            ctx.font = "bold " + Math.round(h * 0.82) + "px '" + family + "'";
            ctx.globalCompositeOperation = "destination-out";
            ctx.fillStyle = "black";
            ctx.fillText(String(Math.round(level * 100)), cx, cy + 0.5);
            ctx.globalCompositeOperation = "source-over";
            if (markBeside) {
                ctx.font = besideSize + "px 'Phosphor-Fill'";
                ctx.fillStyle = color;
                ctx.fillText(Phosphor.glyphs[mark] ?? "", w + 1 + nubW + 2 + besideSize / 2, cy);
            }
        } else if (mark !== "") {
            // A gap round the mark (cut out), then the mark in the icon's colour
            ctx.font = markSize + "px 'Phosphor-Fill'";
            const g = Phosphor.glyphs[mark] ?? "";
            ctx.globalCompositeOperation = "destination-out";
            ctx.lineWidth = 2.5;
            ctx.lineJoin = "round";
            ctx.strokeStyle = "black";
            ctx.strokeText(g, cx, cy);
            ctx.fillText(g, cx, cy);
            ctx.globalCompositeOperation = "source-over";
            ctx.fillStyle = color;
            ctx.fillText(g, cx, cy);
        }
    }
}
