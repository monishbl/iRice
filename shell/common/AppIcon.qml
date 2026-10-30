import QtQuick
import Quickshell.Widgets

// An app icon in the chosen shape and Tahoe style (Settings → Appearance).
// Shape "squircle" puts every icon on the same continuous-corner tile: icons
// with their own background are clipped to it; transparent logos sit on a
// tile coloured from the logo (auto), neutral grey, or the accent. Styles:
// default, dark, clear, tinted, or auto (dark in dark mode). Rendered through
// one small shader layer; plain icons in the original shape cost nothing.
IconImage {
    id: root

    property real size: 48
    // The icon is drawn (and styled) once at this size, and `size` only
    // scales that finished tile: a dock icon growing under the pointer
    // doesn't reload its image or redraw its layer every frame
    property int renderSize: Math.ceil(size)
    property bool styled: true // false for non-app icons (files, folders)
    readonly property var cfg: Config.options.appearance
    readonly property string style: {
        const s = cfg.iconStyle;
        return s === "auto" ? (Theme.dark ? "dark" : "default") : s;
    }
    readonly property int mode: styled ? (({ dark: 1, clear: 2, tinted: 3 })[style] ?? 0) : 0
    readonly property bool tiled: styled && cfg.iconShape === "squircle"
    // (measured for tiles, and for clear/tinted logos: their brightness and detail)
    readonly property var info: tiled || mode >= 2 ? IconInfo.get(source.toString()) : null

    // Where a logo goes on its tile: what it really draws (not its canvas),
    // centred, at the same optical size for every app: by its area (a wide
    // logo like Webex's isn't shrunk to fit its width, a round one isn't
    // blown up), its longer side never past `cap`
    readonly property real logoSize: 0.46 // √(width × height), as a share of the tile
    readonly property real logoCap: 0.62
    // (solid shapes, a folder or a bin, weigh more than open ones at the same
    // size: `fill`, how much of its box a logo covers, shrinks those a little)
    function place(b, glass, fill) {
        const w = Math.max(b[2] - b[0], 0.05), h = Math.max(b[3] - b[1], 0.05);
        const weight = Math.min(1, Math.sqrt(0.85 / (0.5 + 0.5 * Math.max(0.2, Math.min(1, fill ?? 0.7)))));
        return { scale: Math.min(logoSize * weight / Math.sqrt(w * h), logoCap / Math.max(w, h), 1.6), x: (b[0] + b[2]) / 2, y: (b[1] + b[3]) / 2, glass: glass };
    }
    readonly property var fit: {
        const i = info;
        // Clear and tinted put every logo on the same glass, its own tile or
        // plate dropped: the logo itself (or, unmeasured, the whole icon) is
        // placed
        if (i && mode >= 2 && (i.square || i.plate))
            return place(i.gbox ?? i.box ?? [0, 0, 1, 1], true);
        if (!i || i.square || !i.box)
            return { scale: 0.7, x: 0.5, y: 0.5 };
        // A logo on its own plate (a disc or rounded square with a margin):
        // the plate fills the tile, so its edge doesn't show as a box inside it
        if (i.plate) {
            const b = i.box;
            return { scale: 1.06 / Math.max(b[2] - b[0], b[3] - b[1], 0.1), x: (b[0] + b[2]) / 2, y: (b[1] + b[3]) / 2 };
        }
        return place(i.box, false, i.fill);
    }

    function tileFor(dominant) {
        const t = cfg.iconTile;
        if (t === "neutral")
            return Theme.dark ? "#2C2C31" : "#FFFFFF";
        const c = Qt.color(t === "accent" ? Theme.accent : dominant);
        return Theme.dark ? Qt.rgba(c.r * 0.32 + 0.07, c.g * 0.32 + 0.07, c.b * 0.32 + 0.08, 1) : Qt.rgba(c.r * 0.16 + 0.84, c.g * 0.16 + 0.84, c.b * 0.16 + 0.85, 1);
    }

    implicitSize: renderSize
    transformOrigin: Item.TopLeft
    scale: renderSize === Math.ceil(size) ? 1 : size / Math.max(1, renderSize)
    asynchronous: true
    layer.enabled: mode !== 0 || (tiled && info !== null)
    layer.smooth: true
    layer.mipmap: renderSize > size + 0.5 // crisp when shown smaller than drawn
    layer.effect: ShaderEffect {
        property real mode: root.mode
        property real tile: root.tiled && root.info ? 1 : 0
        property real square: root.info?.square ? 1 : 0
        property real logoScale: root.fit.scale
        property real logoX: root.fit.x
        property real logoY: root.fit.y
        property real glassFit: root.fit.glass ? 1 : 0
        property real darkTheme: Theme.dark ? 1 : 0
        property real contrast: Math.max(0, Math.min(1, root.cfg.iconContrast))
        property real plate: root.info?.plate ? 1 : 0
        property real logoLo: root.info?.lo ?? 0
        property real logoHi: root.info?.hi ?? 1
        property real logoMed: root.info?.med ?? 0.5
        property real logoPol: root.info?.pol ?? 1
        property real bgFloor: root.info?.floor ?? 0.15
        property real bgEdge: root.info?.edge ?? 0.3
        property color tint: Theme.accent
        property color tileColor: root.tileFor(root.info?.dominant ?? "#808080")
        property color bgColor: root.info?.bgTop ?? root.info?.bg ?? "#808080"
        property color bgColor2: root.info?.bgBottom ?? root.info?.bg ?? "#808080"
        fragmentShader: Qt.resolvedUrl("icon.frag.qsb")
    }
}
