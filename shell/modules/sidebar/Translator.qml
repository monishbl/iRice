import QtQuick
import QtQuick.Controls.Basic
import Quickshell.Io
import qs.common
import qs.modules.controlcenter
import qs.services

// Translator (translate-shell). Translates a moment after you stop typing.
Item {
    id: root

    property bool active: false
    onActiveChanged: if (active) Qt.callLater(() => input.forceActiveFocus())

    readonly property var languages: [
        ["auto", "Detect"], ["en", "English"], ["hi", "Hindi"], ["es", "Spanish"], ["fr", "French"], ["de", "German"],
        ["it", "Italian"], ["pt", "Portuguese"], ["ru", "Russian"], ["ja", "Japanese"], ["ko", "Korean"], ["zh-CN", "Chinese"],
        ["ar", "Arabic"], ["bn", "Bengali"], ["ta", "Tamil"], ["te", "Telugu"], ["mr", "Marathi"], ["tr", "Turkish"], ["nl", "Dutch"]
    ]
    property string source: "auto"
    property string target: Config.options.sidebar.translateTarget
    property string result: ""
    property string error: ""

    function name(code) {
        return languages.find(l => l[0] === code)?.[1] ?? code;
    }

    Timer {
        id: debounce
        interval: 600
        onTriggered: root.translate()
    }

    property int generation: 0
    property var activeRequest: null
    function translate() {
        const version = ++generation;
        if (activeRequest) activeRequest.running = false;
        result = "";
        error = "";
        if (input.text.trim() === "") return;
        const request = requestComponent.createObject(root, { version: version, requestedText: input.text,
            command: ["trans", "-brief", "-no-ansi", "-s", source, "-t", target, input.text] });
        activeRequest = request;
        request.running = true;
    }
    Component {
        id: requestComponent
        Process {
            id: request
            property int version
            property string requestedText
            stdout: StdioCollector {
                onStreamFinished: if (request.version === root.generation && input.text === request.requestedText) {
                    root.result = text.trim();
                    root.error = "";
                }
            }
            onExited: code => {
                if (version === root.generation && input.text === requestedText && code !== 0)
                    root.error = "Couldn't translate. Check your connection and translate-shell installation.";
                if (root.activeRequest === request) root.activeRequest = null;
                destroy();
            }
        }
    }

    Column {
        anchors.fill: parent
        spacing: 10

        // Language bar
        Item {
            width: parent.width
            height: 30
            LanguageButton {
                id: fromButton
                anchors.left: parent.left
                width: (parent.width - 44) / 2
                label: root.name(root.source)
                options: root.languages
                onChosen: code => { root.source = code; root.translate(); }
            }
            Rectangle {
                anchors.centerIn: parent
                width: 30
                height: 30
                radius: 15
                opacity: root.source === "auto" ? 0.4 : 1
                color: swapMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                Icon { anchors.centerIn: parent; name: "arrows-left-right"; size: 14 }
                MouseArea {
                    id: swapMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    enabled: root.source !== "auto"
                    onClicked: {
                        const s = root.source;
                        root.source = root.target;
                        Config.options.sidebar.translateTarget = s;
                        input.text = root.result;
                    }
                }
            }
            LanguageButton {
                anchors.right: parent.right
                width: (parent.width - 44) / 2
                label: root.name(root.target)
                options: root.languages.slice(1)
                onChosen: code => { Config.options.sidebar.translateTarget = code; root.translate(); }
            }
        }

        Platter {
            width: parent.width
            height: (parent.height - 40 - 10) / 2
            radius: 18
            ScrollView {
                anchors { fill: parent; margins: 12 }
                TextArea {
                    id: input
                    padding: 0
                    wrapMode: TextEdit.Wrap
                    placeholderText: "Type or paste text"
                    placeholderTextColor: Theme.color.tertiary
                    color: Theme.color.label
                    background: null
                    font { family: Theme.font.family; pixelSize: Theme.font.title }
                    onTextChanged: debounce.restart()
                }
            }
        }

        Platter {
            width: parent.width
            height: (parent.height - 40 - 10) / 2
            radius: 18
            ScrollView {
                anchors { fill: parent; margins: 12; bottomMargin: 40 }
                TextEdit {
                    width: parent.width
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    text: root.error || root.result
                    color: root.error ? Theme.color.danger : Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title }
                }
            }
            Rectangle {
                anchors { right: parent.right; bottom: parent.bottom; margins: 10 }
                visible: root.result !== "" && root.error === ""
                width: copyLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: copyMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                Text {
                    id: copyLabel
                    anchors.centerIn: parent
                    text: copied.running ? "Copied" : "Copy"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                MouseArea {
                    id: copyMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                        System.run(["wl-copy", root.result]);
                        copied.restart();
                    }
                }
                Timer { id: copied; interval: 1500 }
            }
        }
    }
}
