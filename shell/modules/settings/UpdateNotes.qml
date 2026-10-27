import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common

// Update notes (Software Update): CHANGELOG.md's entries for an update, as
// markdown (bold sub-headings, pointers, links, `code`), newest first. Long
// notes start with the newest entry only. Without changelog entries, the
// commit subjects as pointers.
//   UpdateNotes { entries: report.changelog; fallback: report.notes }
Column {
    id: root

    property var entries: [] // [{ title, date, body }]
    property var fallback: [] // commit subjects
    property bool all: false
    readonly property var shown: all ? entries : entries.slice(0, 1)

    spacing: 16

    Repeater {
        model: root.shown
        Column {
            id: entry
            required property var modelData
            width: root.width
            spacing: 6
            QQ.Row { // (Row here is the settings row)
                spacing: 8
                Text {
                    id: entryTitle
                    text: entry.modelData.title
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: 16; weight: Font.DemiBold }
                }
                Text {
                    anchors.baseline: entryTitle.baseline
                    visible: entry.modelData.date !== ""
                    text: entry.modelData.date
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                }
            }
            Text {
                width: parent.width
                text: entry.modelData.body
                textFormat: Text.MarkdownText
                wrapMode: Text.Wrap
                color: Theme.color.label
                linkColor: Theme.accent
                font { family: Theme.font.family; pixelSize: 14 }
                onLinkActivated: link => Qt.openUrlExternally(link)
                HoverHandler {
                    cursorShape: parent.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor
                }
            }
        }
    }
    // No changelog: what the commits say
    Text {
        visible: root.entries.length === 0 && root.fallback.length > 0
        width: root.width
        text: root.fallback.map(l => "- " + l.replace(/([*_`\[\]])/g, "\\$1")).join("\n")
        textFormat: Text.MarkdownText
        wrapMode: Text.Wrap
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: 14 }
    }
    Text {
        visible: root.entries.length > 1
        text: root.all ? "Show only the latest" : "Show all notes (" + root.entries.length + ")"
        color: Theme.accent
        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.all = !root.all
        }
    }
}
