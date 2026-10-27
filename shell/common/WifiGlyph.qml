import QtQuick

// Wi-Fi, drawn to match the battery: a solid fan whose unlit bars are the
// same colour, faint (Phosphor's wifi-low/medium/high share one canvas, so
// they stack exactly). Off: crossed out; on with no network: all faint.
//   WifiGlyph { enabled: true; connected: true; strength: 0.7; color: bar.fg }
Item {
    id: root

    property bool on: true
    property bool connected: true
    property real strength: 1 // 0 .. 1
    property color color: "white"
    property int size: 16

    readonly property string lit: !connected ? "" : strength > 0.66 ? "wifi-high" : strength > 0.33 ? "wifi-medium" : "wifi-low"

    implicitWidth: size
    implicitHeight: size

    Icon {
        anchors.centerIn: parent
        name: root.on ? "wifi-high" : "wifi-slash"
        fill: true
        size: root.size
        color: root.on ? Qt.alpha(root.color, 0.35) : root.color
    }
    Icon {
        visible: root.on && root.lit !== ""
        anchors.centerIn: parent
        name: root.lit
        fill: true
        size: root.size
        color: root.color
    }
}
