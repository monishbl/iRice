import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// Power options. Arrow keys move, Enter confirms, Esc cancels.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "session"
        screen: modelData

        Dialog {
            id: dialog
            readonly property var modelData: slot.modelData
            screen: modelData
            dialogName: "session"
            // (Material: bigger shaped buttons, 8 apart, and a dialog's 24 padding)
            contentWidth: Theme.material ? 5 * 96 + 4 * 8 : 5 * 80 + 4 * 12
            padding: Theme.material ? 24 : 20

            property int current: 4
            readonly property bool keyboardConfirmation: Panels.sessionAction !== ""
            onShownChanged: if (shown) resetSelection()
            Component.onCompleted: if (shown) resetSelection()

            // (shape and roles: the Material look's big tonal shaped buttons,
            // as Android 16's power menu; the shape it morphs into when chosen)
            readonly property var options: [
                { id: "lock", text: "Lock", icon: "lock", shape: "cookie4", role: "primary" },
                { id: "sleep", text: "Sleep", icon: "moon", shape: "cookie9", role: "secondary" },
                { id: "logout", text: "Log Out", icon: "sign-out", shape: "clover4", role: "secondary" },
                { id: "restart", text: "Restart", icon: "arrow-clockwise", shape: "cookie12", role: "tertiary" },
                { id: "shutdown", text: "Shut Down", icon: "power", shape: "softBurst", role: "error" }
            ]

            function resetSelection() {
                const requested = options.findIndex(opt => opt.id === Panels.sessionAction);
                current = requested >= 0 ? requested : 4;
                actions.forceActiveFocus();
            }

            function choose(i) {
                if (!shown || i < 0 || i >= options.length)
                    return;
                const opt = options[i];
                Panels.close();
                System.power(opt.id);
            }

            Column {
                width: dialog.contentWidth
                spacing: 20

                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: "What would you like to do?"
                    color: Theme.color.label
                    // (Material: the dialog's headline, headline small)
                    font { family: Theme.font.family; pixelSize: Theme.material ? 24 : Theme.font.large; weight: Theme.material ? Font.Normal : Font.DemiBold }
                }

                Row {
                    id: actions
                    spacing: Theme.material ? 8 : 12
                    focus: dialog.shown
                    Keys.onLeftPressed: dialog.current = Math.max(0, dialog.current - 1)
                    Keys.onRightPressed: dialog.current = Math.min(dialog.options.length - 1, dialog.current + 1)
                    Keys.onUpPressed: dialog.current = Math.max(0, dialog.current - 1)
                    Keys.onDownPressed: dialog.current = Math.min(dialog.options.length - 1, dialog.current + 1)
                    // Holding Enter in Spotlight must never confirm the action.
                    Keys.onReturnPressed: event => { if (!event.isAutoRepeat) dialog.choose(dialog.current); }
                    Keys.onEnterPressed: event => { if (!event.isAutoRepeat) dialog.choose(dialog.current); }
                    Keys.onEscapePressed: Panels.close()

                    Repeater {
                        model: dialog.options

                        Column {
                            id: option
                            required property var modelData
                            required property int index
                            readonly property bool hot: dialog.current === index
                            // Only the chosen option stands out. Material:
                            // it fills with primary (Restart a blue, Shut Down
                            // error); the others share one quiet fill a tone
                            // above the dialog (surfaceContainerHighest, so
                            // their shapes still show), each keeping its own
                            // shape, icons onSurfaceVariant (Restart's a faint
                            // blue, Shut Down's a faint red). Glass: the chosen
                            // one in the accent (Restart blue, Shut Down red),
                            // the others the neutral off fill, their icons
                            // the label colour (Restart's and Shut Down's
                            // softly tinted). A state layer for hover and press.
                            readonly property string kind: modelData.id
                            readonly property color mBlue: Theme.dark ? "#9ecaff" : "#2f6aa8"
                            readonly property color mOnBlue: Theme.dark ? "#003258" : "#ffffff"
                            readonly property color mContainer: !Theme.material ? "transparent"
                                : !hot ? Theme.m3.surfaceContainerHighest
                                : kind === "restart" ? mBlue : kind === "shutdown" ? Theme.m3.error : Theme.m3.primary
                            readonly property color mInk: !Theme.material ? "transparent"
                                : hot ? (kind === "restart" ? mOnBlue : kind === "shutdown" ? Theme.m3.onError : Theme.m3.onPrimary)
                                : kind === "restart" ? Qt.tint(Theme.m3.onSurfaceVariant, Qt.alpha(mBlue, 0.45))
                                : kind === "shutdown" ? Qt.tint(Theme.m3.onSurfaceVariant, Qt.alpha(Theme.m3.error, 0.5))
                                : Theme.m3.onSurfaceVariant
                            readonly property color gBlue: Theme.accents.blue
                            readonly property color gFill: !hot ? Theme.color.toggleOff
                                : kind === "restart" ? gBlue : kind === "shutdown" ? Theme.color.danger : Theme.accent
                            readonly property color gInk: hot ? (kind === "restart" || kind === "shutdown" ? "white" : Theme.color.onAccent)
                                : kind === "restart" ? Qt.tint(Theme.color.label, Qt.alpha(gBlue, 0.5))
                                : kind === "shutdown" ? Qt.tint(Theme.color.label, Qt.alpha(Theme.color.danger, 0.5))
                                : Theme.color.label
                            width: Theme.material ? 96 : 80
                            spacing: Theme.material ? 12 : 8

                            Rectangle {
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: Theme.material ? 88 : 64
                                height: width
                                radius: width / 2
                                color: Theme.material ? "transparent" : option.gFill
                                // (Material's press swell; Glass shrinks by scale instead)
                                transform: Theme.material ? pressFx1.effects : []
                                ElasticControl { id: pressFx1; enabled: Theme.material; pressed: Theme.material && mouse.pressed }
                                scale: Theme.material ? 1 : mouse.pressed ? 0.94 : 1
                                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }

                                Loader {
                                    anchors.fill: parent
                                    active: Theme.material
                                    sourceComponent: MaterialShape {
                                        shape: mouse.pressed ? "square" : option.modelData.shape
                                        color: Qt.tint(option.mContainer, Qt.alpha(option.mInk, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
                                        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                    }
                                }

                                Icon {
                                    anchors.centerIn: parent
                                    name: option.modelData.icon
                                    bold: !Theme.material
                                    fill: Theme.material && option.hot
                                    size: Theme.material ? 32 : 26
                                    color: Theme.material ? option.mInk : option.gInk
                                    Behavior on color {
                                        ColorAnimation { duration: Theme.motion.hover }
                                    }
                                }
                                MouseArea {
                                    id: mouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onEntered: if (!dialog.keyboardConfirmation) dialog.current = option.index
                                    // A click is a deliberate choice: it confirms (the
                                    // keyboard path asks for Enter, against a key
                                    // still held from Spotlight repeating into it)
                                    onClicked: dialog.choose(option.index)
                                }
                            }
                            Text {
                                width: parent.width
                                horizontalAlignment: Text.AlignHCenter
                                text: Theme.material ? option.modelData.text.charAt(0) + option.modelData.text.slice(1).toLowerCase() : option.modelData.text
                                color: option.hot ? Theme.color.label : Theme.color.secondary
                                // (Material: label large, emphasized when chosen)
                                font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material && option.hot ? Font.DemiBold : Font.Medium }
                            }
                        }
                    }
                }

                Text {
                    visible: dialog.keyboardConfirmation
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: "Click, or choose with the arrow keys and press Enter · Esc to cancel"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
            }
        }
    }
}
