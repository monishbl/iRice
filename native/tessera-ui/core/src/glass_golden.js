// The glass the QML shell sends the plugin, for the Rust one to match
// (theme.rs's glass_shapes_match_the_shell runs this and compares): Theme.qml's
// own `glass` object (its properties and preset(), taken from the file as
// they are) and Glass.qml's sum for one shape (pluginParams, pluginShape),
// run under node with Config, CoolDown and Wallpaper stubbed, for a grid of
// settings: every material, Custom's amounts, the optics, depth, the
// wallpaper's tint, both modes, Reduce transparency, Cool-down, Material.
//   node glass_golden.js      prints the cases as JSON
"use strict";
const fs = require("fs");
const path = require("path");
const root = path.resolve(__dirname, "../../../..");
const theme = fs.readFileSync(path.join(root, "shell/common/Theme.qml"), "utf8");

// Theme.qml's glass object: its body, to the closing brace
function body(src, start) {
    const i = src.indexOf(start);
    if (i < 0)
        throw new Error("not in Theme.qml: " + start);
    let depth = 0, j = src.indexOf("{", i + start.length - 1);
    for (; j < src.length; j++) {
        if (src[j] === "{")
            depth++;
        else if (src[j] === "}" && --depth === 0)
            break;
    }
    return src.slice(src.indexOf("{", i + start.length - 1) + 1, j);
}
// A line's code without its // comment (none of these strings hold "//")
function code(line) {
    let q = null;
    for (let i = 0; i < line.length; i++) {
        const c = line[i];
        if (q) {
            if (c === q)
                q = null;
        } else if (c === '"' || c === "'")
            q = c;
        else if (c === "/" && line[i + 1] === "/")
            return line.slice(0, i);
    }
    return line;
}
function balance(s) {
    let n = 0;
    for (const c of s)
        n += "([{".includes(c) ? 1 : ")]}".includes(c) ? -1 : 0;
    return n;
}

const glassSrc = body(theme, "readonly property QtObject glass: QtObject {");
const props = []; // [name, type, expression]
const funcs = []; // function sources
{
    const lines = glassSrc.split("\n");
    for (let i = 0; i < lines.length; i++) {
        const t = code(lines[i]).trim();
        let m;
        if ((m = /^(?:readonly\s+)?property\s+(\w+)\s+(\w+)\s*:\s*(.*)$/.exec(t))) {
            let e = m[3];
            while (balance(e) > 0 && i + 1 < lines.length)
                e += "\n" + code(lines[++i]);
            props.push([m[2], m[1], e]);
        } else if (/^function\s+\w+\s*\(/.test(t)) {
            let f = t;
            while (balance(f) > 0 && i + 1 < lines.length)
                f += "\n" + code(lines[++i]);
            funcs.push(f);
        }
    }
}

// Qt's colours: four channels 0..1, kept at 16 bits (QColor)
const q16 = x => Math.round(Math.max(0, Math.min(1, x)) * 65535) / 65535;
const rgba = (r, g, b, a) => ({ r: q16(r), g: q16(g), b: q16(b), a: q16(a) });
function color(c) {
    if (typeof c !== "string")
        return c;
    const v = parseInt(c.slice(1, 7), 16);
    return rgba((v >> 16 & 255) / 255, (v >> 8 & 255) / 255, (v & 255) / 255, 1);
}
const Qt = {
    rgba,
    alpha: (c, a) => {
        c = color(c);
        return rgba(c.r, c.g, c.b, a);
    },
    // (qquickglobal: the tint over the base by its alpha; opaque or clear tints as they are)
    tint: (base, tint) => {
        base = color(base);
        tint = color(tint);
        const a8 = Math.round(tint.a * 255);
        if (a8 === 255)
            return tint;
        if (a8 === 0)
            return base;
        const a = tint.a, inv = 1 - a;
        return rgba(tint.r * a + base.r * inv, tint.g * a + base.g * inv, tint.b * a + base.b * inv, a + inv * base.a);
    }
};

// Config.qml's defaults for what the glass reads
const DEFAULTS = { glass: "frosted", glassLens: 1, glassFrost: 0.75, glassSolid: 0, glassTint: 0.75, glassIor: 1.5, glassRefraction: 1, glassDispersion: 0.35, glassDepth: "auto", wallpaperTint: true, reduceTransparency: false, materialTransparency: 0 };

const MCU = (() => {
    const src = fs.readFileSync(path.join(root, "shell/common/MaterialColor.js"), "utf8").replace(/^\.pragma.*$/m, "");
    return new Function(src + ";return MCU;")();
})();

// Theme's glass object for one setting
function glassFor(c) {
    const app = Object.assign({}, DEFAULTS, c.appearance);
    const material = c.look === "material";
    const transparency = material && !app.reduceTransparency && c.cooldown !== "solid" ? Math.max(0, Math.min(1, app.materialTransparency ?? 0)) : 0;
    const controlAlpha = 1 - 0.4 * transparency;
    let m3 = {};
    if (material) {
        const s = MCU.scheme(c.seed, c.dark, "tonalSpot", 0);
        m3 = { surfaceContainer: transparency > 0 ? Qt.alpha(s.surfaceContainer, controlAlpha) : s.surfaceContainer };
    }
    const env = {
        root: { reduceTransparency: app.reduceTransparency, material, blur: transparency > 0, panelAlpha: 1 - 0.55 * transparency, dark: c.dark, m3 },
        Config: { options: { appearance: app } },
        CoolDown: { glass: c.cooldown },
        Wallpaper: { tone: color(c.tone) },
        Qt
    };
    const g = {};
    g.glassTheme = g;
    const names = Object.keys(env);
    for (const [name, type, expr] of props) {
        const f = new Function("g", ...names, `with (g) { return (${expr}); }`);
        Object.defineProperty(g, name, { get: () => {
                const v = f(g, ...names.map(n => env[n]));
                return type === "color" ? color(v) : v;
            } });
    }
    for (const src of funcs) {
        const name = /^function\s+(\w+)/.exec(src)[1];
        g[name] = new Function("g", ...names, `with (g) { return (${src}); }`)(g, ...names.map(n => env[n]));
    }
    return g;
}

// Glass.qml's shape for the plugin: [x y w h radius mode refraction depth
// bevel frost chroma rim tintRGBA saturation opacity gloss] (pluginParams,
// then pluginShape); the solid tier draws without the plugin there
function shape(c) {
    const g = glassFor(c);
    const pr = g.preset(Math.min(c.w, c.h) <= 140 ? 140 : 141), t = color(pr.tint);
    const p = [c.radius, pr.mode, pr.refraction * c.lens, pr.depth * c.lens, Math.min(pr.bevel, c.radius), pr.frost, pr.chroma * c.lens, pr.rim, t.r, t.g, t.b, t.a, pr.saturation, pr.gloss];
    return { tier: g.tier, material: g.material, shape: [c.x, c.y, c.w, c.h, p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8], p[9], p[10], p[11], p[12], c.opacity, p[13]] };
}

