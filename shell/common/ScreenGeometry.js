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
