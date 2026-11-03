import QtQuick
import qs.common
import qs.services

// What search does in both looks (SpotlightGlass and SearchView only draw
// it): the results for what's typed, the chosen row, running it, and an AI
// answer in place of the results. The view's field calls `typed` and the
// key functions.
QtObject {
    id: root

    required property TextInput searchField
    required property bool shown // (the window is open)
    property var results: []
    property int selected: 0
    property bool answering: false // showing an AI answer instead of results

    function reset() {
        answering = false;
        Ai.stopQuick();
        searchField.text = "";
        results = [];
        selected = 0;
        searchField.forceActiveFocus();
        Search.refreshClipboard();
    }
    // The field's text changed
    function typed(text) {
        if (answering) {
            answering = false;
            Ai.stopQuick();
        }
        results = Search.query(text);
        selected = 0;
    }

    // Files, conversions and definitions arrive later
    readonly property Connections revisions: Connections {
        target: Search
        function onRevisionChanged() {
            if (!root.shown || root.answering)
                return;
            // Keep the row you moved to; otherwise the new answer leads
            const keep = root.selected > 0 ? root.results[root.selected]?.title : undefined;
            root.results = Search.query(root.searchField.text);
            const i = root.results.findIndex(r => r.title === keep);
            root.selected = i >= 0 ? i : 0;
        }
    }

    function run(i) {
        const r = results[i];
        if (!r)
            return;
        if (r.ai) {
            answering = true;
            r.run();
            return;
        }
        Panels.close();
        r.run();
    }
    function continueInChat() {
        Ai.continueQuick();
    }
    // Ask the AI about what's typed (or open the chat)
    function askAi() {
        const q = searchField.text.trim().replace(/^\?/, "").trim();
        if (q === "") {
            Panels.show("sidebar");
            return;
        }
        const r = Search.aiRow(q);
        if (r.ai) {
            answering = true;
            r.run();
        } else {
            Panels.close();
            r.run();
        }
        searchField.forceActiveFocus();
    }
    // Back from an AI answer to the results
    function backToResults() {
        answering = false;
        Ai.stopQuick();
        results = Search.query(searchField.text);
        selected = 0;
        searchField.forceActiveFocus();
    }

    // The field's keys: Enter runs the row (or continues an answer in the
    // chat), Up, Down and Tab move, Shift+Space or Ctrl+Y Quick Look the
    // chosen file
    function accept() {
        answering ? continueInChat() : run(selected);
    }
    function down() {
        selected = Math.min(results.length - 1, selected + 1);
    }
    function up() {
        selected = Math.max(0, selected - 1);
    }
    function next() {
        selected = (selected + 1) % Math.max(1, results.length);
    }
    function quickLookKey(e) {
        const r = results[selected];
        if (r?.path && ((e.key === Qt.Key_Space && (e.modifiers & Qt.ShiftModifier)) || (e.key === Qt.Key_Y && (e.modifiers & Qt.ControlModifier)))) {
            Panels.quickLook(r.path);
            e.accepted = true;
        }
    }
}
