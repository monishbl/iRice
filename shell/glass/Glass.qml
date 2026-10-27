import QtQuick
import Quickshell
import qs.common
import qs.services

// A rounded glass surface. Place it inside a GlassWindow; content goes on top as
// siblings or children. The item extends `shadowMargin` beyond the glass shape on
// every side so the shadow has room, so anchor content to `shape` instead.
ShaderEffect {
    id: root

    property real radius: Theme.radius.panel
    // How strongly the lens bends what's behind (refraction, depth, colour
    // fringes): menus under the menu bar use less, so its text doesn't ghost
    property real lensScale: 1
    property real bevel: Math.min(radius, 24)
    property real refraction: Theme.glass.refraction
    property real shadowMargin: 24
    property color tint: Theme.glass.tint
    readonly property alias shape: shapeItem

    readonly property var host: QsWindow.window
    // Glass in a GlassWindow can be drawn by the irice-glass Hyprland plugin
    // (true refraction of whatever is behind). Glass in the wallpaper window
    // only ever has wallpaper behind it, so it refracts the baked wallpaper.
    readonly property bool inGlassWindow: host?.registerGlass !== undefined
    property bool forceLiquid: false // e.g. widgets on the wallpaper
    property bool forceFrosted: false // kept for callers; frosted is the fallback anyway
    readonly property bool pluginMode: GlassPlugin.available && inGlassWindow && Theme.glass.tier !== "solid"
    // 0 liquid (baked wallpaper), 1 frosted (Hyprland blur + tint), 2 solid, 3 shadow only (plugin draws the glass)
    readonly property int modeIndex: Theme.glass.tier === "solid" ? 2 : pluginMode ? 3 : (!inGlassWindow || forceLiquid) ? 0 : 1

    // Shader inputs
    readonly property size itemSize: Qt.size(width, height)
    readonly property rect shapeRect: Qt.rect(shadowMargin, shadowMargin, width - 2 * shadowMargin, height - 2 * shadowMargin)
    property rect screenRect: Qt.rect(0, 0, width, height)
    readonly property size screenSize: Qt.size(host?.screen?.width ?? 1, host?.screen?.height ?? 1)
    property real chroma: Theme.glass.chroma
    property real rim: Theme.glass.rim * (1 - Theme.glass.solidAmount)
    readonly property real gloss: Theme.glass.gloss
    readonly property real mode: modeIndex
    readonly property real saturation: Theme.glass.saturation
    property real shadow: modeIndex === 0 || modeIndex === 2 ? Theme.glass.shadow : Theme.glass.shadow * 0.6
    readonly property color effectiveTint: modeIndex === 1 ? Theme.glass.frost : tint
    readonly property vector4d tintColor: Qt.vector4d(effectiveTint.r, effectiveTint.g, effectiveTint.b, effectiveTint.a)
    readonly property vector4d solidColor: Qt.vector4d(Theme.glass.solid.r, Theme.glass.solid.g, Theme.glass.solid.b, 1)
    readonly property var backdrop: host?.backdrop ?? null

    fragmentShader: Qt.resolvedUrl("glass.frag.qsb")
    blending: true

    // Screen position = window origin + position within the window. Recomputed on
    // geometry changes; call updatePosition() if an ancestor moves.
    function updatePosition() {
        if (!host)
            return;
        const p = root.mapToItem(null, 0, 0);
        screenRect = Qt.rect(host.screenX + p.x, host.screenY + p.y, width, height);
        if (pluginMode)
            host.glassChanged?.();
    }
    onOpacityChanged: if (pluginMode) host?.glassChanged?.()
    onVisibleChanged: if (pluginMode) host?.glassChanged?.()
    onPluginModeChanged: host?.glassChanged?.()

    // The material's numbers for the plugin, the same every frame:
    // [radius, mode, refraction, depth, bevel, frost, chroma, rim,
    //  tintR, tintG, tintB, tintA, saturation, gloss] (radius and bevel in item
    // pixels). Re-evaluated only when the material, radius or lens change, or
    // the shape crosses the small/large line (thick lens on small controls).
    readonly property bool smallShape: Math.min(width, height) - 2 * shadowMargin <= 140
    readonly property var pluginParams: {
        const pr = Theme.glass.preset(smallShape ? 140 : 141), t = pr.tint;
        return [radius, pr.mode, pr.refraction * lensScale, pr.depth * lensScale, Math.min(pr.bevel, radius), pr.frost, pr.chroma * lensScale, pr.rim, t.r, t.g, t.b, t.a, pr.saturation, pr.gloss];
    }

    // QML fallback (no native module): the plugin's numbers for this shape, in
    // window coordinates (follows parent moves, scales and fades), as the text
    // sent to it, or null when it shouldn't show. Called every frame while a
    // window syncs, so it's cheap when nothing changed: the same string comes
    // back, rebuilt only when the geometry, opacity or material changed.
    property string cachedShape: ""
    property var cachedVals: []
    function pluginShape() {
        if (!visible) // (effective: false when any ancestor is hidden)
            return null;
        let o = opacity;
        for (let it = parent; it; it = it.parent)
            o *= it.opacity;
        if (o <= 0.002)
            return null;
        const r = mapToItem(null, shadowMargin, shadowMargin, width - 2 * shadowMargin, height - 2 * shadowMargin);
        const w = r.width, h = r.height;
        if (w < 1 || h < 1)
            return null;
        const c = cachedVals, v = GlassPlugin.version, p = pluginParams;
        if (c[0] === r.x && c[1] === r.y && c[2] === w && c[3] === h && c[4] === o && c[5] === p && c[6] === v)
            return cachedShape;
        cachedVals = [r.x, r.y, w, h, o, p, v];
        const k = w / Math.max(1, width - 2 * shadowMargin);
        const shape = [r.x, r.y, w, h, p[0] * k, p[1], p[2], p[3], p[4] * k, p[5], p[6], p[7], p[8], p[9], p[10], p[11], p[12], o];
        if (v >= 2)
            shape.push(p[13]);
        let text = "";
        for (let i = 0; i < shape.length; i++)
            text += (i ? " " : "") + Math.round(shape[i] * 1000) / 1000;
        cachedShape = text;
        return text;
    }
    // A material change reaches the fallback sync through this
    onPluginParamsChanged: if (pluginMode) host?.glassChanged?.()
    onXChanged: updatePosition()
    onYChanged: updatePosition()
    onWidthChanged: updatePosition()
    onHeightChanged: updatePosition()
    Connections {
        target: root.host
        function onScreenXChanged() { root.updatePosition(); }
        function onScreenYChanged() { root.updatePosition(); }
    }
    // Register with the host window (for frosted blur regions) once it's known
    property var registeredHost: null
    function register() {
        if (!host || host === registeredHost)
            return;
        registeredHost?.unregisterGlass?.(root);
        host.registerGlass?.(root);
        registeredHost = host;
    }
    onHostChanged: {
        updatePosition();
        register();
    }
    Component.onCompleted: {
        updatePosition();
        register();
    }
    // (not when the whole window is going: it may already be gone, and
    // calling into it then only logs "invalid context")
    Component.onDestruction: if (registeredHost && !registeredHost.closing) registeredHost.unregisterGlass?.(root)

    Item {
        id: shapeItem
        x: root.shadowMargin
        y: root.shadowMargin
        width: root.width - 2 * root.shadowMargin
        height: root.height - 2 * root.shadowMargin
    }
}
