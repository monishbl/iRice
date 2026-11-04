import QtQuick
import qs.common
import qs.glass
import qs.services

// Control Center's last row (Glass): you and the battery on the left, then
// Settings, Power and Edit Controls as round buttons, each a platter (a chip
// of its own glass without the panel), as the modules above. In Edit Controls
// the pencil turns into Done. Quick Settings (Material) has the same row in
// Material's own drawing.
Item {
    id: root

    property var screen
    property bool chip: false
    property bool editing: false

    width: parent?.width ?? 320
    height: 40

    // A round button on a platter: the hover and press fills of the 1×1
    // modules, the focus ring, Space or Enter to press. With `text`, a pill
    // with the word instead (Done, in the accent)
    component FooterButton: Platter {
        id: btn
        property string icon
        property string text
        property string accessibleName
        property bool accent: false
        signal clicked

        chip: root.chip
        width: text !== "" ? word.implicitWidth + 32 : height
        height: root.height
        radius: height / 2
        color: accent ? (mouse.containsMouse ? Qt.darker(Theme.accent, 1.08) : Theme.accent)
            : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fillHover
            : chip ? Qt.alpha("white", Theme.dark ? 0.07 : 0.28) : Theme.color.platter
        Behavior on width { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
        transform: elastic.effects
        activeFocusOnTab: visible
        Keys.onPressed: event => {
            if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                btn.clicked();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: accessibleName
        Accessible.onPressAction: btn.clicked()
        FocusRing {
            radius: btn.height / 2 + 4
        }
        Icon {
            anchors.centerIn: parent
            visible: btn.text === ""
            name: btn.icon
            size: 18
            color: Theme.color.label
        }
        Text {
            id: word
            anchors.centerIn: parent
            visible: btn.text !== ""
            text: btn.text
            color: Theme.color.onAccent
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: btn.clicked()
        }
        ElasticControl {
            id: elastic
            pressed: mouse.pressed
        }
    }

    // You: your picture (or the person glyph) in a circle
    Platter {
        id: you
        chip: root.chip
        width: root.height
        height: root.height
        radius: height / 2
        Avatar {
            anchors.fill: parent
            color: "transparent"
        }
    }

    // The battery and its percentage (computers that have one)
    Platter {
        visible: Island.hasBattery
        chip: root.chip
        anchors { left: you.right; leftMargin: 8 }
        width: batteryRow.implicitWidth + 28
        height: root.height
        radius: height / 2
        Accessible.role: Accessible.StaticText
        Accessible.name: "Battery " + Math.round(Island.level * 100) + "%" + (Island.charging ? ", charging" : "")
        Row {
            id: batteryRow
            anchors.centerIn: parent
            spacing: 8
            BatteryGlyph {
                anchors.verticalCenter: parent.verticalCenter
                size: 11
                level: Island.level
                charging: Island.charging && Island.level < 1
                plugged: Island.charging && Island.level >= 1
                color: Theme.color.label
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Math.round(Island.level * 100) + "%"
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium; features: { "tnum": 1 } }
            }
        }
    }

    Row {
        anchors.right: parent.right
        spacing: 8
        FooterButton {
            icon: "gear-six"
            accessibleName: "Open Settings"
            onClicked: Panels.openSettings()
        }
        FooterButton {
            icon: "power"
            accessibleName: "Show power options"
            onClicked: Panels.show("session", root.screen)
        }
        FooterButton {
            icon: "pencil-simple"
            text: root.editing ? "Done" : ""
            accent: root.editing
            accessibleName: root.editing ? "Done editing controls" : "Edit controls"
            onClicked: Panels.ccEditing = !Panels.ccEditing
        }
    }
}
