import QtQuick
import qs.common
import qs.services

// Live Text's bar, under the picture (the screenshot editor's bottom bar,
// Quick Preview's footer): what it found, one-tap chips for the links,
// addresses, numbers and Wi-Fi codes in it, Copy text for a selection and
// Copy all text. Every state says what to do next. `live`:
// the LiveTextLayer it speaks for; `maxWidth`: the room it has.
Item {
    id: root

    required property var live
    property real maxWidth: 640

    readonly property string status: live.status
    readonly property bool installing: Updates.running && ScreenTools.ocrInstalling.length > 0
    property string said: "" // what was just copied (a moment)
    function say(what) {
        if (what === "")
            return;
        said = what;
        saidTimer.restart();
    }
    Timer {
        id: saidTimer
        interval: 2000
        onTriggered: root.said = ""
    }
    Connections {
        target: root.live
        function onKeyChanged() { root.said = ""; }
    }

    readonly property string message: said !== "" ? said
        : status === "reading" ? "Finding text…"
        : status === "none" ? "No text found in this picture"
        : status === "missing" && ScreenTools.ocrInstallError !== "" ? "Couldn't install it: " + ScreenTools.ocrInstallError
        : status === "missing" ? "Text recognition needs Tesseract"
        : status === "error" ? "Couldn't read the text in this picture"
        : live.hasSelection ? (live.selTo - live.selFrom + 1) + (live.selTo === live.selFrom ? " word selected" : " words selected")
        : "Drag across text to select it"
    readonly property string glyph: said !== "" ? "check" : status === "reading" ? "circle-notch" : status === "ready" || status === "none" ? "text-aa" : status === "" ? "" : "warning-circle"

    // Chips that fit beside the rest (their labels kept short)
    readonly property real chipRoom: maxWidth - statusRow.implicitWidth - fixed.implicitWidth - 16
    readonly property int chipCount: {
        if (status !== "ready")
            return 0;
        // (as wide as an EditorButton with an icon and this label)
        let used = 0, n = 0;
        for (const a of live.actions) {
            used += labelMetrics.advanceWidth(chipLabel(a)) + 16 + 6 + (Theme.material ? 32 : 28) + 8;
            if (used > chipRoom)
                break;
            n++;
        }
        return n;
    }
    FontMetrics {
        id: labelMetrics
        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; weight: Font.Medium }
    }
    function short(s) {
        return s.length > 22 ? s.slice(0, 21) + "…" : s;
    }
    readonly property var chipIcons: ({ link: "link", email: "envelope", phone: "phone", wifi: "wifi-high" })
    function chipLabel(a) {
        return a.kind === "wifi" ? (WifiShare.joinable(a.wifi) ? "Join " + short(a.label) : "Copy Wi-Fi password") : a.kind === "phone" && !LiveText.telHandler ? "Copy " + short(a.label) : short(a.label);
    }

    implicitWidth: Math.min(maxWidth, content.implicitWidth)
    implicitHeight: Theme.material ? 40 : 36
    visible: status !== ""

    Row {
        id: content
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8

        // What it's doing, found, or what went wrong
        Row {
            id: statusRow
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8
            rightPadding: 4
            // (Material: its loading indicator while reading)
            Loader {
                anchors.verticalCenter: parent.verticalCenter
                active: Theme.material && root.status === "reading" && root.said === ""
                visible: active
                sourceComponent: Spinner {
                    size: 20
                }
            }
            Icon {
                id: statusIcon
                anchors.verticalCenter: parent.verticalCenter
                visible: root.glyph !== "" && !(Theme.material && root.status === "reading" && root.said === "")
                name: root.glyph
                size: 16
                color: root.status === "error" || root.status === "missing" ? Theme.color.danger : Theme.color.secondary
                RotationAnimator on rotation {
                    running: root.status === "reading" && root.said === ""
                    from: 0
                    to: 360
                    duration: 900
                    loops: Animation.Infinite
                    onRunningChanged: if (!running) statusIcon.rotation = 0
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.message
                textFormat: Text.PlainText
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
            }
        }

        // Links, addresses, numbers, Wi-Fi codes
        Repeater {
            model: root.live.actions.slice(0, root.chipCount)
            EditorButton {
                required property var modelData
                anchors.verticalCenter: parent.verticalCenter
                variant: "secondary"
                icon: root.chipIcons[modelData.kind] ?? "link"
                label: root.chipLabel(modelData)
                tipBelow: false
                tooltip: modelData.kind === "link" ? "Open in your browser" : modelData.kind === "email" ? "Write an email" : modelData.kind === "phone" ? (LiveText.telHandler ? "Call" : "Copy the number") : WifiShare.joinable(modelData.wifi) ? "Join this network" : "Copy the password"
                onClicked: root.say(LiveText.act(modelData))
            }
        }

        // Copying, and the way out of each problem
        Row {
            id: fixed
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8
            EditorButton {
                visible: root.status === "ready" && root.live.hasSelection
                variant: "secondary"
                icon: "copy"
                label: "Copy text"
                tipBelow: false
                shortcut: "Ctrl+C"
                tooltip: "Copy the selected text"
                onClicked: root.say(root.live.copy())
            }
            EditorButton {
                visible: root.status === "ready"
                icon: root.live.hasSelection ? "" : "copy"
                label: "Copy all text"
                tipBelow: false
                onClicked: root.say(root.live.copyAll())
            }
            EditorButton {
                visible: root.status === "error"
                variant: "secondary"
                label: "Try again"
                onClicked: root.live.start()
            }
            // (through Software Update, which asks for your password)
            EditorButton {
                visible: root.status === "missing"
                variant: "secondary"
                icon: "download-simple"
                label: root.installing ? "Installing…" : "Install Tesseract"
                busy: root.installing
                enabled: !Updates.running
                tipBelow: false
                tooltip: "Reads text on this computer; nothing is uploaded"
                onClicked: ScreenTools.installOcr("")
            }
        }
    }
}
