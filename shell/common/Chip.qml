import QtQuick

// A chip that turns one thing of several on or off (which screens, which
// days, which pieces). Glass: a pill in the fill colour, on in the accent
// (`solid`) or an accent tint with an accent edge. Material: Material 3's
// filter chip, 32 px with 8 px corners, outlined while off and
// secondaryContainer with a tick while on (`tick: false` for one-letter
// chips such as days). Keyboard: Tab to it, Space or Enter.
//   Chip { text: "Mon"; checked: on; onClicked: toggle() }
Squircle {
    id: root

    property string text
    property bool checked: false
    property bool solid: false // (Glass: on in the accent itself)
    property bool tick: true // (Material: the tick while on)
    signal clicked

    readonly property bool ticked: Theme.material && tick && checked
    readonly property real stateLayer: mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0
    implicitWidth: label.implicitWidth + (Theme.material ? (ticked ? 8 + 18 + 8 + 16 : 32) : 20)
    implicitHeight: Theme.material ? 32 : 26
    radius: Theme.material ? 8 : height / 2
    color: Theme.material ? (checked ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, stateLayer)) : Qt.alpha(Theme.m3.onSurfaceVariant, stateLayer))
        : checked ? (solid ? (mouse.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent) : Theme.accentTint)
        : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
    border.width: Theme.material ? (checked ? 0 : 1) : checked && !solid ? 1 : 0
    border.color: Theme.material ? Theme.m3.outlineVariant : Theme.accent
    opacity: enabled ? 1 : Theme.material ? 0.38 : 0.45
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    Behavior on implicitWidth { enabled: Theme.material; NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

    activeFocusOnTab: true
    Keys.onPressed: event => {
        if (!event.isAutoRepeat && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            root.clicked();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.CheckBox
    Accessible.name: text
    Accessible.checked: checked
    Accessible.onPressAction: root.clicked()
    FocusRing {
        radius: root.radius + 4
    }

    Icon {
        visible: root.ticked
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        name: "check"
        size: 18
        color: Theme.material ? Theme.m3.onSecondaryContainer : "transparent"
    }
    Text {
        id: label
        anchors.verticalCenter: parent.verticalCenter
        x: root.ticked ? root.width - 16 - width : Math.round((root.width - width) / 2)
        text: root.text
        color: Theme.material ? (root.checked ? Theme.m3.onSecondaryContainer : Theme.m3.onSurfaceVariant)
            : root.checked && root.solid ? Theme.color.onAccent : Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Font.Medium; letterSpacing: Theme.material ? 0.1 : 0 }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            root.forceActiveFocus();
            root.clicked();
        }
    }
}
