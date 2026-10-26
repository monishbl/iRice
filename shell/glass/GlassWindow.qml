import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// A layer-shell panel that hosts Glass surfaces. It carries a blurred copy of
// the wallpaper and knows its position on screen, so glass can refract the
// right part of the wallpaper. Exclusive zones of other panels are ignored to
// keep that position exact.
PanelWindow {
    id: root

    property string name
    readonly property var backdrop: backdropItem.texture

    WlrLayershell.namespace: "irice:" + name
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"

    readonly property real screenX: anchors.left ? margins.left : anchors.right ? (screen?.width ?? 0) - width - margins.right : ((screen?.width ?? 0) - width) / 2
    readonly property real screenY: anchors.top ? margins.top : anchors.bottom ? (screen?.height ?? 0) - height - margins.bottom : ((screen?.height ?? 0) - height) / 2

    Backdrop {
        id: backdropItem
        screen: root.screen
        needed: root.glasses.some(g => g.modeIndex === 0)
    }

    // Frosted glass relies on Hyprland blurring behind "irice:*" layers (see
    // hypr/irice/rules.lua). Glass surfaces still register here so a window
    // knows what it hosts.
    property var glasses: []
    function registerGlass(g) {
        glasses = glasses.concat([g]);
        glassChanged();
    }
    function unregisterGlass(g) {
        glasses = glasses.filter(x => x !== g);
        glassChanged();
    }

    // With the irice-glass plugin, this window's glass shapes are sent to the
    // compositor. A change starts a short per-frame sync (animations move and
    // fade glass through their parents), sending only when shapes differ.
    property string sentShapes: ""
    property string sentMonitor: "" // where they went (a window can move to another screen)
    property int materialSerial: 0 // bumped when the material changes: glass rebuilds its shape
    property var lastShapes: [] // the arrays sent last (each glass returns the same one while unchanged)
    function glassChanged() {
        syncWindow.restart();
        if (!syncFrames.running)
            syncFrames.start();
    }
    function syncGlass() {
        const ns = WlrLayershell.namespace;
        const mon = screen?.name ?? "";
        if (!mon)
            return;
        const shapes = visible ? glasses.filter(g => g.pluginMode).map(g => g.pluginShape(materialSerial)).filter(s => s !== null) : [];
        // Nothing moved (the common case while syncing): skip the rest
        if (mon === sentMonitor && shapes.length === lastShapes.length && shapes.every((s, i) => s === lastShapes[i]))
            return;
        lastShapes = shapes;
        const key = JSON.stringify(shapes);
        if (key === sentShapes && mon === sentMonitor)
            return;
        // Moved to another screen: clear the shapes left on the old one
        if (sentMonitor !== "" && sentMonitor !== mon && sentShapes !== "" && sentShapes !== "[]")
            GlassPlugin.update(sentMonitor, ns, []);
        sentShapes = key;
        sentMonitor = mon;
        GlassPlugin.update(mon, ns, shapes);
    }
    onVisibleChanged: glassChanged()
    onScreenChanged: glassChanged()
    // The material (and its colours) changed: shapes carry it, so re-send
    Connections {
        target: Theme.glass
        function onAmountsChanged() { root.materialChanged(); }
        function onTintChanged() { root.materialChanged(); }
        function onDepthChanged() { root.materialChanged(); }
    }
    Connections {
        target: Theme
        function onDarkChanged() { root.materialChanged(); }
    }
    function materialChanged() {
        materialSerial++;
        glassChanged();
    }
    Connections {
        target: GlassPlugin
        function onReady() {
            root.sentShapes = ""; // the plugin may have been reloaded with no shapes
            root.lastShapes = [];
            root.materialSerial++; // and its version may differ
            root.syncGlass();
        }
    }
    FrameAnimation {
        id: syncFrames
        onTriggered: root.syncGlass()
    }
    Timer {
        id: syncWindow
        interval: 400 // keep syncing a little after the last change (animations settle)
        onTriggered: {
            syncFrames.stop();
            root.syncGlass();
        }
    }
    Component.onDestruction: if (sentMonitor !== "" && sentShapes !== "" && sentShapes !== "[]") GlassPlugin.update(sentMonitor, WlrLayershell.namespace, [])
}
