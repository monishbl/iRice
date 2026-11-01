import QtQuick
import qs.common

// The fingerprint in the add-a-finger popup (FingerprintSheet), drawn like
// the familiar fingerprint icon and cut into 11 sections. 0 is the centre; 1-4 the ring around it
// (above, right, below, left); 5-10 the edges (the tip, then clockwise:
// upper right, lower right, the bottom, lower left, upper left). Taken
// sections are in the accent, the next one lit softly, the rest faint; all
// of it green when done. Painted only when something changes.
Item {
    id: root

    property int taken: 0 // sections 0..taken-1 are recorded
    property int target: 0 // the one to touch next (-1: none)
    property bool done: false

    // animation state, read by the painter
    property real glow: 1 // the next section's highlight, swelling in
    property real fill: 1 // the section just taken, filling in (0..1)
    property real flash: 0 // a miss: the next section, red
    property int filling: -1

    function took() {
        filling = taken - 1;
        fillIn.restart();
        bump.restart();
    }
    function missed() {
        flashOut.restart();
        shake.restart();
    }
    function lightTarget() {
        glowIn.restart();
    }

    onTakenChanged: canvas.requestPaint()
    onTargetChanged: canvas.requestPaint()
    onDoneChanged: {
        canvas.requestPaint();
        if (done)
            doneIn.restart();
    }
    onGlowChanged: canvas.requestPaint()
    onFillChanged: canvas.requestPaint()
    onFlashChanged: canvas.requestPaint()
    Connections {
        target: Theme
        function onDarkChanged() { canvas.requestPaint(); }
    }

    NumberAnimation { id: glowIn; target: root; property: "glow"; from: 0.2; to: 1; duration: Math.round(420 * CoolDown.motionScale); easing.type: Easing.OutCubic }
    NumberAnimation { id: fillIn; target: root; property: "fill"; from: 0; to: 1; duration: Math.round(380 * CoolDown.motionScale); easing.type: Easing.OutCubic }
    SequentialAnimation {
        id: flashOut
        NumberAnimation { target: root; property: "flash"; to: 1; duration: 60 }
        PauseAnimation { duration: 450 }
        NumberAnimation { target: root; property: "flash"; to: 0; duration: Math.round(700 * CoolDown.motionScale); easing.type: Easing.InCubic }
    }
    SequentialAnimation {
        id: bump
        NumberAnimation { target: canvas; property: "scale"; to: 0.96; duration: Math.round(80 * CoolDown.motionScale); easing.type: Easing.OutCubic }
        NumberAnimation { target: canvas; property: "scale"; to: 1; duration: Math.round(260 * CoolDown.motionScale); easing.type: Easing.OutBack }
    }
    SequentialAnimation {
        id: shake
        NumberAnimation { target: canvas; property: "x"; to: -8; duration: 50 }
        NumberAnimation { target: canvas; property: "x"; to: 7; duration: 70 }
        NumberAnimation { target: canvas; property: "x"; to: -5; duration: 60 }
        NumberAnimation { target: canvas; property: "x"; to: 3; duration: 50 }
        NumberAnimation { target: canvas; property: "x"; to: 0; duration: 40 }
    }
    SequentialAnimation {
        id: doneIn
        NumberAnimation { target: badge; property: "scale"; from: 0.4; to: 1.1; duration: Math.round(260 * CoolDown.motionScale); easing.type: Easing.OutBack }
        NumberAnimation { target: badge; property: "scale"; to: 1; duration: Math.round(140 * CoolDown.motionScale) }
    }

    // The ridges, as short segments, each knowing its section (made once).
    // A loop print, drawn like the familiar fingerprint icon: hairpins
    // nested around a small core, tilted so their legs run down to the lower
    // left, inside wider arcs that wrap over the top and round the right;
    // the ridges are broken into dashes of different lengths, in thick round
    // strokes. Built in its own units, then fitted to the item.
    readonly property real stroke: 7
    readonly property var segments: {
        const lines = []; // polylines [[x, y], …]
        const gap = 13; // ridge to ridge
        const tilt = 24 * Math.PI / 180;
        const rot = (x, y) => [x * Math.cos(tilt) - y * Math.sin(tilt), x * Math.sin(tilt) + y * Math.cos(tilt)];
        // the hairpins: over the top, down both legs (the left one longer)
        for (let k = 0; k < 6; k++) {
            const r = 6 + k * gap;
            const pts = [];
            const legL = 24 + k * 17 + (k % 2) * 10, legR = 6 + k * 10;
            for (let y = legL; y > 0; y -= 3)
                pts.push(rot(-r, y));
            for (let d = 180; d <= 360; d += 4) {
                const t = d * Math.PI / 180;
                pts.push(rot(r * Math.cos(t), r * 1.3 * Math.sin(t)));
            }
            for (let y = 0; y <= legR; y += 3)
                pts.push(rot(r, y));
            lines.push(pts);
        }
        // the outer ridges: over the top and round the right, down the left
        for (let k = 6; k < 9; k++) {
            const r = 6 + k * gap;
            const pts = [];
            for (let d = 152 + (k - 6) * 8; d <= 424 - (k - 6) * 14; d += 3) {
                const t = d * Math.PI / 180;
                const sx = Math.sin(t) > 0 ? 1.15 : 1.3; // (rounder under, taller over)
                pts.push(rot(r * Math.cos(t), r * sx * Math.sin(t)));
            }
            lines.push(pts);
        }
        // fit into the item, leaving room for the stroke
        let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
        for (const l of lines)
            for (const [x, y] of l) {
                x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y);
            }
        const m = stroke;
        const sc = Math.min((width - 2 * m) / (x1 - x0), (height - 2 * m) / (y1 - y0));
        const ox = (width - (x1 - x0) * sc) / 2 - x0 * sc, oy = (height - (y1 - y0) * sc) / 2 - y0 * sc;
        const cx = ox + (x0 + x1) / 2 * sc, cy = oy + (y0 + y1) / 2 * sc;
        const rxMax = (x1 - x0) / 2 * sc, ryMax = (y1 - y0) / 2 * sc;
        const sectionOf = (x, y) => {
            const dx = (x - cx) / rxMax, dy = (y - cy) / ryMax;
            const r = Math.sqrt(dx * dx + dy * dy);
            const a = Math.atan2(dy, dx) * 180 / Math.PI; // 0 = right, -90 = up
            if (r < 0.27)
                return 0;
            if (r < 0.6) {
                if (a >= -135 && a < -45) return 1;
                if (a >= -45 && a < 45) return 2;
                if (a >= 45 && a < 135) return 3;
                return 4;
            }
            const b = (a + 120 + 360) % 360; // 0 at the top-left end of the tip's sector
            return 5 + Math.floor(b / 60);
        };
        // dashes: lengths cycling per ridge (the core two unbroken)
        const dashes = [62, 30, 48, 22, 70, 36, 54];
        const out = [];
        lines.forEach((l, k) => {
            let along = 0, di = k * 3, until = k < 2 ? 1e9 : dashes[di % dashes.length], on = true;
            for (let i = 1; i < l.length; i++) {
                const ax = ox + l[i - 1][0] * sc, ay = oy + l[i - 1][1] * sc;
                const bx = ox + l[i][0] * sc, by = oy + l[i][1] * sc;
                along += Math.hypot(bx - ax, by - ay);
                if (along > until) {
                    on = !on;
                    along = 0;
                    until = on ? dashes[++di % dashes.length] : 11;
                }
                if (on)
                    out.push({ x1: ax, y1: ay, x2: bx, y2: by, s: sectionOf((ax + bx) / 2, (ay + by) / 2) });
            }
        });
        return out;
    }

    Canvas {
        id: canvas
        width: root.width
        height: root.height
        renderStrategy: Canvas.Cooperative
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            ctx.lineCap = "round";
            ctx.lineWidth = root.stroke;
            const accent = Theme.accent, success = Theme.color.success, danger = Theme.color.danger;
            const faint = Theme.dark ? Qt.rgba(1, 1, 1, 0.16) : Qt.rgba(0, 0, 0, 0.13);
            const mix = (a, b, t) => Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t);
            const soft = Qt.alpha(accent, 0.42);
            const colourOf = s => {
                if (root.done)
                    return success;
                if (s < root.taken)
                    return s === root.filling && root.fill < 1 ? mix(soft, accent, root.fill) : accent;
                if (s === root.target) {
                    const lit = mix(faint, soft, root.glow);
                    return root.flash > 0 ? mix(lit, danger, root.flash) : lit;
                }
                return faint;
            };
            // one path per colour: few strokes, quick to paint
            const groups = {};
            for (const g of root.segments) {
                const c = colourOf(g.s);
                const key = String(c);
                (groups[key] = groups[key] ?? { c: c, list: [] }).list.push(g);
            }
            for (const key in groups) {
                const grp = groups[key];
                ctx.strokeStyle = grp.c;
                ctx.beginPath();
                for (const g of grp.list) {
                    ctx.moveTo(g.x1, g.y1);
                    ctx.lineTo(g.x2, g.y2);
                }
                ctx.stroke();
            }
        }
    }

    // done: a check at the bottom right of the print
    Rectangle {
        id: badge
        visible: root.done
        x: root.width - 52
        y: root.height - 58
        width: 44
        height: 44
        radius: 22
        color: Theme.color.success
        border.width: 3
        border.color: Theme.dark ? "#232327" : "white"
        Icon {
            anchors.centerIn: parent
            name: "check"
            bold: true
            size: 22
            color: "white"
        }
    }
}
