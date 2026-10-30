import QtQuick

// Measures queued app icons for IconInfo, one at a time, on a 32×32 canvas.
// Must sit in a visible window (behind other content) so it can paint.
//
// Besides whether an icon is already a tile and its colours, it finds what
// the clear and tinted styles need to draw any logo alike: the background
// the logo sits on (a square tile, or a disc or rounded plate with a margin,
// as a top-to-bottom gradient), how much that background itself varies, and
// the logo's own tones. (An offline copy of this maths, for previews, lives
// with the icon contact sheets; keep them alike.)
Canvas {
    id: root

    width: 32
    height: 32
    renderTarget: Canvas.Image
    property string current: ""

    readonly property string want: IconInfo.queue.length > 0 ? IconInfo.queue[0] : ""
    onWantChanged: start()
    function start() {
        if (current !== "" || want === "")
            return;
        current = want;
        if (isImageLoaded(current))
            requestPaint();
        else
            loadImage(current);
    }
    onImageLoaded: requestPaint()
    // Icons that never load still have to leave the queue
    Timer {
        id: timeout
        interval: 2000
        running: root.current !== ""
        onTriggered: root.finish({ square: false, plate: false, bg: "#808080", bgTop: "#808080", bgBottom: "#808080", floor: 0, edge: 0.3, dominant: "#808080", lo: 0, hi: 1, med: 0.5, pol: 1 })
    }

    function finish(result) {
        const src = current;
        current = "";
        if (src)
            unloadImage(src); // analysed: don't keep the full-size icon in memory
        IconInfo.done(src, result);
        Qt.callLater(start);
    }

    // The value at fraction p of a list, sorted
    function pct(list, p) {
        if (list.length === 0)
            return 0;
        const v = list.slice().sort((a, b) => a - b);
        return v[Math.min(v.length - 1, Math.max(0, Math.round(p * (v.length - 1))))];
    }

    onPaint: {
        if (current === "" || !isImageLoaded(current))
            return;
        const ctx = getContext("2d");
        ctx.clearRect(0, 0, 32, 32);
        ctx.drawImage(current, 0, 0, 32, 32);
        const d = ctx.getImageData(0, 0, 32, 32).data;
        const px = (x, y) => {
            const i = (y * 32 + x) * 4;
            return [d[i], d[i + 1], d[i + 2], d[i + 3]];
        };
        const solid = (x, y) => d[(y * 32 + x) * 4 + 3] >= 200;
        const luma = p => (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]) / 255;
        const dist = (p, q) => Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]) / 255;

        // Already a tile if points near the corners and edge midpoints are opaque
        // (and nearly all of it opaque: a big logo can reach the probes too,
        // Webex's, say, and must not be blown up to the tile's edges)
        const probes = [[4, 4], [27, 4], [4, 27], [27, 27], [16, 1], [16, 30], [1, 16], [30, 16]];
        let filled = 0;
        for (let i = 3; i < 32 * 32 * 4; i += 4)
            if (d[i] >= 200)
                filled++;
        const square = probes.filter(([x, y]) => px(x, y)[3] > 200).length >= 7 && filled >= 0.86 * 1024;

        // The tile or plate behind the logo, from its rim: the first opaque
        // pixels in from each side (2px in, past the antialiased edge) on
        // every row and column
        let opaque = 0, x0 = 32, x1 = -1, y0 = 32, y1 = -1;
        for (let y = 0; y < 32; y++)
            for (let x = 0; x < 32; x++)
                if (solid(x, y)) {
                    opaque++;
                    x0 = Math.min(x0, x);
                    x1 = Math.max(x1, x);
                    y0 = Math.min(y0, y);
                    y1 = Math.max(y1, y);
                }
        const rim = []; // [pixel, y]
        for (let i = 0; i < 32; i++) {
            let r0 = -1, r1 = -1, c0 = -1, c1 = -1, rn = 0, cn = 0;
            for (let j = 0; j < 32; j++) {
                if (solid(j, i)) {
                    if (r0 < 0)
                        r0 = j;
                    r1 = j;
                    rn++;
                }
                if (solid(i, j)) {
                    if (c0 < 0)
                        c0 = j;
                    c1 = j;
                    cn++;
                }
            }
            if (rn >= 8)
                rim.push([px(r0 + 2, i), i], [px(r1 - 2, i), i]);
            if (cn >= 8)
                rim.push([px(i, c0 + 2), c0 + 2], [px(i, c1 - 2), c1 - 2]);
        }
        const rimOk = rim.filter(e => e[0][3] >= 200);
        // A plate is a simple shape: a disc or a rounded square, nearly
        // square, filling most of its box (a logo's own outline doesn't)
        const bw = x1 - x0 + 1, bh = y1 - y0 + 1;
        const simple = opaque > 0 && bw / bh >= 0.8 && bw / bh <= 1.25 && opaque / (bw * bh) >= 0.74;
        let plate = false, floor = 0;
        let bgT = [128, 128, 128], bgB = [128, 128, 128];
        if (rimOk.length > 0 && opaque >= 0.35 * 1024 && (square || simple)) {
            // its colour at the top and the bottom (icons' gradients run top
            // to bottom): medians of the rim's upper and lower thirds
            const ys = rimOk.map(e => e[1]);
            const ya = pct(ys, 0.15), yb = pct(ys, 0.85), t33 = pct(ys, 0.33), t67 = pct(ys, 0.67);
            const top = rimOk.filter(e => e[1] <= t33).map(e => e[0]);
            const bot = rimOk.filter(e => e[1] >= t67).map(e => e[0]);
            const med3 = list => [0, 1, 2].map(c => pct(list.map(p => p[c]), 0.5));
            const mt = med3(top.length ? top : rimOk.map(e => e[0]));
            const mb = med3(bot.length ? bot : rimOk.map(e => e[0]));
            const at = y => {
                const t = yb === ya ? 0 : Math.max(0, Math.min(1, (y - ya) / (yb - ya)));
                return [0, 1, 2].map(c => mt[c] + (mb[c] - mt[c]) * t);
            };
            const dev = rimOk.map(e => dist(e[0], at(e[1])));
            // most of the rim close to that: a background, not a logo's edge
            if (pct(dev, 0.6) < 0.16) {
                plate = !square;
                bgT = at(0);
                bgB = at(31);
                floor = Math.min(0.3, pct(dev, 0.85));
            }
        }
        // A tile whose rim varies too much (busy artwork to the edges): the
        // average of its corners, with a wider margin for what counts as logo
        if (square && bgT[0] === 128 && bgT[1] === 128 && bgT[2] === 128 && floor === 0) {
            const cs = probes.slice(0, 4).map(([x, y]) => px(x, y));
            bgT = bgB = [0, 1, 2].map(c => cs.reduce((a, p) => a + p[c], 0) / 4);
            floor = 0.15;
        }
        const bgAt = y => [0, 1, 2].map(c => bgT[c] + (bgB[c] - bgT[c]) * y / 31);
        const onTile = square || plate;

        // The logo's pixels (opaque, and on a tile, unlike the tile), its
        // tones and how far it stands from the tile
        const tones = [], gaps = [];
        let gx0 = 32, gx1 = -1, gy0 = 32, gy1 = -1; // the logo's own box (on a tile: what differs from it)
        const logoAt = (x, y) => {
            gx0 = Math.min(gx0, x);
            gx1 = Math.max(gx1, x);
            gy0 = Math.min(gy0, y);
            gy1 = Math.max(gy1, y);
        };
        // Dominant colour: opaque pixels, weighted toward saturated ones
        let r = 0, g = 0, b = 0, w = 0;
        for (let y = 0; y < 32; y++)
            for (let x = 0; x < 32; x++) {
                const p = px(x, y);
                if (p[3] < 128)
                    continue;
                const mx = Math.max(p[0], p[1], p[2]), mn = Math.min(p[0], p[1], p[2]);
                const wt = 0.15 + (mx - mn) / 255;
                r += p[0] * wt;
                g += p[1] * wt;
                b += p[2] * wt;
                w += wt;
                if (onTile) {
                    const dd = dist(p, bgAt(y));
                    if (dd > floor + 0.08 && p[3] >= 230) {
                        tones.push(luma(p));
                        gaps.push(dd);
                        logoAt(x, y);
                    }
                } else {
                    tones.push(luma(p));
                    logoAt(x, y);
                }
            }
        const many = tones.length >= 12;
        const med = many ? pct(tones, 0.5) : 0.5;
        const hex = v => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
        const col = c => "#" + hex(c[0]) + hex(c[1]) + hex(c[2]);
        timeout.stop();
        finish({
            square: square,
            plate: plate,
            bg: col(bgAt(16)),
            bgTop: col(bgT),
            bgBottom: col(bgB),
            floor: floor,
            edge: gaps.length >= 12 ? Math.max(0.15, pct(gaps, 0.7)) : 0.3,
            dominant: w > 0 ? col([r / w, g / w, b / w]) : "#808080",
            lo: many ? pct(tones, 0.1) : 0,
            hi: many ? pct(tones, 0.9) : 1,
            med: med,
            // +1: lighter than the logo's own tone reads brighter (on a tile:
            // away from the tile's tone); -1 the other way
            pol: onTile ? (med >= luma(bgAt(16)) ? 1 : -1) : 1,
            // Where the opaque part sits (0..1: left, top, right, bottom) and
            // how much of that box it fills: logos are sized and centred on
            // their tile by what they really draw, not their canvas
            box: opaque > 0 ? [x0 / 32, y0 / 32, (x1 + 1) / 32, (y1 + 1) / 32] : [0, 0, 1, 1],
            fill: opaque > 0 ? opaque / (bw * bh) : 1,
            // the logo alone, inside its tile or plate (clear and tinted
            // replace that background with glass and size the logo itself)
            gbox: gx1 >= gx0 && tones.length >= 12 ? [gx0 / 32, gy0 / 32, (gx1 + 1) / 32, (gy1 + 1) / 32] : null
        });
    }
}
