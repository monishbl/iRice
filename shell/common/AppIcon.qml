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

    property int size: 48
    property bool styled: true // false for non-app icons (files, folders)
    readonly property var cfg: Config.options.appearance
    readonly property string style: {
        const s = cfg.iconStyle;
        return s === "auto" ? (Theme.dark ? "dark" : "default") : s;
    }
    readonly property int mode: styled ? (({ dark: 1, clear: 2, tinted: 3 })[style] ?? 0) : 0
    readonly property bool tiled: styled && cfg.iconShape === "squircle"
    readonly property var info: tiled ? IconInfo.get(source.toString()) : null

    function tileFor(dominant) {
        const t = cfg.iconTile;
        if (t === "neutral")
            return Theme.dark ? "#2C2C31" : "#FFFFFF";
        const c = Qt.color(t === "accent" ? Theme.accent : dominant);
        return Theme.dark ? Qt.rgba(c.r * 0.32 + 0.07, c.g * 0.32 + 0.07, c.b * 0.32 + 0.08, 1) : Qt.rgba(c.r * 0.16 + 0.84, c.g * 0.16 + 0.84, c.b * 0.16 + 0.85, 1);
    }

    implicitSize: size
    asynchronous: true
    layer.enabled: mode !== 0 || (tiled && info !== null)
    layer.smooth: true
    layer.effect: ShaderEffect {
        property real mode: root.mode
        property real tile: root.tiled && root.info ? 1 : 0
        property real square: root.info?.square ? 1 : 0
        property real logoScale: 0.7
        property real darkTheme: Theme.dark ? 1 : 0
        property color tint: Theme.accent
        property color tileColor: root.tileFor(root.info?.dominant ?? "#808080")
        property color bgColor: root.info?.bg ?? "#808080"
        fragmentShader: Qt.resolvedUrl("icon.frag.qsb")
    }
}
