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
            contentHeight: 300
            padding: 16

            // The draft survives closing (Escape), not saving
            property string draft: ""
            onShownChanged: if (shown) {
                editor.text = draft;
                editor.cursorPosition = editor.length;
                editor.forceActiveFocus();
            } else {
                draft = editor.text;
            }
            function save() {
                if (editor.text.trim() === "")
                    return;
                Notes.create(editor.text, false);
                editor.text = "";
                draft = "";
                Panels.close();
            }

            Column {
                width: parent.width
                spacing: 12

                Row {
                    width: parent.width
                    spacing: 8
                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: "note-pencil"
                        size: 18
                        color: Theme.accents.yellow
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Quick note"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                    }
                }

                Rectangle {
                    width: parent.width
                    height: 196
                    radius: Theme.radius.control
                    color: Theme.color.fill
                    border.width: editor.activeFocus ? 2 : 0
                    border.color: Theme.accent
                    Flickable {
                        id: flick
                        anchors.fill: parent
                        anchors.margins: 10
                        contentHeight: editor.height
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        TextEdit {
                            id: editor
                            width: flick.width
                            height: Math.max(flick.height, contentHeight)
                            wrapMode: TextEdit.Wrap
                            color: Theme.color.label
                            selectionColor: Qt.alpha(Theme.accent, 0.35)
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
                    height: 32
                    Text {
                        anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                        text: "Saved to " + Notes.folder.replace(Quickshell.env("HOME"), "~")
                        color: Theme.color.tertiary
                        elide: Text.ElideMiddle
                        width: parent.width - saveButton.width - 12
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                    Rectangle {
                        id: saveButton
                        anchors.right: parent.right
                        width: saveLabel.implicitWidth + 28
                        height: 32
                        radius: Theme.radius.control
                        readonly property bool enabledNow: editor.text.trim() !== ""
                        color: !enabledNow ? Theme.color.fill : saveMouse.pressed ? Qt.darker(Theme.accent, 1.15) : saveMouse.containsMouse ? Qt.lighter(Theme.accent, 1.08) : Theme.accent
                        scale: saveMouse.pressed ? 0.97 : 1
                        Behavior on scale { NumberAnimation { duration: 100 } }
                        Text {
                            id: saveLabel
                            anchors.centerIn: parent
                            text: "Save note  ⌃↵"
                            color: saveButton.enabledNow ? "white" : Theme.color.tertiary
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
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
