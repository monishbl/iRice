import QtQuick

// A tiny screen outline with one region filled, for tiling menus.
// frac = [x, y, w, h] as fractions of the screen.
Rectangle {
    id: root

    property var frac: [0, 0, 1, 1]
    property bool hot: false

    implicitWidth: 36
    implicitHeight: 24
    radius: 5
    color: "transparent"
    border.width: 1.5
    border.color: hot ? Theme.color.onAccent : Theme.color.secondary

    Rectangle {
        x: 3 + (root.width - 6) * root.frac[0]
        y: 3 + (root.height - 6) * root.frac[1]
        width: Math.max(2, (root.width - 6) * root.frac[2] - 1)
        height: Math.max(2, (root.height - 6) * root.frac[3] - 1)
        radius: 2
        color: root.hot ? Theme.color.onAccent : Theme.color.label
    }
}
