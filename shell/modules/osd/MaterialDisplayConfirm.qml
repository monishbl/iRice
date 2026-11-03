import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// Material "Keep these display settings?" dialog, centred on each screen:
// surfaceContainerHigh, corner 28, a headline, the countdown in words and as
// a wavy bar running down, then a text button (go back) and a filled one
// (keep). Same actions as the Glass dialog.
PanelWindow {
    id: root

    required property var target
    readonly property int m: 32 // room for the shadow
    // The countdown's length, as it was when the dialog came up
    property int total: 1
    Component.onCompleted: {
        total = Math.max(1, Displays.confirmLeft);
        progress = 1;
    }

    screen: target
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "irice:display-confirm"
    WlrLayershell.layer: WlrLayer.Overlay
    implicitWidth: 400 + 2 * m
    implicitHeight: body.implicitHeight + 48 + 2 * m

    property real progress: 0
    Behavior on progress {
        SpringAnimation {
            spring: Theme.motion.springDefault.spring
            damping: Theme.motion.springDefault.damping
            epsilon: 0.002
        }
    }

    // A Material button: filled (primary) or text, 40 high, fully round,
    // squaring off a little while pressed
    component DialogButton: Rectangle {
        id: button
        property string text
        property bool filled: false
        signal clicked
        implicitWidth: label.implicitWidth + (filled ? 48 : 24)
        implicitHeight: 40
        transform: pressFx1.effects
        ElasticControl { id: pressFx1; pressed: mouse.pressed }
        radius: mouse.pressed ? 12 : height / 2
        Behavior on radius { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
        readonly property color content: filled ? Theme.m3.onPrimary : Theme.m3.primary
        color: Qt.tint(filled ? Theme.m3.primary : Qt.alpha(Theme.m3.primary, 0), Qt.alpha(content, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if (!event.isAutoRepeat && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
                button.clicked();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: text
        Accessible.onPressAction: button.clicked()
        FocusRing {
            radius: button.radius + 4
        }
        Text {
            id: label
            anchors.centerIn: parent
            text: button.text
            color: button.content
            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; letterSpacing: 0.1 }
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: button.clicked()
        }
    }

    Item {
        anchors.fill: parent
        anchors.margins: root.m
        opacity: Math.max(0, Math.min(1, root.progress * 2))
        scale: 0.92 + 0.08 * root.progress

        RectangularShadow {
            anchors.fill: parent
            radius: Theme.radius.panel
            offset.y: 4
            blur: 20
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.45 : 0.2)
        }
        Rectangle {
            anchors.fill: parent
            radius: Theme.radius.panel
            color: Theme.m3.surfaceContainerHigh
        }
        Column {
            id: body
            x: 24
            y: 24
            width: parent.width - 48
            spacing: 16
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Keep these display settings?"
                color: Theme.m3.onSurface
                font { family: Theme.font.family; pixelSize: 24 }
            }
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Going back to the previous settings in " + Displays.confirmLeft + (Displays.confirmLeft === 1 ? " second." : " seconds.")
                color: Theme.m3.onSurfaceVariant
                font { family: Theme.font.family; pixelSize: 14; letterSpacing: 0.25; features: { "tnum": 1 } }
            }
            WavyProgress {
                width: parent.width
                value: Displays.confirmLeft / root.total
            }
            Item {
                width: parent.width
                height: buttons.height + 8
                Row {
                    id: buttons
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    spacing: 8
                    DialogButton {
                        text: "Go back"
                        onClicked: Displays.revert()
                    }
                    DialogButton {
                        text: "Keep settings"
                        filled: true
                        onClicked: Displays.keep()
                    }
                }
            }
        }
    }
}
