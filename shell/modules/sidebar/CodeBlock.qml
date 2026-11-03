import QtQuick
import qs.common
import qs.services

// A fenced code block in a reply: its language and a copy button above the
// code, in the look's code fill; long lines wrap. `open`: still streaming.
Rectangle {
    id: root

    property string lang
    property string code
    property bool open: false

    implicitHeight: head.height + codeText.implicitHeight + 10
    radius: Theme.material ? 16 : 10
    color: Theme.codeFill

    Item {
        id: head
        width: parent.width
        height: 30
        Text {
            anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
            text: root.lang !== "" ? root.lang : "code"
            color: Theme.color.secondary
            font { family: Theme.font.mono; pixelSize: Theme.font.caption }
        }
        Rectangle {
            id: copyButton
            visible: !root.open
            anchors { right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
            width: copyRow.implicitWidth + 16
            height: 24
            radius: 12
            color: copyMouse.containsMouse ? (Theme.material ? Qt.alpha(Theme.m3.onSurfaceVariant, 0.08) : Theme.color.fill) : "transparent"
            Row {
                id: copyRow
                anchors.centerIn: parent
                spacing: 4
                Icon { id: copyIcon; anchors.verticalCenter: parent.verticalCenter; name: "copy"; size: 12; color: Theme.color.secondary }
                Text {
                    id: copyLabel
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Copy"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption }
                }
            }
            MouseArea {
                id: copyMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                Accessible.role: Accessible.Button
                Accessible.name: "Copy code"
                onClicked: {
                    System.run(["wl-copy", "--", root.code]);
                    copyIcon.name = "check";
                    copyLabel.text = "Copied";
                    copyReset.restart();
                }
            }
            Timer {
                id: copyReset
                interval: 1500
                onTriggered: {
                    copyIcon.name = "copy";
                    copyLabel.text = "Copy";
                }
            }
        }
    }

    TextEdit {
        id: codeText
        x: 12
        y: head.height
        width: parent.width - 24
        readOnly: true
        selectByMouse: true
        wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
        textFormat: TextEdit.PlainText
        text: root.code
        color: Theme.color.label
        selectionColor: Theme.material ? Qt.alpha(Theme.m3.primary, 0.3) : Theme.accentAlpha(0.4)
        font { family: Theme.font.mono; pixelSize: Theme.font.small }
    }
}
