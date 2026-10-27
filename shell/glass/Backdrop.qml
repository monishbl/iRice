import QtQuick
import qs.common

// The blurred wallpaper that liquid glass refracts, exposed as `texture`.
// Loads the small image the background window bakes for this screen, so glass
// windows share one cached blur instead of each running their own. Two
// buffers: a new bake loads in the hidden one and takes over once it's ready,
// so glass never samples an empty image while it loads (that flickered every
// glass surface when the wallpaper changed).
Item {
    id: root

    property var screen
    property bool needed: true // false when no glass here refracts the wallpaper
    readonly property var texture: useB ? b : a

    property bool useB: false
    readonly property url want: Theme.glass.tier === "solid" || !needed ? "" : Wallpaper.backdropUrl(screen?.name ?? "default")
    onWantChanged: load()
    Component.onCompleted: load()
    function load() {
        const front = useB ? b : a, back = useB ? a : b;
        if (want.toString() === "") {
            a.source = "";
            b.source = "";
            return;
        }
        if (front.source === want)
            return;
        // Nothing shown yet: load straight into the front
        if (front.status !== Image.Ready) {
            front.source = want;
            return;
        }
        back.source = want;
    }
    function ready(img) {
        if (img.source !== want)
            return;
        const old = useB ? b : a;
        if (img === old)
            return;
        useB = img === b;
        old.source = ""; // release the previous bake
    }

    Image {
        id: a
        visible: false
        asynchronous: true
        cache: true
        smooth: true
        onStatusChanged: if (status === Image.Ready) root.ready(a)
    }
    Image {
        id: b
        visible: false
        asynchronous: true
        cache: true
        smooth: true
        onStatusChanged: if (status === Image.Ready) root.ready(b)
    }
}
