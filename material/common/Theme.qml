pragma Singleton
import QtQuick
import Quickshell
import "AccentRamp.js" as AccentRamp
import "MaterialColor.js" as MaterialColor

// Design tokens. Everything visual reads from here; nothing hardcodes colours.
// Two looks share them (Settings → Appearance → Look): Glass (macOS Tahoe)
// and Material (Material 3 Expressive, docs/material-look.md). In the
// Material look the same tokens answer with Material's colour roles, type,
// shape scale and motion, so a component that only reads tokens follows
// either look; `m3` has every colour role for components that draw Material's
// own parts.
Singleton {
    id: root

    // (this root is the Material look's: material/, run instead of shell/ when
    // Settings → Appearance → Look is Material)
    readonly property bool material: true

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
    readonly property color accent: material && m3.primary ? m3.primary : Config.options.appearance.accent === "wallpaper" ? Wallpaper.accent : (accents[Config.options.appearance.accent] ?? accents.blue)

    // The Material look's colour scheme: every Material 3 colour role
    // (primary, onPrimary, primaryContainer, surfaceContainerHigh, outline, …;
    // see tools/gen-material.sh), from the wallpaper (Material You) or a chosen
    // seed, in the chosen style, at the chosen contrast. Empty in the Glass look.
    readonly property string materialSeed: {
        const s = Config.options.appearance.materialSeed;
        return /^#[0-9a-fA-F]{6}$/.test(s) ? s : Wallpaper.seed;
    }
    readonly property var m3: material ? seeThrough(MaterialColor.MCU.scheme(materialSeed, dark, Config.options.appearance.materialStyle, [0, 0.5, 1][contrastLevel])) : ({})
    // Transparency (Settings → Appearance): the containers (panels' and
    // tiles', tonal buttons') let the blurred desktop through; text, accent
    // fills and outlines stay solid. (Hyprland blurs layers above 40% alpha:
    // the most see-through stays over that.)
    readonly property var containerRoles: ["surface", "surfaceDim", "surfaceBright", "surfaceVariant", "surfaceContainerLowest", "surfaceContainerLow", "surfaceContainer", "surfaceContainerHigh", "surfaceContainerHighest", "primaryContainer", "secondaryContainer", "tertiaryContainer", "errorContainer"]
    function seeThrough(s) {
        if (transparency <= 0)
            return s;
        const o = Object.assign({}, s);
        for (const k of containerRoles)
            if (o[k] !== undefined)
                o[k] = Qt.alpha(o[k], controlAlpha);
        return o;
    }
    // A light accent (yellow, cyan, a pale wallpaper colour) can't carry white
    // text: text and glyphs on accent fills are dark then (color.onAccent)
    readonly property bool lightAccent: AccentRamp.contrast(accent, "white") < 3 // (measured as real contrast: white on green #30D158 is only ~2:1)
    // The accent as a family of shades (one hue, light to dark, in OKLCH so
    // they stay vivid). Each use draws the accent where it reads on its
    // background, a lighter shade of it on dark backgrounds when that's what
    // reads, and otherwise neutral, never a darkened (muddy) accent.
    readonly property var accentRamp: AccentRamp.ramp(accent)
    function accentFor(background, ratio) {
        return AccentRamp.pick(accentRamp, accent, background, ratio);
    }
    // An "on" round button (Control Center), as macOS draws it: a white
    // circle with the glyph in the accent (≥ 3:1 on white), or near-black
    // for an accent that doesn't read on white (yellow): the white circle
    // already says it's on
    readonly property color accentOnFill: material ? m3.primary : "white"
    readonly property color accentOnGlyph: material ? m3.onPrimary : accentFor("white", 3)
    // Accent text and small glyphs on the window background (≥ 4.5:1)
    readonly property color accentInk: material ? m3.primary : accentFor(dark ? "#1E1E21" : "#F5F5F7", 4.5)
    // Selected rows, chips and soft highlights: a pale shade in light mode,
    // the accent faintly in dark mode
    readonly property color accentTint: material ? m3.secondaryContainer : dark ? Qt.alpha(accent, 0.26) : Qt.alpha(accentRamp[1], 0.9)

    readonly property QtObject font: QtObject {
        readonly property string family: root.material ? "iRice Sans Flex" : "Inter"
        readonly property string mono: "JetBrains Mono"
        readonly property int caption: 11
        readonly property int small: 12
        readonly property int body: 13
        // (Material: its title, title large and headline sizes)
        readonly property int title: root.material ? 16 : 15
        readonly property int large: root.material ? 22 : 20
        readonly property int display: root.material ? 28 : 24
    }

    readonly property bool reduceTransparency: Config.options.appearance.reduceTransparency
    // The Material look's blur (Android 17's): panels, the volume panel, the
    // power menu, the app drawer and menus let the blurred desktop through,
    // tinted by their own colour. Off (or Reduce transparency): opaque
    readonly property real transparency: material && !reduceTransparency && CoolDown.glass !== "solid" ? Math.max(0, Math.min(1, Config.options.appearance.materialTransparency ?? 0)) : 0
    readonly property bool blur: transparency > 0
    readonly property real panelAlpha: 1 - 0.55 * transparency
    readonly property real controlAlpha: 1 - 0.4 * transparency // (tiles and buttons a little more solid than their panel)
    // A panel's colour, see-through as far as Transparency says
    function veil(c) {
        return blur ? Qt.alpha(c, panelAlpha) : c;
    }
    // Text contrast (Settings → Appearance): 0 standard (macOS vibrancy),
    // 1 higher (text about 4.5:1 over glass), 2 maximum (about 7:1). It's
    // for text, icons, controls and selection; borders barely change.
    // ("increaseContrast" is the old on/off switch: on counts as higher.)
    readonly property int contrastLevel: Config.options.appearance.contrast === "maximum" ? 2 : Config.options.appearance.contrast === "higher" || Config.options.appearance.increaseContrast ? 1 : 0
    readonly property bool contrast: contrastLevel > 0
    function level(standard, higher, maximum) {
        return contrastLevel === 2 ? maximum : contrastLevel === 1 ? higher : standard;
    }
    // Selection and accent tints, stronger with more contrast
    function accentAlpha(a) {
        return Qt.alpha(accent, Math.min(1, a * level(1, 1.3, 1.6)));
    }

    readonly property QtObject color: QtObject {
        readonly property color label: root.material ? root.m3.onSurface : root.dark ? Qt.rgba(1, 1, 1, root.level(0.92, 1, 1)) : Qt.rgba(0, 0, 0, root.level(0.85, 0.9, 0.95))
        readonly property color secondary: root.material ? root.m3.onSurfaceVariant : root.dark ? Qt.rgba(1, 1, 1, root.level(0.58, 0.72, 0.85)) : Qt.rgba(0, 0, 0, root.level(0.52, 0.68, 0.8))
        readonly property color tertiary: root.material ? Qt.alpha(root.m3.onSurfaceVariant, root.level(0.72, 0.85, 1)) : root.dark ? Qt.rgba(1, 1, 1, root.level(0.34, 0.56, 0.7)) : Qt.rgba(0, 0, 0, root.level(0.3, 0.6, 0.7))
        // (Material: its outline variant, faint, so panel edges and dividers stay quiet)
        readonly property color separator: root.material ? Qt.alpha(root.m3.outlineVariant, root.level(0.45, 0.7, 1)) : root.dark ? Qt.rgba(1, 1, 1, root.level(0.12, 0.14, 0.16)) : Qt.rgba(0, 0, 0, root.level(0.1, 0.12, 0.14))
        readonly property color outline: root.material ? root.m3.outline : root.dark ? Qt.rgba(1, 1, 1, 0.42) : Qt.rgba(0, 0, 0, 0.38)
        // Material: containers, with state layers (8% hover, 10% pressed of the content colour)
        readonly property color fill: root.material ? root.m3.surfaceContainerHighest : root.dark ? Qt.rgba(1, 1, 1, root.level(0.1, 0.14, 0.18)) : Qt.rgba(0, 0, 0, root.level(0.06, 0.09, 0.12))
        readonly property color fillHover: root.material ? Qt.tint(root.m3.surfaceContainerHighest, Qt.alpha(root.m3.onSurface, 0.08)) : root.dark ? Qt.rgba(1, 1, 1, root.level(0.16, 0.2, 0.24)) : Qt.rgba(0, 0, 0, root.level(0.1, 0.13, 0.16))
        readonly property color fillPressed: root.material ? Qt.tint(root.m3.surfaceContainerHighest, Qt.alpha(root.m3.onSurface, 0.1)) : root.dark ? Qt.rgba(1, 1, 1, root.level(0.22, 0.26, 0.3)) : Qt.rgba(0, 0, 0, root.level(0.15, 0.18, 0.21))
        readonly property color platter: root.material ? root.m3.surfaceContainerHigh : root.dark ? Qt.rgba(1, 1, 1, 0.07) : Qt.rgba(1, 1, 1, 0.45)
        // Unchecked toggles, slider and segmented tracks
        readonly property color toggleOff: root.material ? root.m3.surfaceContainerHighest : root.dark ? Qt.rgba(1, 1, 1, root.level(0.14, 0.24, 0.32)) : Qt.rgba(0, 0, 0, root.level(0.08, 0.16, 0.24))
        // A menu's hovered row: macOS fills it with the accent (white text),
        // Material lays a state layer over it (text unchanged)
        readonly property color highlight: root.material ? Qt.alpha(root.m3.onSurface, 0.1) : root.accent
        readonly property color onHighlight: root.material ? root.m3.onSurface : onAccent
        // A selected row (Spotlight's chosen result, Settings' page): the
        // accent, or Material's secondary container
        readonly property color selection: root.material ? root.m3.secondaryContainer : root.accent
        readonly property color onSelection: root.material ? root.m3.onSecondaryContainer : onAccent
        readonly property color onAccent: root.material ? root.m3.onPrimary : root.lightAccent ? "#1C1C1E" : "white" // text and glyphs on accent fills: near-black on light accents (yellow), else white
        readonly property color danger: root.material ? root.m3.error : root.dark ? "#FF453A" : "#FF3B30"
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
        // (veil: the Material look's, Android 17's tinted blur: no lens, all
        // frost, mostly its own tonal colour)
        readonly property var presets: ({ liquid: [1, 0.75, 0, 0.75], frosted: [0.3, 1, 0.45, 0.5], solid: [0, 0, 1, 0], veil: [0, 1, root.panelAlpha, 0.5] })
        // (the Material look: tonal surfaces, blurred behind unless that's
        // off, then opaque)
        readonly property string chosen: root.reduceTransparency ? "solid" : root.material ? (root.blur ? "veil" : "solid") : Config.options.appearance.glass === "auto" ? "liquid" : Config.options.appearance.glass
        // Cool-down swaps in a cheaper material (never a heavier one)
        readonly property string material: CoolDown.glass === "solid" ? "solid" : CoolDown.glass === "frosted" && chosen !== "solid" && chosen !== "veil" ? "frosted" : chosen
        readonly property var amounts: material === "custom" ? [Config.options.appearance.glassLens, Config.options.appearance.glassFrost, Config.options.appearance.glassSolid, Config.options.appearance.glassTint] : (presets[material] ?? presets.liquid)
        // Switching materials (Cool-down, or picking Glass, Frost or Solid)
        // glides over about 0.6 s rather than snapping; Custom's sliders
        // apply at once
        property real lensAmount: amounts[0]
        property real frostAmount: amounts[1]
        property real solidAmount: amounts[2]
        property real tintAmount: amounts[3] ?? 0.45
        readonly property bool gliding: material !== "custom"
        Behavior on lensAmount { enabled: glassTheme.gliding; NumberAnimation { duration: 600; easing.type: Easing.InOutCubic } }
        Behavior on frostAmount { enabled: glassTheme.gliding; NumberAnimation { duration: 600; easing.type: Easing.InOutCubic } }
        Behavior on solidAmount { enabled: glassTheme.gliding; NumberAnimation { duration: 600; easing.type: Easing.InOutCubic } }
        Behavior on tintAmount { enabled: glassTheme.gliding; NumberAnimation { duration: 600; easing.type: Easing.InOutCubic } }
        // How much 3D lighting (specular, glint, sheen, inner shadow): full
        // for clear glass, fading to an even hairline as it frosts, none solid
        readonly property real gloss: lensAmount * (1 - 0.85 * frostAmount) * (1 - solidAmount)
        // The nearest named material, for code that needs a yes/no answer
        readonly property string tier: solidAmount >= 0.97 ? "solid" : solidAmount < 0.4 && (frostAmount < 0.4 || lensAmount >= 0.6) ? "liquid" : "frosted" // (a strong lens reads as glass even when frosty)
        // Lens thickness: thin | depth | auto (thick on small controls)
        readonly property string depth: Config.options.appearance.glassDepth
        readonly property color base: root.dark ? Qt.rgba(0.08, 0.08, 0.1, 0.16) : Qt.rgba(0.98, 0.98, 1.0, 0.3)
        readonly property color tint: Config.options.appearance.wallpaperTint ? Qt.tint(base, Qt.alpha(Wallpaper.tone, root.dark ? 0.1 : 0.08)) : base
        readonly property color solid: root.material ? root.m3.surfaceContainer : root.dark ? "#26262A" : "#F2F2F5"
        // Frosted sits over compositor-blurred windows, so it needs more body
        readonly property color frost: root.dark ? Qt.rgba(0.11, 0.11, 0.13, 0.62) : Qt.rgba(0.97, 0.97, 0.98, 0.66)
        readonly property real saturation: 1.35
        // (Material: no rim; it read as a dark border round see-through panels)
        readonly property real rim: root.material ? 0 : root.dark ? 0.5 : 0.65
        // Optics (Settings → Glass → Advanced), the same for every material:
        // how far the rim bends what's behind follows the index of
        // refraction like real glass, (1 − 1/IOR), relative to glass's 1.5;
        // `band` widens or narrows the bending rim; dispersion splits the
        // colours by slightly different amounts at the rim
        readonly property real ior: Math.max(1, Math.min(2, Config.options.appearance.glassIor ?? 1.5))
        readonly property real bend: (1 - 1 / ior) / (1 - 1 / 1.5)
        readonly property real band: Math.max(0.4, Math.min(2, Config.options.appearance.glassRefraction ?? 1))
        readonly property real dispersion: Math.max(0, Math.min(1, Config.options.appearance.glassDispersion ?? 0.35)) / 0.35
        readonly property real refraction: 36 * bend
        readonly property real chroma: 0.12 * dispersion
        // (Material, see-through: none; Hyprland blurs a soft shadow into a
        // blurred rim round each panel)
        readonly property real shadow: root.material ? (root.blur ? 0 : root.dark ? 0.24 : 0.1) : root.dark ? 0.32 : 0.14

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
                refraction: (thick ? 30 : 22) * l * bend,
                depth: (thick ? 0.55 : 0) * l * Math.min(1.5, bend),
                bevel: (thick ? 26 : 18) * (0.5 + 0.5 * l) * band,
                frost: f,
                chroma: (thick ? 0.28 : 0.18) * l * dispersion,
                rim: root.material ? 0 : (0.12 + 0.16 * l) * (1 - s),
                gloss: gloss,
                tint: t,
                saturation: 1.35 + 0.15 * f - 0.5 * s
            };
        }
    }

    readonly property QtObject radius: QtObject {
        // Material: its shape scale (small 8, medium 12, large 16,
        // extra-large 28); buttons and chips are pills where they're drawn
        readonly property int item: root.material ? 8 : 6
        readonly property int control: root.material ? 12 : 10
        readonly property int menu: root.material ? 16 : 12
        readonly property int popover: root.material ? 28 : 18
        readonly property int module: root.material ? 24 : 20
        // iOS tiles and widgets: half a 1×1 Control Center button (a 320 px grid
        // of four, 10 px apart), with continuous corners, so every card's curve
        // matches the round buttons'
        readonly property int tile: root.material ? 28 : Math.round((320 - 3 * 10) / 8)
        readonly property int panel: root.material ? 28 : 26
    }

    readonly property QtObject motion: QtObject {
        // (the Material look moves on Material's expressive springs)
        readonly property bool expressive: root.material || Config.options.appearance.motion === "expressive"
        // Springs (SpringAnimation): open/expand, settle back, and elastic pull
        // (Material: its default spatial spring)
        readonly property real spring: root.material ? 6.08 : expressive ? 3.0 : 3.8
        readonly property real damping: root.material ? 0.5 : expressive ? 0.22 : 0.32
        // Material 3 Expressive's springs as SpringAnimation values (Qt's spring
        // steps 16 ms: spring = k / 62.5, damping = 2ζ√k / 62.5). Spatial ones
        // (position, size, shape) overshoot; for colour and opacity use the
        // `hover`/`popover` durations, which don't.
        //   fast ζ 0.6 k 800 · default ζ 0.8 k 380 · slow ζ 0.8 k 200
        readonly property var springFast: ({ spring: 12.8, damping: 0.54 })
        readonly property var springDefault: ({ spring: 6.08, damping: 0.5 })
        readonly property var springSlow: ({ spring: 3.2, damping: 0.36 })
        // A press (Expressive): the control swells and its corners square
        // off, then it springs back past its size and settles (ζ ~0.35)
        readonly property var springPress: ({ spring: 7.5, damping: 0.3 })
        // (not tied to cool-down's slower motion: a press is cheap; only
        // Reduce motion turns it off)
        readonly property bool pressBounce: root.material && Config.options.appearance.reduceMotion !== true
        // How much a pressed control grows: ~6%, at most ~10 px for big ones
        function pressGrow(size) {
            return pressBounce ? Math.min(0.06, 10 / Math.max(1, size)) : 0;
        }
        // (Material has no elastic give: presses change shape instead)
        readonly property real pull: root.material ? 0 : expressive ? 14 : 6 // how far a panel gives when pressed and dragged
        readonly property real controlPull: root.material ? 0 : expressive ? 5 : 2.5 // same for small controls
        readonly property real squish: root.material ? 0.97 : expressive ? 0.9 : 0.95 // press scale
        readonly property int press: Math.round(100 * CoolDown.motionScale)
        readonly property int hover: Math.round(150 * CoolDown.motionScale)
        readonly property int popover: Math.round(220 * CoolDown.motionScale)
        readonly property int panel: Math.round(320 * CoolDown.motionScale)
        readonly property int large: Math.round(450 * CoolDown.motionScale)
        // Apple-like decelerate (Material: emphasized decelerate, standard,
        // emphasized accelerate); QML BezierSpline control points.
        readonly property var emphasized: root.material ? [0.05, 0.7, 0.1, 1.0, 1, 1] : [0.2, 0.9, 0.1, 1.0, 1, 1]
        readonly property var standard: root.material ? [0.2, 0.0, 0.0, 1.0, 1, 1] : [0.3, 0.0, 0.2, 1.0, 1, 1]
        readonly property var exit: root.material ? [0.3, 0.0, 0.8, 0.15, 1, 1] : [0.4, 0.0, 1.0, 1.0, 1, 1]
    }
}
