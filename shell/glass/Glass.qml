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

    // The plugin's parameters for this shape, in window coordinates (follows
    // parent moves, scales and fades), or null when it shouldn't show
    // The last shape sent, rebuilt only when the geometry, opacity or material
    // (`material`, bumped by the window) changed; the same array otherwise,
    // so the window can tell nothing moved without comparing numbers
    property var cachedShape: null
    property string cachedKey: ""
    function pluginShape(material) {
        let o = 1;
        for (let it = root; it; it = it.parent) {
            if (!it.visible)
                return null;
            o *= it.opacity;
        }
        if (o <= 0.002)
            return null;
        const p0 = root.mapToItem(null, shadowMargin, shadowMargin);
        const p1 = root.mapToItem(null, width - shadowMargin, height - shadowMargin);
        const w = p1.x - p0.x, h = p1.y - p0.y;
        if (w < 1 || h < 1)
            return null;
        const key = p0.x + " " + p0.y + " " + w + " " + h + " " + o + " " + radius + " " + material + " " + GlassPlugin.version;
        if (key === cachedKey)
            return cachedShape;
        const k = w / Math.max(1, width - 2 * shadowMargin);
        const pr = Theme.glass.preset(Math.min(w, h));
        const t = pr.tint;
        const shape = [p0.x, p0.y, w, h, radius * k, pr.mode, pr.refraction, pr.depth, Math.min(pr.bevel, radius) * k, pr.frost, pr.chroma, pr.rim, t.r, t.g, t.b, t.a, pr.saturation, o];
        cachedKey = key;
        cachedShape = GlassPlugin.version >= 2 ? shape.concat([pr.gloss]) : shape;
        return cachedShape;
    }
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
    Component.onDestruction: registeredHost?.unregisterGlass?.(root)

    Item {
        id: shapeItem
        x: root.shadowMargin
        y: root.shadowMargin
        width: root.width - 2 * root.shadowMargin
        height: root.height - 2 * root.shadowMargin
    }
    // Increase contrast: a clear outline, so surfaces never melt into what's behind
    Rectangle {
        visible: Theme.contrast
        anchors.fill: shapeItem
        radius: root.radius
        color: "transparent"
        border.width: 1
        border.color: Theme.color.outline
    }
}
