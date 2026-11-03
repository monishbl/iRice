import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// Material "Identify" card, top left of each screen: the screen's number on
// a cookie shape in primaryContainer, its name and size beside it, on a
// surfaceContainerHigh card (corner 28) that springs in. Clicks go through.
PanelWindow {
    id: root

    required property var target // the screen this card names
    readonly property int m: 24 // room for the shadow

    screen: target
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "irice:display-identify"
    WlrLayershell.layer: WlrLayer.Overlay
    anchors { top: true; left: true }
    margins { top: 48 - m; left: 32 - m }
    implicitWidth: Math.max(240, info.implicitWidth + 48) + 2 * m
    implicitHeight: 136 + 2 * m
    mask: Region {}

    property real progress: 0
    Component.onCompleted: progress = 1
    Behavior on progress {
        SpringAnimation {
            spring: Theme.motion.springDefault.spring
            damping: Theme.motion.springDefault.damping
            epsilon: 0.002
        }
    }

    Item {
        anchors.fill: parent
        anchors.margins: root.m
        opacity: Math.max(0, Math.min(1, root.progress * 2))
        scale: 0.9 + 0.1 * root.progress
        transformOrigin: Item.TopLeft

        RectangularShadow {
            anchors.fill: parent
            radius: Theme.radius.panel
            offset.y: 2
            blur: 12
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.4 : 0.16)
        }
        Rectangle {
            anchors.fill: parent
            radius: Theme.radius.panel
            color: Theme.m3.surfaceContainerHigh
        }
        Row {
            id: info
            anchors.centerIn: parent
            spacing: 20
            MaterialShape {
                anchors.verticalCenter: parent.verticalCenter
                width: 88
                height: 88
                shape: "cookie9"
                color: Theme.m3.primaryContainer
                Text {
                    anchors.centerIn: parent
                    text: Displays.numberOf(root.target.name) || "?"
                    color: Theme.m3.onPrimaryContainer
                    font { family: Theme.font.family; pixelSize: 45; weight: Font.Medium; features: { "tnum": 1 } }
                }
            }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4
                Text {
                    text: Displays.monitors.find(x => x.name === root.target.name)?.label ?? root.target.name
                    color: Theme.m3.onSurface
                    font { family: Theme.font.family; pixelSize: 22; weight: Font.Medium }
                }
                Text {
                    text: root.target.name + " · " + root.target.width + " × " + root.target.height
                    color: Theme.m3.onSurfaceVariant
                    font { family: Theme.font.family; pixelSize: 14; letterSpacing: 0.25; features: { "tnum": 1 } }
                }
            }
        }
    }
}
