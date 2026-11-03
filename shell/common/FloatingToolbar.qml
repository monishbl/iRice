import QtQuick
import QtQuick.Effects

// Material 3 Expressive's floating toolbar: a pill that floats over the
// content (an elevation 3 shadow), holding a row of icon buttons; standard
// (surfaceContainer) or vibrant (primaryContainer). Put the buttons in it:
//   FloatingToolbar { IconButton { … } IconButton { … } }
Item {
    id: root

    property bool vibrant: false
    property real padding: 8
    property real spacing: 4
    default property alias content: row.data
    readonly property color ink: vibrant ? Theme.m3.onPrimaryContainer : Theme.m3.onSurfaceVariant

    implicitWidth: row.implicitWidth + 2 * padding
    implicitHeight: Math.max(64, row.implicitHeight + 2 * padding)

    RectangularShadow {
        anchors.fill: bg
        radius: bg.radius
        offset.y: 3
        blur: 12
        color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.45 : 0.18)
    }
    Rectangle {
        id: bg
        anchors.fill: parent
        radius: height / 2
        color: root.vibrant ? Theme.m3.primaryContainer : Theme.m3.surfaceContainer
    }
    Row {
        id: row
        anchors.centerIn: parent
        spacing: root.spacing
    }
}
