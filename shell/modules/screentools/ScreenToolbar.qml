import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// ⇧⌘5-style toolbar. Super+Shift+5: every tool (capture screen / selection,
// copy text, translate, ask AI, record screen / window / selection).
// Control Center → Record: recording only, like macOS: pick Entire screen,
// Window or Selected region, then Record. Options (microphone, system audio,
// pointer, countdown, folder) open above the bar. Esc closes.
Variants {
    model: Quickshell.screens

    // ── Pieces of the options list ──────────────────────────────
    component Caption: Text {
        width: parent?.width ?? 0
        leftPadding: 8
        topPadding: 2
        bottomPadding: 4
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
    }
    component Gap: Item {
        width: 1
        height: 6
    }
    component OptionRow: Rectangle {
        id: optionRow
        property string text
        property bool checked
        signal clicked
        width: parent?.width ?? 0
        height: 32
        radius: 8
        color: optionMouse.pressed ? Theme.color.fillPressed : optionMouse.containsMouse ? Theme.color.fill : "transparent"
        Icon {
            id: tick
            anchors { left: parent.left; leftMargin: 8; verticalCenter: parent.verticalCenter }
            name: "check"
            bold: true
            size: 14
            color: Theme.accentInk
            opacity: optionRow.checked ? 1 : 0
        }
        Text {
            anchors { left: tick.right; leftMargin: 8; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
            text: optionRow.text
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        MouseArea {
            id: optionMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: optionRow.clicked()
        }
    }
    component SwitchRow: Item {
        id: switchRow
        property string text
        property bool checked
        signal toggled
        width: parent?.width ?? 0
        height: 36
        Text {
            anchors { left: parent.left; leftMargin: 8; verticalCenter: parent.verticalCenter }
            text: switchRow.text
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        Switch {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            checked: switchRow.checked
            onToggled: switchRow.toggled()
        }
    }

    PanelLoader {
        id: slot
        required property var modelData
        panel: "screenTools"
        screen: modelData

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "screenTools"

            readonly property bool shown: Panels.isOpen("screenTools", screen)
            readonly property bool recordOnly: Recorder.toolbarRecord
            readonly property var rec: Config.options.recorder
            readonly property int m: 24
            property bool optionsOpen: false
            onShownChanged: if (shown) {
                optionsOpen = false;
                Recorder.refreshMics();
            }
            // Starts at 0 so the window animates in when it is created
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => shown ? 1 : 0)
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: win.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            function record(area) {
                if (Recorder.recording)
                    return Recorder.stop();
                Recorder.recordArea(area);
            }

            readonly property var tools: recordOnly ? [
                { icon: "monitor", label: "Entire Screen", area: "screen" },
                { icon: "app-window", label: "Window", area: "window" },
                { icon: "selection", label: "Selected Region", area: "region" },
                { sep: true },
                { options: true }
            ] : [
                { icon: "monitor", label: "Capture Screen", run: () => ScreenTools.screenshotScreen() },
                { icon: "selection", label: "Capture Selection", run: () => ScreenTools.select("screenshot") },
                { sep: true },
                { icon: "text-t", label: "Copy Text", run: () => ScreenTools.select("ocr") },
                { icon: "translate", label: "Translate", run: () => ScreenTools.select("translate") },
                { icon: "sparkle", label: "Ask AI", run: () => ScreenTools.select("ask") },
                { sep: true },
                { icon: "record", label: Recorder.recording ? "Stop Recording" : "Record Screen", run: () => win.record("screen") },
                { icon: "app-window", label: "Record Window", run: () => win.record("window"), hide: Recorder.recording },
                { icon: "selection-plus", label: "Record Selection", run: () => win.record("region"), hide: Recorder.recording },
                { sep: true },
                { options: true }
            ]

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            anchors.bottom: true
            margins.bottom: (Config.options.dock.enabled ? Config.options.dock.iconSize + 14 + 6 + 14 : 14) - m
            implicitWidth: Math.max(row.implicitWidth + 24, 340) + 2 * m
            implicitHeight: 64 + 2 * m + (optionsOpen ? options.height - 2 * m + 8 : 0)

            Item {
                anchors.fill: parent
                opacity: win.progress
                y: (1 - win.progress) * 12
                onYChanged: {
                    glass.updatePosition();
                    optionsGlass.updatePosition();
                }
                focus: win.shown
                Keys.onEscapePressed: win.optionsOpen ? win.optionsOpen = false : Panels.close()
                Keys.onReturnPressed: if (win.recordOnly) win.record(win.rec.area)

                // ── Options, above the bar ──────────────────────────────
                Item {
                    id: options
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: bar.top; bottomMargin: 8 - 2 * win.m }
                    width: 340 + 2 * win.m
                    height: win.optionsOpen ? optionList.implicitHeight + 28 + 2 * win.m : 0
                    visible: win.optionsOpen
                    onYChanged: optionsGlass.updatePosition()

                    Glass {
                        id: optionsGlass
                        anchors.fill: parent
                        radius: Theme.radius.popover
                        shadowMargin: win.m
                        lensScale: 0.35
                    }
                    Column {
                        id: optionList
                        x: win.m + 14
                        y: win.m + 14
                        width: parent.width - 2 * win.m - 28
                        spacing: 2

                        Caption { text: "Microphone" }
                        Repeater {
                            model: [{ name: "", description: "None" }, { name: "default", description: "Default microphone" }].concat(Recorder.mics)
                            OptionRow {
                                required property var modelData
                                text: modelData.description
                                checked: win.rec.mic === modelData.name
                                onClicked: win.rec.mic = modelData.name
                            }
                        }
                        Gap {}
                        SwitchRow {
                            text: "System audio"
                            checked: win.rec.systemAudio
                            onToggled: win.rec.systemAudio = !win.rec.systemAudio
                        }
                        SwitchRow {
                            visible: Recorder.gsr
                            text: "Show mouse pointer"
                            checked: win.rec.cursor
                            onToggled: win.rec.cursor = !win.rec.cursor
                        }
                        Item {
                            width: parent.width
                            height: 36
                            Text {
                                anchors { left: parent.left; leftMargin: 8; verticalCenter: parent.verticalCenter }
                                text: "Countdown"
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body }
                            }
                            Segmented {
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                width: 168
                                options: ["None", "3 s", "5 s"]
                                current: [0, 3, 5].indexOf(win.rec.countdown) < 0 ? 0 : [0, 3, 5].indexOf(win.rec.countdown)
                                onSelected: i => win.rec.countdown = [0, 3, 5][i]
                            }
                        }
                        Gap {}
                        Item {
                            width: parent.width
                            height: 36
                            Text {
                                anchors { left: parent.left; leftMargin: 8; right: openFolder.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                                text: "Save to " + Recorder.folder.replace(Quickshell.env("HOME"), "~")
                                elide: Text.ElideMiddle
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                            IconButton {
                                id: openFolder
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                icon: "folder-open"
                                tooltip: "Open the folder"
                                onClicked: {
                                    Panels.close();
                                    Quickshell.execDetached(["sh", "-c", "mkdir -p \"$1\" && exec ${2:-xdg-open} \"$1\"", "sh", Recorder.folder, Files.manager]);
                                }
                            }
                        }
                        Text {
                            visible: !Recorder.gsr
                            width: parent.width
                            leftPadding: 8
                            rightPadding: 8
                            topPadding: 4
                            wrapMode: Text.Wrap
                            text: Recorder.available ? "Install gpu-screen-recorder to pause recordings and record the microphone together with system audio." : "Install gpu-screen-recorder to record the screen."
                            color: Theme.color.tertiary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                }

                // ── The bar ─────────────────────────────────────────────
                Item {
                    id: bar
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom }
                    width: row.implicitWidth + 24 + 2 * win.m
                    height: 64 + 2 * win.m
                    onXChanged: glass.updatePosition()

                    Glass {
                        id: glass
                        anchors.fill: parent
                        radius: 22
                        bevel: 18
                        shadowMargin: win.m
                    }

                    Row {
                        id: row
                        anchors.centerIn: parent
                        spacing: 4

                        Repeater {
                            model: win.tools.filter(t => !t.hide)
                            Item {
                                id: tool
                                required property var modelData
                                readonly property bool picked: win.recordOnly && modelData.area === win.rec.area
                                width: modelData.sep ? 17 : modelData.options ? optionsLabel.implicitWidth + 40 : 104
                                height: 56

                                Rectangle {
                                    visible: !!tool.modelData.sep
                                    anchors.centerIn: parent
                                    width: 1
                                    height: 36
                                    color: Theme.color.separator
                                }
                                Rectangle {
                                    visible: !tool.modelData.sep
                                    anchors.fill: parent
                                    radius: 12
                                    color: tool.picked ? Theme.accentAlpha(0.22) : toolMouse.pressed || (tool.modelData.options && win.optionsOpen) ? Theme.color.fillPressed : toolMouse.containsMouse ? Theme.color.fill : "transparent"
                                    Column {
                                        visible: !tool.modelData.options
                                        anchors.centerIn: parent
                                        spacing: 4
                                        Icon {
                                            anchors.horizontalCenter: parent.horizontalCenter
                                            name: tool.modelData.icon ?? ""
                                            size: 22
                                            color: tool.modelData.icon === "record" && Recorder.recording ? Theme.color.danger : tool.picked ? Theme.accent : Theme.color.label
                                        }
                                        Text {
                                            anchors.horizontalCenter: parent.horizontalCenter
                                            text: tool.modelData.label ?? ""
                                            color: tool.picked ? Theme.color.label : Theme.color.secondary
                                            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                                        }
                                    }
                                    Row {
                                        visible: !!tool.modelData.options
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text {
                                            id: optionsLabel
                                            anchors.verticalCenter: parent.verticalCenter
                                            text: "Options"
                                            color: Theme.color.label
                                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                                        }
                                        Icon {
                                            anchors.verticalCenter: parent.verticalCenter
                                            name: win.optionsOpen ? "caret-down" : "caret-up"
                                            bold: true
                                            size: 11
                                            color: Theme.color.secondary
                                        }
                                    }
                                    MouseArea {
                                        id: toolMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            const t = tool.modelData;
                                            if (t.options)
                                                win.optionsOpen = !win.optionsOpen;
                                            else if (t.area)
                                                win.rec.area = t.area;
                                            else
                                                t.run();
                                        }
                                    }
                                }
                            }
                        }

                        // Recording only: the Record button
                        Item {
                            visible: win.recordOnly
                            width: recordLabel.implicitWidth + 40
                            height: 56
                            Rectangle {
                                anchors.centerIn: parent
                                width: parent.width
                                height: 36
                                radius: 18
                                color: !Recorder.available ? Theme.color.fill : recordMouse.pressed ? Qt.darker(Theme.color.danger, 1.2) : recordMouse.containsMouse ? Qt.darker(Theme.color.danger, 1.08) : Theme.color.danger
                                scale: recordMouse.pressed ? 0.97 : 1
                                Behavior on scale { NumberAnimation { duration: 100 } }
                                Text {
                                    id: recordLabel
                                    anchors.centerIn: parent
                                    text: "Record"
                                    color: Recorder.available ? "white" : Theme.color.tertiary
                                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                                }
                                MouseArea {
                                    id: recordMouse
                                    anchors.fill: parent
                                    enabled: Recorder.available
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: win.record(win.rec.area)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
