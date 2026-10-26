pragma Singleton
import QtQuick
import Quickshell

// Design tokens. Everything visual reads from here; nothing hardcodes colours.
Singleton {
    id: root

    // "auto" follows the sun: light from sunrise to sunset
    readonly property bool dark: Config.options.appearance.mode === "auto" ? !Daylight.day : Config.options.appearance.mode !== "light"

    readonly property var accents: ({
            "blue": dark ? "#0A84FF" : "#007AFF",
            "purple": dark ? "#BF5AF2" : "#AF52DE",
            "pink": dark ? "#FF375F" : "#FF2D55",
            "red": dark ? "#FF453A" : "#FF3B30",
            "orange": dark ? "#FF9F0A" : "#FF9500",
            "yellow": dark ? "#FFD60A" : "#FFCC00",
            "green": dark ? "#30D158" : "#34C759",
            // (not offered as swatches; the wallpaper accent snaps to them too)
            "teal": dark ? "#40C8E0" : "#30B0C7",
            "cyan": dark ? "#64D2FF" : "#32ADE6",
            "indigo": dark ? "#5E5CE6" : "#5856D6",
            "graphite": dark ? "#98989D" : "#8E8E93"
        })
    readonly property color accent: Config.options.appearance.accent === "wallpaper" ? Wallpaper.accent : (accents[Config.options.appearance.accent] ?? accents.blue)

    readonly property QtObject font: QtObject {
        readonly property string family: "Inter"
        readonly property string mono: "JetBrains Mono"
        readonly property int caption: 11
        readonly property int small: 12
        readonly property int body: 13
        readonly property int title: 15
        readonly property int large: 20
        readonly property int display: 24
    }

    readonly property bool reduceTransparency: Config.options.appearance.reduceTransparency
    readonly property bool contrast: Config.options.appearance.increaseContrast

    readonly property QtObject color: QtObject {
        readonly property color label: root.dark ? Qt.rgba(1, 1, 1, root.contrast ? 1 : 0.92) : Qt.rgba(0, 0, 0, root.contrast ? 0.95 : 0.85)
        readonly property color secondary: root.dark ? Qt.rgba(1, 1, 1, root.contrast ? 0.78 : 0.58) : Qt.rgba(0, 0, 0, root.contrast ? 0.72 : 0.52)
        readonly property color tertiary: root.dark ? Qt.rgba(1, 1, 1, root.contrast ? 0.6 : 0.34) : Qt.rgba(0, 0, 0, root.contrast ? 0.56 : 0.3)
        readonly property color separator: root.dark ? Qt.rgba(1, 1, 1, root.contrast ? 0.3 : 0.12) : Qt.rgba(0, 0, 0, root.contrast ? 0.28 : 0.1)
        // Outline around glass surfaces with Increase contrast
        readonly property color outline: root.dark ? Qt.rgba(1, 1, 1, 0.42) : Qt.rgba(0, 0, 0, 0.38)
        readonly property color fill: root.dark ? Qt.rgba(1, 1, 1, 0.1) : Qt.rgba(0, 0, 0, 0.06)
        readonly property color fillHover: root.dark ? Qt.rgba(1, 1, 1, 0.16) : Qt.rgba(0, 0, 0, 0.1)
        readonly property color fillPressed: root.dark ? Qt.rgba(1, 1, 1, 0.22) : Qt.rgba(0, 0, 0, 0.15)
        readonly property color platter: root.dark ? Qt.rgba(1, 1, 1, 0.07) : Qt.rgba(1, 1, 1, 0.45)
        readonly property color toggleOff: root.dark ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0, 0, 0, 0.08)
        readonly property color onAccent: "white"
        readonly property color danger: root.dark ? "#FF453A" : "#FF3B30"
        readonly property color success: root.dark ? "#30D158" : "#34C759"
        readonly property color warning: root.dark ? "#FF9F0A" : "#FF9500"
    }

    // Glass material. Tint picks up a little of the wallpaper's colour, like
    // Tahoe's tinted appearance.
    readonly property QtObject glass: QtObject {
        id: glassTheme
        // A material is three amounts, 0..1: lens (bends what's behind), frost
        // (blurs it) and solid (opacity). Glass, Frost and Solid are tuned
        // presets; Custom uses your own (Settings → Appearance → Advanced).
        // [lens, frost, solid, tint]
        readonly property var presets: ({ liquid: [1, 0.1, 0.12, 0.45], frosted: [0.3, 1, 0.45, 0.5], solid: [0, 0, 1, 0] })
        readonly property string material: root.reduceTransparency ? "solid" : Config.options.appearance.glass === "auto" ? "liquid" : Config.options.appearance.glass
        readonly property var amounts: material === "custom" ? [Config.options.appearance.glassLens, Config.options.appearance.glassFrost, Config.options.appearance.glassSolid, Config.options.appearance.glassTint] : (presets[material] ?? presets.liquid)
        readonly property real lensAmount: amounts[0]
        readonly property real frostAmount: amounts[1]
        readonly property real solidAmount: amounts[2]
        readonly property real tintAmount: amounts[3] ?? 0.45
        // How much 3D lighting (specular, glint, sheen, inner shadow): full
        // for clear glass, fading to an even hairline as it frosts, none solid
        readonly property real gloss: lensAmount * (1 - 0.85 * frostAmount) * (1 - solidAmount)
        // The nearest named material, for code that needs a yes/no answer
        readonly property string tier: solidAmount >= 0.97 ? "solid" : (frostAmount < 0.4 && solidAmount < 0.4) ? "liquid" : "frosted"
        // Lens thickness: thin | depth | auto (thick on small controls)
        readonly property string depth: Config.options.appearance.glassDepth
        readonly property color base: root.dark ? Qt.rgba(0.08, 0.08, 0.1, 0.16) : Qt.rgba(0.98, 0.98, 1.0, 0.3)
        readonly property color tint: Config.options.appearance.wallpaperTint ? Qt.tint(base, Qt.alpha(Wallpaper.tone, root.dark ? 0.1 : 0.08)) : base
        readonly property color solid: root.dark ? "#26262A" : "#F2F2F5"
        // Frosted sits over compositor-blurred windows, so it needs more body
        readonly property color frost: root.dark ? Qt.rgba(0.11, 0.11, 0.13, 0.62) : Qt.rgba(0.97, 0.97, 0.98, 0.66)
        readonly property real saturation: 1.35
        readonly property real rim: root.dark ? 0.5 : 0.65
        readonly property real refraction: 36
        readonly property real chroma: 0.12
        readonly property real shadow: root.dark ? 0.32 : 0.14

        function smooth(a, b, x) {
            const t = Math.max(0, Math.min(1, (x - a) / (b - a)));
            return t * t * (3 - 2 * t);
        }
        function mixc(a, b, t) {
            return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t);
        }

        // The plugin's per-shape parameters for the current material (see irice-glass)
        function preset(minSide) {
            const l = lensAmount, f = frostAmount, s = solidAmount;
            const thick = depth === "depth" || (depth === "auto" && minSide <= 140);
            if (s >= 0.99)
                return { mode: 2, refraction: 0, depth: 0, bevel: 8, frost: 0, chroma: 0, rim: 0, gloss: 0, tint: solid, saturation: 1 };
            // Tint: how much it darkens (light mode: whitens) what's behind
            const k = 0.3 + 1.4 * tintAmount;
            const clearTint = Qt.alpha(tint, Math.min(0.9, (root.dark ? 0.16 : 0.18) * k));
            const frostTint = Qt.alpha(frost, Math.min(0.9, (root.dark ? 0.38 : 0.45) * (0.6 + 0.8 * tintAmount)));
            // Body: clear tint, toward the frosted tint as it frosts, toward
            // the solid colour as it gets opaque
            const body = mixc(clearTint, frostTint, f);
            const t = mixc(body, Qt.alpha(solid, 1), Math.max(0, (s - body.a) / Math.max(0.01, 1 - body.a)));
            return {
                mode: 0,
                refraction: (thick ? 30 : 22) * l,
                depth: (thick ? 0.55 : 0) * l,
                bevel: (thick ? 26 : 18) * (0.5 + 0.5 * l),
                frost: f,
                chroma: (thick ? 0.28 : 0.18) * l,
                rim: (0.12 + 0.16 * l) * (1 - s),
                gloss: gloss,
                tint: t,
                saturation: 1.35 + 0.15 * f - 0.5 * s
            };
        }
    }

    readonly property QtObject radius: QtObject {
        readonly property int item: 6
        readonly property int control: 10
        readonly property int menu: 12
        readonly property int popover: 18
        readonly property int module: 20
        readonly property int panel: 26
    }

    readonly property QtObject motion: QtObject {
        readonly property bool expressive: Config.options.appearance.motion === "expressive"
        // Springs (SpringAnimation): open/expand, settle back, and elastic pull
        readonly property real spring: expressive ? 3.0 : 3.8
        readonly property real damping: expressive ? 0.22 : 0.32
        readonly property real pull: expressive ? 14 : 6 // how far a panel gives when pressed and dragged
        readonly property real controlPull: expressive ? 5 : 2.5 // same for small controls
        readonly property real squish: expressive ? 0.9 : 0.95 // press scale
        readonly property int press: 100
        readonly property int hover: 150
        readonly property int popover: 220
        readonly property int panel: 320
        readonly property int large: 450
        // Apple-like decelerate; QML BezierSpline control points.
        readonly property var emphasized: [0.2, 0.9, 0.1, 1.0, 1, 1]
        readonly property var standard: [0.3, 0.0, 0.2, 1.0, 1, 1]
        readonly property var exit: [0.4, 0.0, 1.0, 1.0, 1, 1]
    }
}
