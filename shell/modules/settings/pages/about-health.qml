import QtQuick
import QtQuick as QQ
import qs.common
import qs.modules.settings
import qs.services

// Settings → About → System health: each service iRice uses, checked when
// the page opens (services/Health.qml), with how to fix what's broken.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22
    readonly property string pageTitle: "System health"

    Component.onCompleted: Health.check()
    // A fix that runs as administrator is confirmed first: the row shows
    // exactly what will run, and only the second press runs it (asking for
    // the password until an update installed iRice's root helper)
    property string confirming: ""

    // What a status looks like: colour and a word, never colour alone
    function tone(status) {
        return status === "ok" ? Theme.color.success : status === "warn" ? Theme.color.warning : status === "fail" ? Theme.color.danger : Theme.color.tertiary;
    }
    function word(status) {
        return ({ ok: "OK", warn: "Check", fail: "Not working", off: "Off" })[status] ?? status;
    }

    Group {
        width: parent.width
        footer: Health.error !== "" ? Health.error
            : Health.checkedAt > 0 ? "Checked at " + Qt.formatTime(new Date(Health.checkedAt), "h:mm AP") + ". Nothing is checked in the background; this page asks each service when it opens."
            : ""
        Row {
            label: Health.checking && Health.checks.length === 0 ? "Checking…"
                : Health.attention.length === 0 ? "All good"
                : Health.attention.length === 1 ? "1 thing needs attention"
                : Health.attention.length + " things need attention"
            labelColor: Health.attention.some(c => c.status === "fail") ? Theme.color.danger : Theme.color.label
            detail: Health.checks.length === 0 ? "" : Health.attention.length === 0 ? "Every service iRice uses is answering." : Health.attention.map(c => c.name).join(", ")
            QQ.Row {
                spacing: 8
                SettingsButton {
                    text: "Copy report"
                    icon: "copy"
                    enabled: Health.checks.length > 0
                    onClicked: Health.copyReport()
                }
                SettingsButton {
                    text: Health.checking ? "Checking…" : "Check again"
                    icon: "arrow-clockwise"
                    busy: Health.checking
                    onClicked: Health.check()
                }
            }
        }
    }

    Repeater {
        model: Health.groups
        Group {
            id: group
            required property string modelData
            // (not `rows`: Group's own rows are its content)
            readonly property var checks: Health.checks.filter(c => c.group === modelData)
            visible: checks.length > 0
            width: page.width
            title: modelData
            Repeater {
                model: group.checks
                Row {
                    id: check
                    required property var modelData
                    label: modelData.name
                    detail: modelData.detail + (page.confirming === modelData.id ? (modelData.fixPassword ? "\nRuns as administrator, after your password: " : "\nRuns as administrator: ") + modelData.fixCommand
                        : modelData.fix !== "" && (modelData.status === "fail" || modelData.status === "warn") ? "\nTo fix: " + modelData.fix : "")
                    QQ.Row {
                        spacing: 10
                        SettingsButton {
                            visible: page.confirming === check.modelData.id
                            text: "Cancel"
                            onClicked: page.confirming = ""
                        }
                        SettingsButton {
                            visible: check.modelData.canFix
                            primary: page.confirming === check.modelData.id
                            text: Health.fixing === check.modelData.id ? "Fixing…" : page.confirming === check.modelData.id ? (check.modelData.fixPassword ? "Run with password" : "Run fix") : "Fix"
                            busy: Health.fixing === check.modelData.id
                            enabled: Health.fixing === ""
                            onClicked: {
                                if (check.modelData.fixRoot && page.confirming !== check.modelData.id) {
                                    page.confirming = check.modelData.id;
                                    return;
                                }
                                page.confirming = "";
                                Health.fix(check.modelData.id);
                            }
                        }
                        // The status: a dot and a word
                        QQ.Row {
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 6
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: 8
                                height: 8
                                radius: 4
                                color: page.tone(check.modelData.status)
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: page.word(check.modelData.status)
                                color: check.modelData.status === "ok" || check.modelData.status === "off" ? Theme.color.secondary : Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: check.modelData.status === "fail" ? Font.DemiBold : Font.Normal }
                            }
                        }
                    }
                }
            }
        }
    }
}
