.pragma library
// The accent as a family of shades, and picking the one that reads on a
// background (Theme.accentFor). OKLCH keeps the hue and vividness as the
// lightness changes; dark yellows are warmed toward gold so they don't turn
// olive. 11 shades, lightness 0.97 (palest) .. 0.24 (deepest).

const steps = [0.97, 0.93, 0.87, 0.80, 0.72, 0.64, 0.56, 0.48, 0.40, 0.32, 0.24];

function s2l(c) { return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); }
function l2s(c) { c = Math.max(0, Math.min(1, c)); return c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055; }

function toOklch(r, g, b) {
    r = s2l(r); g = s2l(g); b = s2l(b);
    const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    const L = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    const A = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    const B = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    return [L, Math.hypot(A, B), Math.atan2(B, A)];
}
// Linear sRGB of an OKLCH colour (may be out of gamut)
function linear(L, C, h) {
    const A = C * Math.cos(h), B = C * Math.sin(h);
    const l = Math.pow(L + 0.3963377774 * A + 0.2158037573 * B, 3);
    const m = Math.pow(L - 0.1055613458 * A - 0.0638541728 * B, 3);
    const s = Math.pow(L - 0.0894841775 * A - 1.2914855480 * B, 3);
    return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s];
}
// The colour, with its chroma reduced until it fits sRGB
function inGamut(L, C, h) {
    let rgb = linear(L, C, h);
    for (let i = 0; i < 30 && (Math.min(...rgb) < -1e-4 || Math.max(...rgb) > 1 + 1e-4); i++) {
        C *= 0.92;
        rgb = linear(L, C, h);
    }
    return Qt.rgba(l2s(rgb[0]), l2s(rgb[1]), l2s(rgb[2]), 1);
}

function ramp(accent) {
    const [L, C, h] = toOklch(accent.r, accent.g, accent.b);
    const deg = ((h * 180 / Math.PI) % 360 + 360) % 360;
    const yellow = deg > 70 && deg < 120;
    return steps.map(t => {
        // vivid near the accent's own lightness, a little softer at the ends
        let c = C * (1 - Math.pow(Math.min(1, Math.abs(t - L) / 0.75), 1.6) * 0.55);
        let hh = h;
        if (yellow && t < L) {
            hh = (deg - Math.min(28, (L - t) * 70)) * Math.PI / 180;
            c *= 1.12;
        }
        return inGamut(t, c, hh);
    });
}

function luminance(c) { return 0.2126 * s2l(c.r) + 0.7152 * s2l(c.g) + 0.0722 * s2l(c.b); }
function contrast(a, b) {
    const la = luminance(a), lb = luminance(b);
    return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

// What to draw in the accent on `background` (contrast ≥ ratio): the accent
// itself when it reads; on a dark background, the nearest lighter shade of
// it that reads; otherwise neutral (near-black on light, near-white on dark).
// Never a darkened accent: a dark yellow or orange only looks muddy.
function pick(shades, accent, background, ratio) {
    accent = Qt.color(accent);
    background = Qt.color(background);
    if (contrast(accent, background) >= ratio)
        return accent;
    const darkBg = luminance(background) < 0.18;
    if (darkBg) {
        const L = toOklch(accent.r, accent.g, accent.b)[0];
        let best = null, bestD = 9;
        for (const c of shades) {
            const cl = toOklch(c.r, c.g, c.b)[0];
            if (cl < L || contrast(c, background) < ratio)
                continue;
            if (cl - L < bestD) {
                best = c;
                bestD = cl - L;
            }
        }
        if (best)
            return best;
    }
    return Qt.color(darkBg ? "#EDEDEF" : "#1C1C1E");
}

// The lighter of two colours (by OKLCH lightness)
function lighter(a, b) {
    a = Qt.color(a);
    b = Qt.color(b);
    return toOklch(a.r, a.g, a.b)[0] >= toOklch(b.r, b.g, b.b)[0] ? a : b;
}
