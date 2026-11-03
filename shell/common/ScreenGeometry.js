// Clip in logical screen coordinates, including partially off-screen windows.
function clip(rect, width, height) {
    if (![rect.x, rect.y, rect.width, rect.height, width, height].every(Number.isFinite) || width <= 0 || height <= 0)
        return { x: 0, y: 0, width: 0, height: 0 };
    const left = Math.max(0, Math.min(width, rect.x));
    const top = Math.max(0, Math.min(height, rect.y));
    const right = Math.max(left, Math.min(width, rect.x + Math.max(0, rect.width)));
    const bottom = Math.max(top, Math.min(height, rect.y + Math.max(0, rect.height)));
    return { x: left, y: top, width: right - left, height: bottom - top };
}

function integerClip(rect, width, height) {
    const r = clip(rect, width, height);
    const left = Math.floor(r.x), top = Math.floor(r.y);
    const right = Math.min(Math.floor(width), Math.ceil(r.x + r.width));
    const bottom = Math.min(Math.floor(height), Math.ceil(r.y + r.height));
    return {x: left, y: top, width: Math.max(0, right - left), height: Math.max(0, bottom - top)};
}

// Circle to search: what a drawn stroke selects. A drag that runs roughly
// straight from press to release is a box (its ends are opposite corners);
// anything else (a circle, a scribble, an outline) is a shape, and its
// bounding box counts. "none": too small to be either (a click).
function strokeKind(points) {
    if (!points || points.length < 2)
        return "none";
    const e = extent(points);
    if (Math.max(e.width, e.height) < 8)
        return "none";
    const a = points[0], b = points[points.length - 1];
    const chord = Math.hypot(b.x - a.x, b.y - a.y);
    if (chord < 24)
        return "circle"; // (it ends near where it began)
    // The path's length, and how far it strays from the line between its ends
    let length = 0, stray = 0;
    for (let i = 0; i < points.length; i++) {
        const p = points[i];
        if (i > 0)
            length += Math.hypot(p.x - points[i - 1].x, p.y - points[i - 1].y);
        stray = Math.max(stray, Math.abs((b.x - a.x) * (a.y - p.y) - (a.x - p.x) * (b.y - a.y)) / chord);
    }
    return length <= 1.2 * chord + 16 && stray <= Math.max(12, 0.12 * chord) ? "box" : "circle";
}

// The area a stroke selects, clipped to the screen: a box from end to end,
// or a shape's bounding box padded by `pad` (it's drawn round the thing,
// often across its edges)
function strokeBox(points, pad, width, height) {
    const kind = strokeKind(points);
    if (kind === "none")
        return { x: 0, y: 0, width: 0, height: 0 };
    const e = extent(kind === "box" ? [points[0], points[points.length - 1]] : points);
    const p = kind === "box" ? 0 : pad;
    return clip({ x: e.x - p, y: e.y - p, width: e.width + 2 * p, height: e.height + 2 * p }, width, height);
}

function extent(points) {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const p of points) {
        x0 = Math.min(x0, p.x); y0 = Math.min(y0, p.y);
        x1 = Math.max(x1, p.x); y1 = Math.max(y1, p.y);
    }
    return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}
