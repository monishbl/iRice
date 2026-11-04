import QtQuick
import Quickshell
import qs.common
import qs.services

// Settings → Lock & Login Screen → Boot animation: the system's own, Tessera's
// designs (a still of each, made with them: assets/plymouth/thumbnails) and
// none, as cards; picking one sets it (BootSplash: the password, then about
// a minute while the boot image is rebuilt). Both looks.
Item {
    id: root
    width: parent?.width ?? 0
    height: cards.implicitHeight + 2 * pad
    readonly property int pad: Theme.material ? 20 : 14
    readonly property var choices: [{ id: "system", name: "System (" + SystemInfo.osName + "'s)" }]
        .concat(BootSplash.designs.map(d => ({ id: d.id, name: d.name })))
        .concat([{ id: "none", name: "None (text)" }])

    Flow {
        id: cards
        x: root.pad
        y: root.pad
        width: root.width - 2 * root.pad
        spacing: 12

        Repeater {
            model: root.choices
            Item {
                id: card
                required property var modelData
                readonly property string choice: modelData.id
                readonly property bool isChosen: (BootSplash.busy ? BootSplash.pending : BootSplash.chosen) === choice
                readonly property bool usable: BootSplash.blocked === "" && !BootSplash.busy
                width: 136
                height: thumb.height + 8 + name.implicitHeight
                opacity: usable || isChosen ? 1 : 0.45
                activeFocusOnTab: true
                Accessible.role: Accessible.RadioButton
                Accessible.name: modelData.name
                Accessible.checked: isChosen
                Keys.onSpacePressed: BootSplash.set(choice)
                Keys.onReturnPressed: BootSplash.set(choice)

                Rectangle {
                    id: thumb
                    width: parent.width
                    height: Math.round(width * 0.625)
                    radius: Theme.material ? 16 : Theme.radius.control
                    color: "#0A0A0B" // (the boot screen's own)
                    clip: true
                    border.width: 1
                    border.color: Theme.color.separator
                    Image {
                        anchors.fill: parent
                        anchors.margins: 1
                        visible: card.choice !== "system" && card.choice !== "none"
                        source: visible ? Quickshell.shellDir + "/assets/plymouth/thumbnails/" + card.choice + ".png" : ""
                        sourceSize: Qt.size(320, 200)
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }
                    // The system's: your distribution's mark
                    Image {
                        anchors.centerIn: parent
                        visible: card.choice === "system"
                        width: 36
                        height: 36
                        source: visible ? IconLookup.path(SystemInfo.osLogo || "distributor-logo", "computer") : ""
                        sourceSize: Qt.size(72, 72)
                        asynchronous: true
                    }
                    // None: Plymouth's text, as the boot messages scroll by
                    Text {
                        anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
                        visible: card.choice === "none"
                        text: "Starting…\n[  OK  ] Network\n[  OK  ] Bluetooth"
                        color: "#94949D"
                        lineHeight: 1.2
                        font { family: Theme.font.mono; pixelSize: 9 }
                    }
                    // (being set: the password, then the rebuild)
                    Rectangle {
                        anchors.fill: parent
                        visible: BootSplash.pending === card.choice
                        color: Qt.rgba(0, 0, 0, 0.5)
                        Spinner {
                            anchors.centerIn: parent
                            size: 24
                            color: "#EDEDEF"
                        }
                    }
                    Rectangle { // (hover)
                        anchors.fill: parent
                        radius: parent.radius
                        color: "white"
                        opacity: mouse.containsMouse && card.usable && !card.isChosen ? 0.06 : 0
                        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                    }
                }
                // The chosen one: a ring in the accent, outside the picture
                Rectangle {
                    anchors.fill: thumb
                    anchors.margins: Theme.material ? -5 : -4
                    radius: thumb.radius - anchors.margins
                    color: "transparent"
                    border.width: Theme.material ? 3 : 2
                    border.color: Theme.material ? Theme.m3.primary : Theme.accent
                    visible: card.isChosen
                }
                FocusRing { radius: thumb.radius + 4; anchors.bottomMargin: card.height - thumb.height - 4 }
                Text {
                    id: name
                    anchors { top: thumb.bottom; topMargin: 8; left: parent.left; right: parent.right }
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: card.modelData.name
                    color: card.isChosen ? Theme.color.label : Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: card.isChosen ? Font.Medium : Font.Normal }
                }
                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: card.usable ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: {
                        card.forceActiveFocus();
                        BootSplash.set(card.choice);
                    }
                }
            }
        }
    }
}
