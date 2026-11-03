import QtQuick
import qs.common
import qs.services
import "../../../common/LiveTextLogic.js" as Logic

// Live Text over a picture: the words tesseract found in it (LiveText, on
// this computer) as invisible boxes you select like text on a phone: drag
// across words, click one, double-click for its line, Shift+click to
// extend. copy() takes the selection (or everything), selectAll(), clear().
// Shared by the screenshot editor and Quick Preview: put it exactly over the
// picture as shown; `region` is the part shown in the picture's own pixels
// (a crop), empty for all of it. LiveTextBar shows its state and actions.
Item {
    id: root

    property string path
    property string revision: "" // changes when the file does (the editor's saves): read again
    property bool active: false
    property rect region: Qt.rect(0, 0, 0, 0)
    property string highlight: "" // words to select once read (what Search found it by)

    // "" | reading | ready | none | missing | error
    property string status: ""
    property var words: []
    property real pageW: 0
    property real pageH: 0
    property var wifi: [] // Wi-Fi codes in the picture (WifiShare)
    property int anchorIndex: -1
    property int focusIndex: -1
    readonly property bool hasSelection: anchorIndex >= 0 && focusIndex >= 0
    readonly property int selFrom: Math.min(anchorIndex, focusIndex)
    readonly property int selTo: Math.max(anchorIndex, focusIndex)
    readonly property string selectedText: hasSelection ? Logic.textOf(words, anchorIndex, focusIndex) : ""
    readonly property string allText: Logic.allText(words)
    // The chips: links, addresses and numbers in the text, then Wi-Fi codes
    readonly property var actions: Logic.actions(allText, 4).concat(wifi.map(w => ({ kind: "wifi", value: w.ssid, label: w.ssid, wifi: w })))

    // The picture's pixels → this item's
    readonly property rect shown: region.width > 0 ? region : Qt.rect(0, 0, pageW, pageH)
    readonly property real sx: shown.width > 0 ? width / shown.width : 1
    readonly property real sy: shown.height > 0 ? height / shown.height : 1
    function toPicture(x, y) {
        return Qt.point(x / sx + shown.x, y / sy + shown.y);
    }

    readonly property string key: LiveText.keyOf(path, revision)
    function start() {
        clear();
        words = [];
        wifi = [];
        if (!active || path === "") {
            status = "";
            return;
        }
        status = "reading";
        LiveText.read(path, revision);
        const k = key;
        WifiShare.find(path, found => {
            if (root && root.key === k)
                root.wifi = found;
        });
    }
    onActiveChanged: start()
    onKeyChanged: if (active)
        start()
    Connections {
        target: LiveText
        // (Tesseract just installed: read it now)
        function onAvailableChanged() {
            if (root.status === "missing" && LiveText.available)
                root.start();
        }
        function onDone(k, r) {
            if (k !== root.key || !root.active)
                return;
            root.pageW = r.width;
            root.pageH = r.height;
            root.words = r.words;
            root.status = r.status;
            root.selectFound();
            if (r.status === "ready")
                found.restart();
        }
    }

    function clear() {
        anchorIndex = -1;
        focusIndex = -1;
    }
    function selectAll() {
        if (words.length === 0)
            return;
        anchorIndex = 0;
        focusIndex = words.length - 1;
    }
    // What Search found it by: the first match, and the words right after
    // it that match too ("invoice total")
    function selectFound() {
        const m = highlight === "" ? [] : Logic.matching(words, highlight);
        if (m.length === 0)
            return;
        let to = m[0];
        while (m.indexOf(to + 1) >= 0)
            to++;
        anchorIndex = m[0];
        focusIndex = to;
    }
    // Copies the selection, or all the text; returns what to say about it
    function copy() {
        if (hasSelection)
            return LiveText.copy(selectedText, "Text copied");
        return copyAll();
    }
    function copyAll() {
        return allText === "" ? "" : LiveText.copy(allText, "All text copied");
    }

    // ── The words ───────────────────────────────────────────────────────
    // Found: they glow once, then only the selection shows
    property real glow: 0
    SequentialAnimation {
        id: found
        NumberAnimation { target: root; property: "glow"; to: 1; duration: Theme.motion.hover; easing.type: Easing.OutCubic }
        PauseAnimation { duration: 400 }
        NumberAnimation { target: root; property: "glow"; to: 0; duration: Theme.motion.large; easing.type: Easing.InOutQuad }
    }
    Repeater {
        model: root.status === "ready" ? root.words : []
        Rectangle {
            id: box
            required property var modelData
            required property int index
            readonly property bool selected: root.hasSelection && index >= root.selFrom && index <= root.selTo
            // (a selected word reaches the next one on its line: one band, as selected text is)
            readonly property var next: root.words[index + 1]
            readonly property bool joined: selected && index < root.selTo && next && next.line === modelData.line
            readonly property var prev: root.words[index - 1]
            readonly property bool joinedBefore: selected && index > root.selFrom && prev && prev.line === modelData.line
            x: (modelData.x - root.shown.x) * root.sx - 2
            y: (modelData.ly - root.shown.y) * root.sy - 1
            width: ((joined ? next.x : modelData.x + modelData.w) - modelData.x) * root.sx + (joined ? 0 : 4)
            height: modelData.lh * root.sy + 2
            // (rounded only at the band's ends)
            readonly property real r: Theme.material ? 4 : 3
            topLeftRadius: joinedBefore ? 0 : r
            bottomLeftRadius: joinedBefore ? 0 : r
            topRightRadius: joined ? 0 : r
            bottomRightRadius: joined ? 0 : r
            color: selected ? Theme.accentAlpha(0.36) : Theme.accentAlpha(0.22 * root.glow)
        }
    }

    // ── Selecting ───────────────────────────────────────────────────────
    MouseArea {
        anchors.fill: parent
        enabled: root.status === "ready"
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        cursorShape: overWord ? Qt.IBeamCursor : Qt.ArrowCursor
        property bool overWord: false
        property point start
        property bool dragged: false
        function at(m) {
            return root.toPicture(m.x, m.y);
        }
        onPressed: m => {
            const p = at(m);
            start = p;
            dragged = false;
            const i = Logic.wordAt(root.words, p.x, p.y, 4 / root.sx);
            if ((m.modifiers & Qt.ShiftModifier) && root.hasSelection) {
                root.focusIndex = Logic.nearestWord(root.words, p.x, p.y);
            } else if (i >= 0) {
                root.anchorIndex = i;
                root.focusIndex = i;
            } else {
                root.clear(); // (a drag from beside the text starts at its nearest word)
            }
        }
        onPositionChanged: m => {
            const p = at(m);
            if (!pressed) {
                overWord = Logic.wordAt(root.words, p.x, p.y, 4 / root.sx) >= 0;
                return;
            }
            if (!dragged && Math.hypot((p.x - start.x) * root.sx, (p.y - start.y) * root.sy) < 4)
                return;
            dragged = true;
            if (root.anchorIndex < 0)
                root.anchorIndex = Logic.nearestWord(root.words, start.x, start.y);
            root.focusIndex = Logic.nearestWord(root.words, p.x, p.y);
        }
        onDoubleClicked: m => {
            const p = at(m);
            const i = Logic.wordAt(root.words, p.x, p.y, 4 / root.sx);
            if (i < 0)
                return;
            const l = Logic.lineOf(root.words, i);
            root.anchorIndex = l[0];
            root.focusIndex = l[1];
        }
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: status === "ready" ? "Text in the picture" : ""
    Accessible.description: hasSelection ? selectedText : allText
}
