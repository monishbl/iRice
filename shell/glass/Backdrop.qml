import QtQuick
import qs.common

// The blurred wallpaper that liquid glass refracts, exposed as `texture`.
// Loads the small image the background window bakes for this screen, so glass
// windows share one cached blur instead of each running their own.
Item {
    id: root

    property var screen
    property bool needed: true // false when no glass here refracts the wallpaper
    readonly property alias texture: image

    Image {
        id: image
        visible: false
        source: Theme.glass.tier === "solid" || !root.needed ? "" : Wallpaper.backdropUrl(root.screen?.name ?? "default")
        asynchronous: true
        cache: false
        smooth: true
    }
}
