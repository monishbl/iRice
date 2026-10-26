import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Spotlight: a floating glass search field; results appear in a second glass
// panel below as you type. Up/Down to move, Enter to open, Esc to close.
// Prefixes: ">" command, ";" clipboard, ":" emoji, "/" files.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "spotlight"
        screen: modelData

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "spotlight"

            readonly property bool shown: Panels.isOpen("spotlight", screen)
            readonly property int panelWidth: 680
            readonly property int m: 24 // glass shadow margin
            property var results: []
            property int selected: 0
            property bool answering: false // showing an AI answer instead of results

            // Starts at 0 so the window animates in when it is created
            property real progress: 0
            Component.onCompleted: {
                progress = Qt.binding(() => shown ? 1 : 0);
                if (shown)
                    reset();
            }
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: win.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            onShownChanged: if (shown) reset()
            function reset() {
                answering = false;
                Ai.stopQuick();
                field.text = "";
                results = [];
                selected = 0;
                field.forceActiveFocus();
                Search.refreshClipboard();
            }

            // File results arrive asynchronously
            Connections {
                target: Search
                function onFilesChanged() {
                    if (win.shown && field.text.startsWith("/"))
                        win.results = Search.query(field.text);
                }
            }

            function run(i) {
                const r = results[i];
                if (!r)
                    return;
                if (r.ai) {
                    answering = true;
                    r.run();
                    return;
                }
                Panels.close();
                r.run();
            }
            function continueInChat() {
                Ai.continueQuick();
            }

            // Click outside closes
            MouseArea {
                anchors.fill: parent
                onClicked: Panels.close()
            }

            Item {
                id: column
                x: (parent.width - width) / 2
                y: Math.round(parent.height * 0.2) - win.m + (1 - win.progress) * -10
                width: win.panelWidth + 2 * win.m
                height: parent.height
                opacity: win.progress
                onYChanged: {
                    fieldGlass.updatePosition();
                    resultsGlass.updatePosition();
                }

                Glass {
                    id: fieldGlass
                    width: parent.width
                    height: 56 + 2 * win.m
                    radius: 28
                    bevel: 22
                    shadowMargin: win.m
                }

                Icon {
                    x: win.m + 20
                    y: win.m + (56 - height) / 2
                    name: "magnifying-glass"
                    bold: true
                    size: 22
                    color: Theme.color.secondary
                }

                TextInput {
                    id: field
                    x: win.m + 56
                    y: win.m
                    width: win.panelWidth - 56 - 20
                    height: 56
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.color.label
                    selectionColor: Qt.alpha(Theme.accent, 0.5)
                    font { family: Theme.font.family; pixelSize: 22; weight: Font.Normal }
                    clip: true

                    onTextChanged: {
                        if (win.answering) {
                            win.answering = false;
                            Ai.stopQuick();
                        }
                        win.results = Search.query(text);
                        win.selected = 0;
                    }
                    Keys.onEscapePressed: Panels.close()
                    Keys.onReturnPressed: win.answering ? win.continueInChat() : win.run(win.selected)
                    Keys.onEnterPressed: win.answering ? win.continueInChat() : win.run(win.selected)
                    Keys.onDownPressed: win.selected = Math.min(win.results.length - 1, win.selected + 1)
                    Keys.onUpPressed: win.selected = Math.max(0, win.selected - 1)
                    Keys.onTabPressed: win.selected = (win.selected + 1) % Math.max(1, win.results.length)

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: field.text === ""
                        text: "Spotlight Search"
                        color: Theme.color.tertiary
                        font: field.font
                    }
                }

                Glass {
                    id: resultsGlass
                    y: fieldGlass.height - 2 * win.m + 8
                    visible: win.results.length > 0 && !win.answering
                    width: parent.width
                    height: Math.min(list.contentHeight, 440) + 16 + 2 * win.m
                    radius: 24
                    bevel: 18
                    shadowMargin: win.m

                    // Swallow clicks so they don't close Spotlight
                    MouseArea { anchors.fill: parent.shape }

                    ListView {
                        id: list
                        x: win.m + 8
                        y: win.m + 8
                        width: win.panelWidth - 16
                        height: parent.height - 2 * win.m - 16
                        clip: true
                        model: win.results
                        currentIndex: win.selected
                        boundsBehavior: Flickable.StopAtBounds
                        highlightMoveDuration: 0
                        delegate: Column {
                            required property var modelData
                            required property int index
                            width: list.width

                            Text {
                                visible: parent.index === 0 || win.results[parent.index - 1].section !== parent.modelData.section
                                leftPadding: 10
                                topPadding: parent.index === 0 ? 2 : 10
                                bottomPadding: 4
                                text: parent.modelData.section
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                            }
                            ResultRow {
                                width: parent.width
                                result: parent.modelData
                                selected: parent.index === win.selected
                                onActivated: win.run(parent.index)
                            }
                        }
                    }
                }

                // Inline AI answer
                Glass {
                    id: answerGlass
                    y: fieldGlass.height - 2 * win.m + 8
                    visible: win.answering
                    width: parent.width
                    height: answerBody.implicitHeight + 32 + 2 * win.m
                    radius: 24
                    bevel: 18
                    shadowMargin: win.m
                    onHeightChanged: updatePosition()

                    MouseArea { anchors.fill: parent.shape }

                    Column {
                        id: answerBody
                        x: win.m + 20
                        y: win.m + 16
                        width: win.panelWidth - 40
                        spacing: 12

                        Row {
                            spacing: 8
                            Icon {
                                anchors.verticalCenter: parent.verticalCenter
                                name: "sparkle"
                                fill: true
                                size: 14
                                color: Theme.accent
                                SequentialAnimation on opacity {
                                    running: Ai.quick.busy && Ai.quick.answer === ""
                                    loops: Animation.Infinite
                                    NumberAnimation { to: 0.35; duration: 600 }
                                    NumberAnimation { to: 1; duration: 600 }
                                }
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: Ai.quick.busy && Ai.quick.answer === "" ? "Thinking…" : Ai.model.name
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                            }
                        }

                        Flickable {
                            width: parent.width
                            height: Math.min(answerText.implicitHeight, 360)
                            contentHeight: answerText.implicitHeight
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds
                            visible: Ai.quick.answer !== "" || Ai.quick.error !== ""
                            TextEdit {
                                id: answerText
                                width: parent.width
                                readOnly: true
                                selectByMouse: true
                                wrapMode: TextEdit.Wrap
                                textFormat: Ai.quick.error !== "" ? TextEdit.PlainText : TextEdit.MarkdownText
                                text: Ai.quick.error !== "" ? Ai.quick.error : Ai.quick.answer
                                color: Ai.quick.error !== "" ? Theme.color.danger : Theme.color.label
                                selectionColor: Qt.alpha(Theme.accent, 0.4)
                                font { family: Theme.font.family; pixelSize: Theme.font.title }
                            }
                        }

                        Row {
                            visible: !Ai.quick.busy && Ai.quick.answer !== "" && Ai.quick.error === ""
                            spacing: 8
                            Repeater {
                                model: [
                                    { label: "Continue in chat", primary: true, run: () => win.continueInChat() },
                                    { label: "Copy answer", primary: false, run: () => { Quickshell.execDetached(["wl-copy", Ai.quick.answer]); Panels.close(); } }
                                ]
                                Rectangle {
                                    required property var modelData
                                    width: label.implicitWidth + 24
                                    height: 28
                                    radius: Theme.radius.control - 2
                                    color: modelData.primary ? (btnMouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent) : (btnMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill)
                                    Text {
                                        id: label
                                        anchors.centerIn: parent
                                        text: parent.modelData.label
                                        color: parent.modelData.primary ? Theme.color.onAccent : Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                                    }
                                    MouseArea {
                                        id: btnMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: parent.modelData.run()
                                    }
                                }
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                leftPadding: 4
                                text: "↵ to continue in chat"
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }
                    }
                }
            }
        }
    }
}
