import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common

// A path in a text field, with Choose… to pick it in a file chooser.
//   PathField { value: cfg.x; folder: true; onPicked: p => cfg.x = p }
QQ.Row {
    id: root

    property string value
    property string placeholder
    property bool folder: false
    property string filter // e.g. "Images|*.jpg *.png"; files only
    property string start: "~"
    property int fieldWidth: 200
    signal picked(string path)

    spacing: 8

    TextField {
        width: root.fieldWidth
        anchors.verticalCenter: parent.verticalCenter
        text: root.value
        placeholder: root.placeholder
        onCommitted: t => {
            const p = t.trim().replace(/^~/, Quickshell.env("HOME"));
            if (p !== root.value)
                root.picked(p);
        }
    }
    SettingsButton {
        anchors.verticalCenter: parent.verticalCenter
        text: FilePicker.busy ? "Choosing…" : "Choose…"
        enabled: !FilePicker.busy
        onClicked: FilePicker.pick({
            folder: root.folder,
            title: root.folder ? "Choose a folder" : "Choose a file",
            start: root.value !== "" ? (root.folder ? root.value : root.value.replace(/\/[^/]*$/, "")) : root.start,
            filter: root.filter
        }, p => root.picked(p))
    }
}
