import QtQuick
import qs.common
import qs.services

// Circle to search's "Search the web": the picture goes to Google Lens only
// on this click, and the results open in the browser. Below it, how that
// went (opened, paste it in, or what went wrong and what to do).
Column {
    id: root

    property string picture // (the circled picture's file)
    readonly property var done: ScreenTools.lensDone?.path === picture ? ScreenTools.lensDone : null
    readonly property bool busy: ScreenTools.lensBusy !== "" && ScreenTools.lensBusy === picture

    spacing: 6

    ChatButton {
        icon: root.done?.ok ? "check" : "magnifying-glass"
        label: root.busy ? "Searching the web…" : "Search the web"
        busy: root.busy
        onClicked: ScreenTools.searchWeb(root.picture)
    }
    Text {
        visible: root.done !== null
        width: root.width
        wrapMode: Text.Wrap
        text: root.done?.text ?? ""
        color: root.done?.ok ? Theme.color.secondary : Theme.color.danger
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
}
