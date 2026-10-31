import QtQuick
import qs.common
import qs.services

// The captions as running text (services/Captions): the sentence before in
// a dimmer white, then what's being said, wrapped, the newest lines kept in
// view (the oldest go off the top), like subtitles that scroll up.
Item {
    id: root

    property color ink: "white"
    property int size: 20
    property int lines: 3
    property bool centred: false
    // Why there are no words yet, in place of them
    readonly property string hint: Captions.error !== "" ? Captions.error
        : Captions.latest !== "" ? ""
        : !Captions.ready ? "Live captions need setting up: Settings → Accessibility"
        : "Captions appear here when something speaks"

    function esc(t) {
        return String(t).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }
    // (whole lines only: the newest `lines` of them)
    implicitHeight: words.lineCount <= lines ? words.implicitHeight : Math.ceil(words.implicitHeight * lines / words.lineCount)
    clip: true

    Text {
        id: words
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        wrapMode: Text.WordWrap
        horizontalAlignment: root.centred ? Text.AlignHCenter : Text.AlignLeft
        lineHeight: 1.35
        lineHeightMode: Text.ProportionalHeight
        textFormat: Text.StyledText
        text: root.hint !== "" ? "<font color=\"" + Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.6) + "\">" + root.esc(root.hint) + "</font>"
            : (Captions.previous !== "" ? "<font color=\"" + Qt.rgba(root.ink.r, root.ink.g, root.ink.b, 0.6) + "\">" + root.esc(Captions.previous) + "</font> " : "") + root.esc(Captions.latest)
        color: root.ink
        font { family: Theme.font.family; pixelSize: root.size; weight: Font.Medium }
        Accessible.role: Accessible.StaticText
        Accessible.name: "Live captions: " + (root.hint || Captions.latest)
    }
}
