import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services
import "../../common/Markdown.js" as Markdown

// Spotlight: a floating glass search field; results appear in a second glass
// panel below as you type. Up/Down to move, Enter to open, Esc to close.
// Prefixes: ">" command, ";" clipboard, ":" emoji, "/" files. At the field's
// end: Clear (while there's text) and Ask AI, reached with Shift+Tab (Tab
// moves through the results).
Variants {
    model: Theme.material ? [] : Quickshell.screens

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

            // Starts at 0 so the window animates in when it is created
            property real progress: 0
            Component.onCompleted: {
                progress = Qt.binding(() => shown ? 1 : 0);
                if (shown)
                    search.reset();
            }
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: win.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            onShownChanged: if (shown) search.reset()
            // The results, the chosen row and running it: SearchModel
            SearchModel {
                id: search
                searchField: field
                shown: win.shown
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
                    width: trailing.x - x - 8
                    height: 56
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.color.label
                    selectionColor: Theme.accentAlpha(0.5)
                    font { family: Theme.font.family; pixelSize: 22; weight: Font.Normal }
                    clip: true
                    Accessible.role: Accessible.EditableText
                    Accessible.name: "Search"

                    onTextChanged: search.typed(text)
                    Keys.onEscapePressed: Panels.close()
                    Keys.onReturnPressed: search.accept()
                    Keys.onEnterPressed: search.accept()
                    Keys.onDownPressed: search.down()
                    Keys.onUpPressed: search.up()
                    // Tab moves through the results (to the buttons when
                    // there are none to move through); Shift+Tab, the buttons
                    Keys.onTabPressed: e => {
                        if (e.modifiers & Qt.ShiftModifier)
                            askButton.forceActiveFocus(Qt.BacktabFocusReason);
                        else if (search.results.length > 1)
                            search.next();
                        else
                            (clearButton.visible ? clearButton : askButton).forceActiveFocus(Qt.TabFocusReason);
                    }
                    Keys.onBacktabPressed: askButton.forceActiveFocus(Qt.BacktabFocusReason)
                    Keys.onPressed: e => search.quickLookKey(e)

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: field.text === ""
                        text: "Search"
                        color: Theme.color.tertiary
                        font: field.font
                    }
                }

                // At the end: Clear (while there's text) and Ask AI, small
                // and grey until pointed at or focused
                Row {
                    id: trailing
                    x: win.m + win.panelWidth - width - 10
                    y: win.m + (56 - height) / 2
                    spacing: 2
                    Keys.onEscapePressed: Panels.close()
                    IconButton {
                        id: clearButton
                        visible: field.text !== ""
                        width: 36
                        height: 36
                        icon: "x-circle"
                        fill: true
                        iconSize: 18
                        iconColor: hovered || activeFocus ? Theme.color.secondary : Theme.color.tertiary
                        tooltip: "Clear search"
                        KeyNavigation.tab: askButton
                        KeyNavigation.backtab: field
                        onClicked: {
                            field.text = "";
                            field.forceActiveFocus();
                        }
                    }
                    IconButton {
                        id: askButton
                        width: 36
                        height: 36
                        icon: "sparkle"
                        fill: search.answering
                        iconSize: 18
                        iconColor: search.answering ? Theme.accentInk : hovered || activeFocus ? Theme.color.secondary : Theme.color.tertiary
                        tooltip: Ai.configured ? "Ask " + Ai.model.name.replace(/ \(.*\)$/, "") : "Set up AI"
                        KeyNavigation.tab: field
                        KeyNavigation.backtab: clearButton.visible ? clearButton : field
                        onClicked: search.askAi()
                    }
                }

                Glass {
                    id: resultsGlass
                    y: fieldGlass.height - 2 * win.m + 8
                    visible: search.results.length > 0 && !search.answering
                    width: parent.width
                    height: Math.min(list.contentHeight, 440) + 16 + 2 * win.m
                    radius: 24
                    bevel: 18
                    shadowMargin: win.m

                    // Swallow clicks so they don't close Spotlight
                    MouseArea { anchors.fill: parent.shape }

                    ListView {
                        id: list

                        Momentum {}
                        x: win.m + 8
                        y: win.m + 8
                        width: win.panelWidth - 16
                        height: parent.height - 2 * win.m - 16
                        clip: true
                        model: search.results
                        currentIndex: search.selected
                        boundsBehavior: Flickable.StopAtBounds
                        highlightMoveDuration: 0
                        delegate: Column {
                            required property var modelData
                            required property int index
                            width: list.width

                            // (an answer or a thing to do, on top: a big card, no heading)
                            readonly property bool big: index === 0 && ["Calculator", "Answer", "Conversion", "Dictionary", "Do"].includes(modelData.section)
                            AnswerCard {
                                visible: parent.big
                                width: parent.width
                                result: parent.modelData
                                selected: parent.index === search.selected
                                onActivated: search.run(parent.index)
                            }
                            Item {
                                visible: parent.big
                                width: 1
                                height: 6
                            }
                            Text {
                                visible: !parent.big && (parent.index === 0 || search.results[parent.index - 1].section !== parent.modelData.section)
                                leftPadding: 10
                                topPadding: parent.index === 0 ? 2 : 10
                                bottomPadding: 4
                                text: parent.modelData.section
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                            }
                            ResultRow {
                                visible: !parent.big
                                width: parent.width
                                result: parent.modelData
                                selected: parent.index === search.selected
                                onActivated: search.run(parent.index)
                            }
                        }
                    }
                }

                // Inline AI answer
                Glass {
                    id: answerGlass
                    y: fieldGlass.height - 2 * win.m + 8
                    visible: search.answering
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
                                color: Theme.accentInk
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

                            Momentum {}
                            width: parent.width
                            height: Math.min(answerText.implicitHeight, 360)
                            contentHeight: answerText.implicitHeight
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds
                            visible: Ai.quick.answer !== "" || Ai.quick.error !== ""
                            TextEdit {
                                id: answerText
                                onLinkActivated: link => Launch.openLink(link)
                                width: parent.width
                                readOnly: true
                                selectByMouse: true
                                wrapMode: TextEdit.Wrap
                                textFormat: Ai.quick.error !== "" ? TextEdit.PlainText : TextEdit.RichText
                                text: Ai.quick.error !== "" ? Ai.quick.error : Markdown.html(Ai.quick.answer, Theme.markdown) // (escaped; links http(s) and mailto only)
                                color: Ai.quick.error !== "" ? Theme.color.danger : Theme.color.label
                                selectionColor: Theme.accentAlpha(0.4)
                                font { family: Theme.font.family; pixelSize: Theme.font.title }
                            }
                        }

                        Row {
                            visible: !Ai.quick.busy && Ai.quick.answer !== "" && Ai.quick.error === ""
                            spacing: 8
                            Repeater {
                                model: [
                                    { label: "Continue in chat", primary: true, run: () => search.continueInChat() },
                                    { label: "Copy answer", primary: false, run: () => { Quickshell.execDetached(["wl-copy", Ai.quick.answer]); Panels.close(); } }
                                ]
                                Squircle {
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
