import QtQuick

// Measures queued app icons for IconInfo, one at a time, on a 32×32 canvas.
// Must sit in a visible window (behind other content) so it can paint.
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
        onTriggered: root.finish({ square: false, bg: "#808080", dominant: "#808080", lum: 0.5, spread: 0.25 })
    }

    function finish(result) {
        const src = current;
        current = "";
        if (src)
            unloadImage(src); // analysed: don't keep the full-size icon in memory
        IconInfo.done(src, result);
        Qt.callLater(start);
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
        // Already a tile if points near the corners and edge midpoints are opaque
        const probes = [[4, 4], [27, 4], [4, 27], [27, 27], [16, 1], [16, 30], [1, 16], [30, 16]];
        const opaque = probes.filter(([x, y]) => px(x, y)[3] > 200).length;
        const square = opaque >= 7;
        // Tile colour: average of the corner probes
        let br = 0, bgc = 0, bb = 0;
        for (const [x, y] of probes.slice(0, 4)) {
            const p = px(x, y);
            br += p[0];
            bgc += p[1];
            bb += p[2];
        }
        // Dominant colour: opaque pixels, weighted toward saturated ones
        let r = 0, g = 0, b = 0, w = 0;
        for (let i = 0; i < d.length; i += 4) {
            if (d[i + 3] < 128)
                continue;
            const mx = Math.max(d[i], d[i + 1], d[i + 2]), mn = Math.min(d[i], d[i + 1], d[i + 2]);
            const wt = 0.15 + (mx - mn) / 255;
            r += d[i] * wt;
            g += d[i + 1] * wt;
            b += d[i + 2] * wt;
            w += wt;
        }
        // The logo's brightness and detail (for clear and tinted styles, which
        // normalise them so every logo reads alike): over the logo's pixels,
        // the opaque ones that differ from the tile (square icons) or just
        // the opaque ones (transparent logos)
        const tr = br / 4, tg = bgc / 4, tb = bb / 4;
        let n = 0, sum = 0, sum2 = 0;
        for (let i = 0; i < d.length; i += 4) {
            if (d[i + 3] < 128)
                continue;
            if (square && Math.hypot(d[i] - tr, d[i + 1] - tg, d[i + 2] - tb) < 40)
                continue;
            const l = (0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2]) / 255;
            n++;
            sum += l;
            sum2 += l * l;
        }
        const mean = n >= 12 ? sum / n : 0.5;
        const spread = n >= 12 ? Math.sqrt(Math.max(0, sum2 / n - mean * mean)) : 0.25;
        const hex = (v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
        timeout.stop();
        finish({
            square: square,
            bg: "#" + hex(br / 4) + hex(bgc / 4) + hex(bb / 4),
            dominant: w > 0 ? "#" + hex(r / w) + hex(g / w) + hex(b / w) : "#808080",
            lum: mean,
            spread: spread
        });
    }
}
