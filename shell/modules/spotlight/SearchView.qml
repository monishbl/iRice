import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services
import "../../common/Markdown.js" as Markdown

// Spotlight in the Material look: Google-style search. A 56 px pill search
// bar drops into Material 3's docked search view (corner 28), the results in
// Expressive segmented lists grouped by section, the first one a hero card
// when it's the top hit or an answer. The same results, keys and actions as
// the Glass Spotlight (Spotlight.qml): Up/Down to move, Enter to open, Esc to
// close, Shift+Space for Quick Look; prefixes ">", ";", ":", "/", "?".
Variants {
    model: Theme.material ? Quickshell.screens : []

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
            readonly property int panelWidth: Math.min(720, (screen?.width ?? 1920) - 64)
            // The list just appeared: its rows come in one after another
            property bool entering: false
            property bool hadResults: false
            // A first result of these kinds is a hero card, not a row
            readonly property var heroSections: ["Top Hit", "Calculator", "Answer", "Conversion", "Dictionary", "Do"]

            // Opens on Material's default spatial spring, closes quickly
            property real progress: 0
            Component.onCompleted: {
                if (shown) {
                    openAnim.start();
                    search.reset();
                }
            }
            onShownChanged: {
                if (shown) {
                    closeAnim.stop();
                    openAnim.start();
                    search.reset();
                } else {
                    openAnim.stop();
                    closeAnim.start();
                }
            }
            SpringAnimation {
                id: openAnim
                target: win
                property: "progress"
                to: 1
                spring: Theme.motion.springDefault.spring
                damping: Theme.motion.springDefault.damping
                epsilon: 0.004
            }
            NumberAnimation {
                id: closeAnim
                target: win
                property: "progress"
                to: 0
                duration: Theme.motion.popover * 0.7
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Theme.motion.exit
            }

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            // The results, the chosen row and running it: SearchModel, as
            // Glass's Spotlight
            SearchModel {
                id: search
                searchField: field
                shown: win.shown
            }

            Connections {
                target: search
                function onResultsChanged() {
                    const has = search.results.length > 0;
                    if (has && !win.hadResults) {
                        win.entering = true;
                        enterTimer.restart();
                    }
                    win.hadResults = has;
                }
            }
            Timer {
                id: enterTimer
                interval: 450
                onTriggered: win.entering = false
            }

            // Material's scrim; a click on it closes
            Rectangle {
                anchors.fill: parent
                color: Theme.m3.scrim
                opacity: 0.32 * Math.max(0, Math.min(1, win.progress))
                MouseArea {
                    anchors.fill: parent
                    onClicked: Panels.close()
                }
            }

            Item {
                id: column
                width: win.panelWidth
                height: view.height
                x: Math.round((parent.width - width) / 2)
                y: Math.round(parent.height * 0.2)
                opacity: Math.max(0, Math.min(1, win.progress * 1.6))
                // Drops in from the menu bar and grows, on a spring
                transform: [
                    Scale {
                        origin.x: column.width / 2
                        origin.y: 0
                        xScale: 0.92 + 0.08 * win.progress
                        yScale: 0.92 + 0.08 * win.progress
                    },
                    Translate {
                        y: (1 - win.progress) * -32
                    }
                ]

                // The search view: the bar on top, the results dropping below
                Rectangle {
                    id: view
                    readonly property bool open: search.answering || search.results.length > 0
                    readonly property real bodyHeight: search.answering ? answerBody.implicitHeight + 28 : list.height + 20
                    width: parent.width
                    height: 56 + (open ? bodyHeight : 0)
                    Behavior on height { SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.5 } }
                    radius: 28
                    color: open ? Theme.m3.surfaceContainer : Theme.m3.surfaceContainerHigh
                    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

                    // Swallow clicks so they don't close the search
                    MouseArea { anchors.fill: parent }

                    // The search bar
                    Rectangle {
                        id: bar
                        width: parent.width
                        height: 56
                        radius: 28
                        color: Theme.m3.surfaceContainerHigh

                        Icon {
                            visible: !search.answering
                            x: 18
                            anchors.verticalCenter: parent.verticalCenter
                            name: "magnifying-glass"
                            size: 22
                            color: Theme.m3.onSurfaceVariant
                        }
                        // (on an AI answer: back to the results)
                        IconButton {
                            visible: search.answering
                            x: 8
                            anchors.verticalCenter: parent.verticalCenter
                            width: 40
                            height: 40
                            icon: "caret-left"
                            iconSize: 22
                            iconColor: Theme.m3.onSurface
                            tooltip: "Back to results"
                            onClicked: search.backToResults()
                        }

                        TextInput {
                            id: field
                            x: 56
                            width: trailing.x - x - 8
                            height: parent.height
                            verticalAlignment: TextInput.AlignVCenter
                            color: Theme.m3.onSurface
                            selectionColor: Theme.m3.primaryContainer
                            selectedTextColor: Theme.m3.onPrimaryContainer
                            font { family: Theme.font.family; pixelSize: 18; weight: Font.Normal }
                            clip: true
                            Accessible.role: Accessible.EditableText
                            Accessible.name: "Search"

                            onTextChanged: search.typed(text)
                            Keys.onEscapePressed: Panels.close()
                            Keys.onReturnPressed: search.accept()
                            Keys.onEnterPressed: search.accept()
                            Keys.onDownPressed: search.down()
                            Keys.onUpPressed: search.up()
                            Keys.onTabPressed: search.next()
                            Keys.onPressed: e => search.quickLookKey(e)

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width
                                visible: field.text === ""
                                text: "Search apps, files and the web"
                                elide: Text.ElideRight
                                color: Theme.m3.onSurfaceVariant
                                font: field.font
                            }
                        }

                        // Trailing: clear, and ask AI (round icon buttons)
                        Row {
                            id: trailing
                            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            spacing: 4
                            IconButton {
                                visible: field.text !== ""
                                width: 40
                                height: 40
                                icon: "x"
                                iconSize: 22
                                iconColor: Theme.m3.onSurfaceVariant
                                tooltip: "Clear search"
                                onClicked: {
                                    field.text = "";
                                    field.forceActiveFocus();
                                }
                            }
                            IconButton {
                                width: 40
                                height: 40
                                icon: "sparkle"
                                fill: search.answering
                                iconSize: 22
                                iconColor: search.answering ? Theme.m3.primary : Theme.m3.onSurfaceVariant
                                tooltip: Ai.configured ? "Ask " + Ai.model.name.replace(/ \(.*\)$/, "") : "Set up AI"
                                onClicked: search.askAi()
                            }
                        }
                    }

                    // Below the bar: results or the AI answer (clipped while
                    // the view springs open)
                    Item {
                        id: body
                        y: 56
                        width: parent.width
                        height: Math.max(0, view.height - 56)
                        visible: height > 1
                        clip: true

                        ListView {
                            id: list

                            Momentum {}
                            x: 12
                            y: 8
                            width: parent.width - 24
                            height: Math.min(contentHeight, 480)
                            visible: !search.answering
                            clip: true
                            model: search.results
                            currentIndex: search.selected
                            boundsBehavior: Flickable.StopAtBounds
                            highlightMoveDuration: 0

                            delegate: Item {
                                id: entry
                                required property var modelData
                                required property int index
                                width: list.width
                                height: entryColumn.height

                                readonly property var prev: index > 0 ? search.results[index - 1] : null
                                readonly property var next: search.results[index + 1] ?? null
                                readonly property bool hero: index === 0 && win.heroSections.includes(modelData.section)
                                // A section's rows form one group; a row after the hero starts its own
                                readonly property bool groupStart: index === 0 || prev.section !== modelData.section || (index === 1 && win.heroSections.includes(prev.section))
                                readonly property bool groupEnd: !next || next.section !== modelData.section
                                readonly property bool labelled: !hero && (index === 0 || prev.section !== modelData.section)

                                // Comes in on a small spring, a little after the one above
                                property real intro: 0
                                property bool introArmed: false
                                Behavior on intro { enabled: entry.introArmed; SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.01 } }
                                Component.onCompleted: {
                                    if (!win.entering || index > 10)
                                        return;
                                    intro = 1;
                                    introArmed = true;
                                    introDelay.interval = 16 + index * 28;
                                    introDelay.start();
                                }
                                Timer {
                                    id: introDelay
                                    onTriggered: entry.intro = 0
                                }
                                opacity: Math.max(0, Math.min(1, 1 - entry.intro))
                                transform: Translate { y: entry.intro * 14 }

                                Column {
                                    id: entryColumn
                                    width: parent.width

                                    // (space between groups)
                                    Item {
                                        visible: entry.groupStart && entry.index > 0
                                        width: 1
                                        height: 12
                                    }
                                    Text {
                                        visible: entry.labelled
                                        width: parent.width
                                        leftPadding: 16
                                        topPadding: entry.index === 0 ? 4 : 0
                                        bottomPadding: 8
                                        text: entry.modelData.section === "Top Hit" ? "Top hit" : entry.modelData.section
                                        elide: Text.ElideRight
                                        color: Theme.m3.primary
                                        // (Material's label large)
                                        font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; letterSpacing: 0.1 }
                                    }
                                    SearchHero {
                                        visible: entry.hero
                                        width: parent.width
                                        result: entry.modelData
                                        selected: entry.index === search.selected
                                        onActivated: search.run(entry.index)
                                    }
                                    SearchRow {
                                        visible: !entry.hero
                                        width: parent.width
                                        result: entry.modelData
                                        selected: entry.index === search.selected
                                        firstInGroup: entry.groupStart
                                        lastInGroup: entry.groupEnd
                                        onActivated: search.run(entry.index)
                                    }
                                    // (rows of a group sit 2 apart)
                                    Item {
                                        visible: !!entry.next
                                        width: 1
                                        height: 2
                                    }
                                }
                            }
                        }

                        // Inline AI answer
                        Column {
                            id: answerBody
                            visible: search.answering
                            x: 20
                            y: 12
                            width: parent.width - 40
                            spacing: 16

                            Row {
                                spacing: 12
                                Item {
                                    width: 36
                                    height: 36
                                    readonly property bool thinking: Ai.quick.busy && Ai.quick.answer === ""
                                    Spinner {
                                        visible: parent.thinking
                                        anchors.centerIn: parent
                                        size: 36
                                    }
                                    MaterialShape {
                                        id: aiShape
                                        visible: !parent.thinking
                                        anchors.fill: parent
                                        shape: "sunny"
                                        color: Theme.m3.primaryContainer
                                    }
                                    Icon {
                                        visible: !parent.thinking
                                        anchors.centerIn: parent
                                        name: "sparkle"
                                        fill: true
                                        size: 18
                                        color: Theme.m3.onPrimaryContainer
                                    }
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: Ai.quick.busy && Ai.quick.answer === "" ? "Thinking…" : Ai.model.name
                                    color: Theme.m3.onSurfaceVariant
                                    font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; letterSpacing: 0.1 }
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
                                    color: Ai.quick.error !== "" ? Theme.m3.error : Theme.m3.onSurface
                                    selectionColor: Theme.m3.primaryContainer
                                    selectedTextColor: Theme.m3.onPrimaryContainer
                                    font { family: Theme.font.family; pixelSize: 16 }
                                }
                            }

                            Row {
                                visible: !Ai.quick.busy && Ai.quick.answer !== "" && Ai.quick.error === ""
                                spacing: 8
                                SearchButton {
                                    text: "Continue in chat"
                                    filled: true
                                    onClicked: search.continueInChat()
                                }
                                SearchButton {
                                    text: "Copy answer"
                                    onClicked: {
                                        Quickshell.execDetached(["wl-copy", Ai.quick.answer]);
                                        Panels.close();
                                    }
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    leftPadding: 8
                                    text: "Enter continues in chat"
                                    color: Theme.m3.onSurfaceVariant
                                    font { family: Theme.font.family; pixelSize: 13 }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