// The grid: a fixed pseudo-random walk over the settings, plus the cases
// that matter most (the defaults, the comparison's custom settings)
let seed = 7;
const rnd = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
const pick = a => a[Math.floor(rnd() * a.length)];
const cases = [];
const sizes = [[640, 40, 0], [280, 120, 26], [140, 140, 22], [141, 400, 28], [1920, 32, 0], [36, 36, 10], [560, 512, 28]];
function add(c) {
    for (const [w, h, r] of sizes)
        for (const lens of [1, 0.35])
            cases.push(Object.assign({ x: 12, y: 30, w, h, radius: r, opacity: 0.8, lens }, c));
}
add({ look: "glass", dark: true, appearance: {}, cooldown: "keep", tone: "#3366cc" });
add({ look: "glass", dark: true, appearance: { glass: "custom", glassLens: 1, glassFrost: 0.25, glassSolid: 0, glassTint: 1, glassIor: 1.33 }, cooldown: "keep", tone: "#8a5a3c" });
for (let i = 0; i < 160; i++) {
    const appearance = {
        glass: pick(["liquid", "frosted", "solid", "custom", "auto"]),
        glassLens: pick([0, 0.3, 0.6, 1]),
        glassFrost: pick([0, 0.25, 0.5, 0.75, 1]),
        glassSolid: pick([0, 0, 0.2, 0.5, 0.96, 0.98, 1]),
        glassTint: pick([0, 0.45, 0.75, 1]),
        glassIor: pick([1, 1.2, 1.33, 1.5, 1.8, 2, 2.4]),
        glassRefraction: pick([0.2, 0.4, 1, 1.5, 2, 3]),
        glassDispersion: pick([0, 0.35, 0.7, 1, 1.5]),
        glassDepth: pick(["auto", "thin", "depth"]),
        wallpaperTint: pick([true, false]),
        reduceTransparency: rnd() < 0.12,
        materialTransparency: pick([0, 0.3, 0.6, 1])
    };
    const look = rnd() < 0.2 ? "material" : "glass";
    const c = { look, dark: rnd() < 0.6, appearance, cooldown: pick(["keep", "keep", "frosted", "solid"]), tone: pick(["#3366cc", "#d97706", "#16a34a", "#808080"]), seed: pick(["#3b6ea5", "#d97706"]) };
    const [w, h, r] = pick(sizes);
    cases.push(Object.assign({ x: 100 * rnd(), y: 50, w, h, radius: r, opacity: pick([0, 0.5, 1]), lens: pick([1, 0.35]) }, c));
}
console.log(JSON.stringify(cases.map(c => Object.assign({}, c, shape(c)))));
