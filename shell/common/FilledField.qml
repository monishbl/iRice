import QtQuick

// Material's field, shaped like Settings' search bar: a filled pill (rounded
// to half its height, at most 28 px on a tall field) that tints on hover and
// takes a 2 px primary outline while active (error red when `error`).
// TextField's, Dropdown's, Settings' Choice's and the password prompt's look
// in the Material look (Glass draws its own), loaded only there:
//   Loader { anchors.fill: parent; active: Theme.material; sourceComponent: FilledField { active: input.activeFocus } }
Rectangle {
    id: root

    property bool active: false // typing, or the list open
    property bool hovered: hover.hovered
    property bool error: false // a wrong entry: outlined in error red
    property alias cursorShape: hover.cursorShape

    radius: Math.min(height / 2, 28)
    color: hovered && !active ? Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHighest
    border.width: active || error ? 2 : 0
    border.color: error ? Theme.m3.error : Theme.m3.primary
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    HoverHandler {
        id: hover
    }
}
