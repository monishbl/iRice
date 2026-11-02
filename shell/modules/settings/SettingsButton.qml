import QtQuick
import qs.common

// Text button for Settings pages: neutral fill, `primary` (accent) for the
// one main action, `danger` for removing. Keyboard: Tab to it, Space or Enter.
Squircle {
    id: root

    property string text
    property string icon
    property bool primary: false
    property bool danger: false
    property bool busy: false
    signal clicked

    readonly property bool active: enabled && !busy
    implicitWidth: inner.implicitWidth + 24
    implicitHeight: 28
    radius: Theme.radius.control
    opacity: enabled ? 1 : 0.45
    activeFocusOnTab: true
    color: primary ? (mouse.pressed ? Qt.darker(Theme.accent, 1.2) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent) : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
    scale: mouse.pressed ? 0.97 : 1
    Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

    Keys.onPressed: event => {
        if (active && !event.isAutoRepeat && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            root.clicked();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.Button
    Accessible.name: text
    // (the settings row it's in: "Use" or "Remove" alone don't say what for)
    Accessible.description: {
        let p = parent;
        for (let i = 0; p && i < 4; i++, p = p.parent)
            if (typeof p.label === "string" && p.label !== "")
                return p.label;
        return "";
    }
    Accessible.onPressAction: if (active) root.clicked()

    FocusRing {
        radius: root.radius + 4
    }

    // (not a Row: in Settings pages Row is the settings row)
    Item {
        id: inner
        anchors.centerIn: parent
        implicitWidth: (glyph.visible ? glyph.implicitWidth + 6 : 0) + label.implicitWidth
        implicitHeight: label.implicitHeight
        width: implicitWidth
        height: implicitHeight
        Icon {
            id: glyph
            visible: root.icon !== "" || root.busy
            anchors.verticalCenter: parent.verticalCenter
            name: root.busy ? "circle-notch" : root.icon
            bold: true
            size: 12
            color: label.color
            RotationAnimation on rotation {
                running: root.busy
                from: 0
                to: 360
                duration: 900
                loops: Animation.Infinite
            }
        }
        Text {
            id: label
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            text: root.text
            color: root.primary ? Theme.color.onAccent : root.danger ? Theme.color.danger : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: root.primary ? Font.DemiBold : Font.Medium }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.active
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            root.forceActiveFocus();
            root.clicked();
        }
    }
}
