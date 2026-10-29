import QtQuick
import QtQuick as QQ
import QtQuick.Controls.Basic as QQC
import qs.common
import qs.modules.settings
import qs.services

// Report a Bug: what happened, and what's attached (screenshot, log, system
// info), each shown before it's sent and each can be left out. Sent to
// iRice's report server (services/BugReport.qml), which files a GitHub issue.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 24

    // A labelled box for a few lines of text
    component Box: Column {
        id: box
        property string label
        property string hint
        property string value
        signal edited(string text)
        // (typing breaks the text binding; a cleared form still clears the box)
        onValueChanged: if (area.text !== value) area.text = value
        width: parent?.width ?? 0
        spacing: 6
        Text {
            text: box.label
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        Rectangle {
            width: parent.width
            height: 88
            radius: Theme.radius.control
            color: Theme.color.fill
            border.width: 1
            border.color: area.activeFocus ? Theme.accent : Theme.color.separator
            QQC.ScrollView {
                anchors { fill: parent; margins: 8 }
                QQC.TextArea {
                    id: area
                    wrapMode: TextEdit.Wrap
                    text: box.value
                    placeholderText: box.hint
                    placeholderTextColor: Theme.color.tertiary
                    color: Theme.color.label
                    background: null
                    selectByMouse: true
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                    onTextChanged: if (text !== box.value) box.edited(text)
                }
            }
        }
    }
    // What's attached: scrubbed text shown as it will be sent
    component Preview: Rectangle {
        id: pv
        property string body
        width: parent?.width ?? 0
        height: 200
        radius: 8
        color: Theme.dark ? "#0E0E10" : "#18181B"
        Flickable {
            id: flick
            anchors { fill: parent; margins: 10 }
            clip: true
            contentHeight: previewText.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            Text {
                id: previewText
                width: flick.width
                text: pv.body
                wrapMode: Text.WrapAnywhere
                textFormat: Text.PlainText
                color: "#D4D4D8"
                font { family: Theme.font.mono; pixelSize: 11 }
            }
        }
        ScrollIndicator {
            target: flick
            anchors { right: parent.right; top: parent.top; bottom: parent.bottom; rightMargin: 2 }
        }
    }

    property bool showLog: false
    property bool showInfo: false
    readonly property string infoText: Object.keys(BugReport.sysinfo).map(k => k + ": " + BugReport.sysinfo[k]).join("\n")

    Group {
        width: parent.width
        title: "What happened"
        footer: "Write in your own words; a few clear sentences help most. Reports become public issues on GitHub, so don't include anything private."
        Row {
            label: "Title"
            TextField {
                width: 300
                placeholder: "e.g. Dock disappears after waking"
                text: BugReport.title
                commitOnBlur: true
                onTextChanged: BugReport.title = text
            }
        }
        Item {
            width: parent.width
            height: fields.implicitHeight + 24
            QQ.Column {
                id: fields
                x: 12
                y: 12
                width: parent.width - 24
                spacing: 16
                Box {
                    label: "What happened"
                    hint: "What you saw, and when"
                    value: BugReport.happened
                    onEdited: t => BugReport.happened = t
                }
                Box {
                    label: "Steps to reproduce"
                    hint: "1. Open …  2. Click …  (optional)"
                    value: BugReport.steps
                    onEdited: t => BugReport.steps = t
                }
                Box {
                    label: "What you expected"
                    hint: "Optional"
                    value: BugReport.expected
                    onEdited: t => BugReport.expected = t
                }
            }
        }
    }

    Group {
        width: parent.width
        title: "Attached"
        footer: "Only what's on is sent. Home folders, your user and computer names, email addresses, keys, IP and MAC addresses, and the names of your Wi-Fi networks and Bluetooth devices are taken out of the log and system info first."
        Row {
            label: "Screenshot"
            detail: !BugReport.hasShot ? "None was taken. Open this page with the Report a bug shortcut or Spotlight to include one" : "The screen when you started the report. Mark it up to hide anything private"
            QQ.Row {
                spacing: 8
                SettingsButton {
                    visible: BugReport.hasShot
                    text: "Mark up…"
                    icon: "pencil-simple"
                    onClicked: BugReport.editShot()
                }
                Switch {
                    enabled: BugReport.hasShot
                    checked: BugReport.includeShot && BugReport.hasShot
                    onToggled: BugReport.includeShot = !checked
                }
            }
        }
        Item {
            visible: BugReport.hasShot && BugReport.includeShot
            width: parent.width
            height: visible ? shotImage.height + 24 : 0
            Image {
                id: shotImage
                x: 12
                y: 12
                width: Math.min(parent.width - 24, 480)
                height: implicitHeight > 0 ? width * implicitHeight / implicitWidth : 0
                source: BugReport.hasShot ? "file://" + BugReport.shot + "?v=" + BugReport.shotVersion : ""
                cache: false
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                sourceSize.width: 960
            }
        }
        Row {
            label: "iRice log"
            detail: BugReport.gathering ? "Reading…" : "The last 500 lines, scrubbed"
            QQ.Row {
                spacing: 8
                SettingsButton {
                    text: page.showLog ? "Hide" : "Show"
                    onClicked: page.showLog = !page.showLog
                }
                Switch {
                    checked: BugReport.includeLog
                    onToggled: BugReport.includeLog = !checked
                }
            }
        }
        Item {
            visible: page.showLog
            width: parent.width
            height: visible ? 224 : 0
            Preview {
                x: 12
                y: 12
                width: parent.width - 24
                body: BugReport.gathering ? "Reading the log…" : (BugReport.log || "The log is empty.")
            }
        }
        Row {
            label: "System info"
            detail: "iRice, Hyprland and Quickshell versions, graphics, displays, distribution"
            QQ.Row {
                spacing: 8
                SettingsButton {
                    text: page.showInfo ? "Hide" : "Show"
                    onClicked: page.showInfo = !page.showInfo
                }
                Switch {
                    checked: BugReport.includeInfo
                    onToggled: BugReport.includeInfo = !checked
                }
            }
        }
        Item {
            visible: page.showInfo
            width: parent.width
            height: visible ? 164 : 0
            Preview {
                x: 12
                y: 12
                width: parent.width - 24
                height: 140
                body: BugReport.gathering ? "Reading…" : page.infoText
            }
        }
    }

    Group {
        width: parent.width
        footer: "Sent to iRice's report server, which files it as an issue on github.com/monishbl/iRice. Your IP address is used only to limit how many reports come from one network and isn't stored with the report."
        Row {
            label: BugReport.state_ === "sending" ? "Sending…"
                : BugReport.state_ === "sent" ? "Report sent. Thank you!"
                : BugReport.state_ === "error" ? BugReport.error
                : BugReport.waitSeconds > 0 ? "You can send another in " + BugReport.waitSeconds + " s"
                : "Ready to send"
            labelColor: BugReport.state_ === "error" ? Theme.color.danger : Theme.color.secondary
            detail: BugReport.state_ === "sent" && BugReport.issueUrl !== "" ? BugReport.issueUrl : ""
            QQ.Row {
                spacing: 8
                SettingsButton {
                    visible: BugReport.state_ === "sent" && BugReport.issueUrl !== ""
                    text: "Open issue"
                    icon: "arrow-square-out"
                    onClicked: Qt.openUrlExternally(BugReport.issueUrl)
                }
                SettingsButton {
                    visible: BugReport.state_ === "error"
                    text: "Copy report"
                    icon: "copy"
                    onClicked: BugReport.copy()
                }
                SettingsButton {
                    visible: BugReport.state_ === "error"
                    text: "Open on GitHub"
                    icon: "github-logo"
                    onClicked: BugReport.openGithub()
                }
                SettingsButton {
                    primary: true
                    text: "Send report"
                    icon: "paper-plane-tilt"
                    busy: BugReport.state_ === "sending"
                    enabled: BugReport.state_ !== "sending" && BugReport.waitSeconds === 0 && !BugReport.gathering
                    onClicked: BugReport.send()
                }
            }
        }
    }

    // Opened from the sidebar rather than the shortcut: no screenshot, but
    // the log and system info are still read
    Component.onCompleted: if (BugReport.log === "" && !BugReport.gathering) BugReport.gather()
}
