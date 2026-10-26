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
        const shapes = visible ? glasses.filter(g => g.pluginMode).map(g => g.pluginShape()).filter(s => s !== null) : [];
        const key = JSON.stringify(shapes);
        if (key === sentShapes)
            return;
        sentShapes = key;
        GlassPlugin.update(mon, ns, shapes);
    }
    onVisibleChanged: glassChanged()
    Connections {
        target: GlassPlugin
        function onReady() {
            root.sentShapes = ""; // the plugin may have been reloaded with no shapes
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
    Component.onDestruction: if (sentShapes !== "" && sentShapes !== "[]") GlassPlugin.update(screen?.name ?? "", WlrLayershell.namespace, [])
}
