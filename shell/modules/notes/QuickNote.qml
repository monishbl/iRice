import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// Quick note (Super+Alt+N, a hot corner, or Spotlight "note …"): type, then
// Ctrl+Enter saves it as a Markdown file in your Notes folder. Escape closes
// and keeps the draft for next time; an empty note isn't saved.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "quickNote"
        screen: modelData

        Dialog {
            id: dialog
            screen: slot.modelData
            dialogName: "quickNote"
            contentWidth: 440
            padding: Theme.material ? 24 : 16

            // The draft survives closing (Escape), not saving
            property string draft: ""
            function start() {
                editor.text = draft;
                editor.cursorPosition = editor.length;
                editor.forceActiveFocus();
            }
            // (the window is created already shown the first time)
            Component.onCompleted: if (shown) Qt.callLater(start)
            onShownChanged: if (shown) {
                start();
            } else {
                draft = editor.text;
            }
            property string savingPath: ""
            function save() {
                if (savingPath !== "" || editor.text.trim() === "")
                    return;
                savingPath = Notes.create(editor.text, false);
            }
            Connections {
                target: Notes
                function onSaved(path) {
                    if (path !== dialog.savingPath) return;
                    dialog.savingPath = "";
                    editor.text = "";
                    dialog.draft = "";
                    Panels.close();
                }
                function onSaveFailed(path) {
                    if (path === dialog.savingPath) dialog.savingPath = "";
                }
            }

            Column {
                width: parent.width
                spacing: 12

                Row {
                    width: parent.width
                    spacing: Theme.material ? 12 : 8
                    // (Material: the icon on a tertiary shape, and the
                    // dialog's headline)
                    Item {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.material ? 40 : noteIcon.width
                        height: Theme.material ? 40 : noteIcon.height
                        Loader {
                            anchors.fill: parent
                            active: Theme.material
                            sourceComponent: MaterialShape {
                                shape: "cookie9"
                                color: Theme.m3.tertiaryContainer
                            }
                        }
                        Icon {
                            id: noteIcon
                            anchors.centerIn: parent
                            name: "note-pencil"
                            fill: Theme.material
                            size: Theme.material ? 20 : 18
                            color: Theme.material ? Theme.m3.onTertiaryContainer : Theme.accents.yellow
                        }
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Quick note"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.large : Theme.font.body; weight: Theme.material ? Font.Normal : Font.DemiBold }
                    }
                }

                Squircle {
                    width: parent.width
                    height: 196
                    radius: Theme.material ? 16 : Theme.radius.control
                    color: Theme.color.fill
                    border.width: editor.activeFocus ? 2 : 0
                    border.color: Theme.accent
                    Flickable {
                        id: flick
                        Momentum {}
                        anchors.fill: parent
                        anchors.margins: Theme.material ? 16 : 10
                        contentHeight: editor.height
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        TextEdit {
                            id: editor
                            readOnly: dialog.savingPath !== ""
                            width: flick.width
                            height: Math.max(flick.height, contentHeight)
                            wrapMode: TextEdit.Wrap
                            color: Theme.color.label
                            selectionColor: Theme.accentAlpha(0.35)
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                            onCursorRectangleChanged: {
                                if (cursorRectangle.y < flick.contentY)
                                    flick.contentY = cursorRectangle.y;
                                else if (cursorRectangle.y + cursorRectangle.height > flick.contentY + flick.height)
                                    flick.contentY = cursorRectangle.y + cursorRectangle.height - flick.height;
                            }
                            Keys.onPressed: e => {
                                if ((e.key === Qt.Key_Return || e.key === Qt.Key_Enter) && (e.modifiers & Qt.ControlModifier)) {
                                    dialog.save();
                                    e.accepted = true;
                                } else if (e.key === Qt.Key_Escape) {
                                    Panels.close();
                                    e.accepted = true;
                                }
                            }
                        }
                        Text {
                            visible: editor.text === ""
                            text: "Write anything. The first line becomes the title."
                            color: Theme.color.tertiary
                            font: editor.font
                        }
                    }
                }

                Item {
                    width: parent.width
                    height: Theme.material ? 40 : 32
                    Text {
                        anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                        text: "Saved to " + Notes.folder.replace(Quickshell.env("HOME"), "~")
                        color: Theme.color.tertiary
                        elide: Text.ElideMiddle
                        width: parent.width - saveButton.width - 12
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                    Squircle {
                        id: saveButton
                        anchors.right: parent.right
                        // Material: a filled primary pill with state layers,
                        // its corners tightening while pressed; greyed out
                        // (12% / 38% of onSurface) with nothing to save
                        width: saveLabel.implicitWidth + (Theme.material ? 48 : 28)
                        height: Theme.material ? 40 : 32
                        radius: Theme.material ? (saveMouse.pressed ? 8 : height / 2) : Theme.radius.control
                        Behavior on radius {
                            enabled: Theme.material
                            SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.25 }
                        }
                        readonly property bool enabledNow: dialog.savingPath === "" && editor.text.trim() !== ""
                        color: Theme.material ? (!enabledNow ? Qt.alpha(Theme.m3.onSurface, 0.12) : Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, saveMouse.pressed ? 0.1 : saveMouse.containsMouse ? 0.08 : 0)))
                            : !enabledNow ? Theme.color.fill : saveMouse.pressed ? Qt.darker(Theme.accent, 1.15) : saveMouse.containsMouse ? Qt.lighter(Theme.accent, 1.08) : Theme.accent
                        scale: saveMouse.pressed && !Theme.material ? 0.97 : 1
                        Behavior on scale { ScaleAnimator { duration: 100 } }
                        Text {
                            id: saveLabel
                            anchors.centerIn: parent
                            text: dialog.savingPath !== "" ? "Saving…" : "Save note  ⌃↵"
                            color: Theme.material ? (saveButton.enabledNow ? Theme.m3.onPrimary : Qt.alpha(Theme.m3.onSurface, 0.38)) : saveButton.enabledNow ? "white" : Theme.color.tertiary
                            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
                        }
                        MouseArea {
                            id: saveMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            enabled: saveButton.enabledNow
                            cursorShape: Qt.PointingHandCursor
                            onClicked: dialog.save()
                        }
                    }
                }
            }
        }
    }
}
