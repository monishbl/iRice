import QtQuick
import Quickshell
import qs.common
import qs.services

// Today's own options, over the panel (its Options button, or the Today
// widget's right-click menu): when it opens by itself, its entrance, the
// name the greeting uses and the chime. Escape or Done goes back to the day.
FocusScope {
    id: root

    readonly property var cfg: Config.options.today
    readonly property bool mat: Theme.material

    onVisibleChanged: if (visible) Qt.callLater(() => modeChoice.forceActiveFocus())
    Keys.onEscapePressed: event => {
        Today.optionsOpen = false;
        event.accepted = true;
    }

    // The panel's own colour, so the day doesn't show through
    Rectangle {
        anchors.fill: parent
        radius: root.mat ? 28 : Theme.radius.panel + 8 // (TodayScreen's panel)
        color: root.mat ? Qt.alpha(Theme.m3.surfaceContainer, 1) : Theme.dark ? "#1C1C1F" : "#F5F5F7"
    }
    MouseArea {
        anchors.fill: parent // (clicks stay on the sheet)
    }

    component Label: Text {
        width: parent?.width ?? 0
        wrapMode: Text.Wrap
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
    }
    component Hint: Text {
        width: parent?.width ?? 0
        wrapMode: Text.Wrap
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Flickable {
        anchors { fill: parent; margins: 32; bottomMargin: 32 + done.height + 16 }
        contentHeight: list.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Column {
            id: list
            width: parent.width
            spacing: 20

            Text {
                text: "Today options"
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: root.mat ? 22 : Theme.font.large; weight: root.mat ? Font.Normal : Font.DemiBold }
            }

            Column {
                width: parent.width
                spacing: 8
                Label { text: "Show Today" }
                Segmented {
                    id: modeChoice
                    width: Math.min(parent.width, 400)
                    accessibleName: "Show Today"
                    readonly property var values: ["ask", "widget", "both", "off"]
                    options: ["Every morning", "Widget", "Both", "Off"]
                    current: Math.max(0, values.indexOf(Today.mode))
                    onSelected: i => root.cfg.mode = values[i]
                }
                Hint {
                    text: Today.mode === "ask" ? "Opens on your first unlock after 4 AM. Close it and it's gone until tomorrow; Pin for today keeps it on the desktop until the day ends."
                        : Today.mode === "widget" ? "No morning screen. The Today widget stays on your desktop; remove it and it comes back tomorrow."
                        : Today.mode === "both" ? "Opens on your first unlock after 4 AM, and the Today widget stays on your desktop."
                        : "Today doesn't open by itself. Spotlight's Show Today opens it, and the Today widget is in the widget gallery."
                }
            }

            Column {
                width: parent.width
                spacing: 8
                Label { text: "Entrance" }
                Segmented {
                    width: Math.min(parent.width, 320)
                    accessibleName: "Entrance"
                    readonly property var values: ["bloom", "shrink", "drop"]
                    options: ["Bloom", "Shrink", "Drop"]
                    current: Math.max(0, values.indexOf(root.cfg.entrance))
                    onSelected: i => root.cfg.entrance = values[i]
                }
                Hint {
                    text: root.cfg.entrance === "shrink" ? "A full-screen sheet draws in to the panel" : root.cfg.entrance === "drop" ? "A drop falls from the top and spreads into the panel" : "The panel settles out of the frosted screen"
                }
            }

            Column {
                width: parent.width
                spacing: 8
                Label { text: "Your name" }
                TextField {
                    id: nameField
                    width: Math.min(parent.width, 320)
                    accessibleName: "Your name"
                    placeholder: Today.accountName.split(/\s+/)[0] || "Your name"
                    text: root.cfg.name
                    onCommitted: t => root.cfg.name = t.trim()
                }
                Hint {
                    text: "What the greeting calls you. Empty: " + (Today.accountName ? "\"" + Today.accountName.split(/\s+/)[0] + "\", from your account" : "no name")
                }
            }

            Item {
                width: parent.width
                height: Math.max(chimeText.implicitHeight, chime.height)
                Column {
                    id: chimeText
                    anchors { left: parent.left; right: chime.left; rightMargin: 16; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Label { text: "Chime" }
                    Hint { text: "A sound as it opens, from your sound style" }
                }
                Switch {
                    id: chime
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    checked: Config.options.sounds.today
                    onToggled: Config.options.sounds.today = !checked
                }
            }
        }
    }

    // Done: back to the day (a name being typed is kept)
    Rectangle {
        id: done
        anchors { right: parent.right; bottom: parent.bottom; margins: 32 }
        implicitWidth: doneLabel.implicitWidth + (root.mat ? 48 : 32)
        implicitHeight: root.mat ? 40 : 36
        radius: root.mat ? height / 2 : Theme.radius.control
        color: root.mat ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, doneMouse.pressed ? 0.1 : doneMouse.containsMouse ? 0.08 : 0))
            : doneMouse.pressed ? Qt.darker(Theme.accent, 1.15) : doneMouse.containsMouse ? Qt.lighter(Theme.accent, 1.08) : Theme.accent
        activeFocusOnTab: true
        function activate() {
            if (nameField.editing)
                nameField.committed(nameField.text);
            Today.optionsOpen = false;
        }
        Keys.onReturnPressed: done.activate()
        Keys.onEnterPressed: done.activate()
        Keys.onSpacePressed: done.activate()
        Accessible.role: Accessible.Button
        Accessible.name: "Done"
        Accessible.onPressAction: done.activate()
        FocusRing {
            radius: done.radius + 4
        }
        Text {
            id: doneLabel
            anchors.centerIn: parent
            text: "Done"
            color: root.mat ? Theme.m3.onPrimary : Theme.color.onAccent
            font { family: Theme.font.family; pixelSize: root.mat ? 14 : Theme.font.small; weight: root.mat ? Font.Medium : Font.DemiBold }
        }
        MouseArea {
            id: doneMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: done.activate()
        }
    }
}
