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
        if (nativeSync)
            native.item.add(g);
        glassChanged();
    }
    function unregisterGlass(g) {
        glasses = glasses.filter(x => x !== g);
        if (nativeSync)
            native.item.remove(g);
        glassChanged();
    }

    // With iRice's native module (shell/native), C++ keeps the plugin's shapes
    // in step on every frame this window draws, and nothing below runs
    readonly property bool nativeSync: native.status === Loader.Ready
    Loader {
        id: native
        active: Native.glassSync
        source: active ? "NativeSync.qml" : ""
        onLoaded: {
            item.monitor = Qt.binding(() => root.screen?.name ?? "");
            item.ns = Qt.binding(() => root.WlrLayershell.namespace);
            item.version = Qt.binding(() => GlassPlugin.version);
            item.active = Qt.binding(() => root.visible && GlassPlugin.available);
            for (const g of root.glasses)
                item.add(g);
        }
    }

    // With the irice-glass plugin, this window's glass shapes are sent to the
    // compositor. A change starts a short per-frame sync (animations move and
    // fade glass through their parents), sending only when shapes differ.
    property string sentShapes: "" // what went last: the shape count and shapes
    property string sentMonitor: "" // where they went (a window can move to another screen)
    property var lastShapes: [] // the shapes sent last (each glass returns the same string while unchanged)
    function glassChanged() {
        if (nativeSync) {
            native.item.poke();
            return;
        }
        idleFrames = 0;
        syncWindow.restart();
        if (!syncFrames.running)
            syncFrames.start();
    }
    // Frames in a row where nothing moved: the per-frame sync stops early
    // after a few (an animation in progress changes something every frame)
    property int idleFrames: 0
    function syncGlass() {
        const mon = screen?.name ?? "";
        if (!mon)
            return;
        const shapes = [];
        if (visible) {
            for (let i = 0; i < glasses.length; i++) {
                const g = glasses[i];
                if (!g.pluginMode)
                    continue;
                const s = g.pluginShape();
                if (s !== null)
                    shapes.push(s);
            }
        }
        // Nothing moved (the common case while syncing): skip the rest
        const last = lastShapes;
        if (mon === sentMonitor && shapes.length === last.length) {
            let same = true;
            for (let i = 0; same && i < shapes.length; i++)
                same = shapes[i] === last[i];
            if (same) {
                idleFrames++;
                return;
            }
        }
        idleFrames = 0;
        lastShapes = shapes;
        const key = shapes.length + (shapes.length ? " " + shapes.join(" ") : "");
        if (key === sentShapes && mon === sentMonitor)
            return;
        const ns = WlrLayershell.namespace;
        // Moved to another screen: clear the shapes left on the old one
        if (sentMonitor !== "" && sentMonitor !== mon && sentShapes !== "" && sentShapes !== "0")
            GlassPlugin.update(sentMonitor, ns, "0");
        sentShapes = key;
        sentMonitor = mon;
        GlassPlugin.update(mon, ns, key);
    }
    onVisibleChanged: glassChanged()
    onScreenChanged: glassChanged()
    Connections {
        target: GlassPlugin
        function onReady() {
            if (root.nativeSync) {
                native.item.resend();
                return;
            }
            root.sentShapes = ""; // the plugin may have been reloaded with no shapes
            root.lastShapes = [];
            root.syncGlass();
        }
    }
    FrameAnimation {
        id: syncFrames
        onTriggered: {
            root.syncGlass();
            if (root.idleFrames >= 8)
                stop();
        }
    }
    Timer {
        id: syncWindow
        interval: 400 // keep syncing a little after the last change (animations settle)
        onTriggered: {
            syncFrames.stop();
            root.syncGlass();
        }
    }
    property bool closing: false // being destroyed: its glass needn't unregister
    Component.onDestruction: {
        closing = true;
        if (sentMonitor !== "" && sentShapes !== "" && sentShapes !== "0")
            GlassPlugin.update(sentMonitor, WlrLayershell.namespace, "0");
    }
}
