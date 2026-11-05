import QtQuick
import Quickshell
import qs.common
import qs.services

// While tessera-ui draws the quick note (rust.surfaces: notes), what only
// this shell knows reaches it here, a line on its stdin (RustUi.tell): the
// Notes folder (with ~), and how a save went ({ saving: path } as it starts,
// then { saved: path } or { failed: path }). What it asks (RustUi's "notes"
// call): save the text (Notes.create), open the Notes folder.
// (native/tessera-ui/src/surfaces/notes reads what this sends)
Item {
    id: root

    readonly property bool here: RustUi.drawing("notes")
    readonly property string folder: Notes.folder.replace(Quickshell.env("HOME"), "~")
    function send() {
        RustUi.tell("notes", { folder: root.folder });
    }
    onFolderChanged: send()
    onHereChanged: if (here) send()
    Connections {
        target: RustUi
        function onStarted() {
            root.send();
        }
        function onDrawnChanged() {
            if (root.here)
                root.send();
        }
        function onFeedCall(surface, what, a) {
            if (surface !== "notes")
                return;
            if (what === "save") {
                const text = String(a[0] ?? "");
                RustUi.tell("notes", { saving: text.trim() === "" ? "" : Notes.create(text, false) });
            } else if (what === "openFolder") {
                Panels.close();
                Notes.openFolder();
            }
        }
    }
    Connections {
        target: Notes
        enabled: root.here
        function onSaved(path) {
            RustUi.tell("notes", { saved: path });
        }
        function onSaveFailed(path) {
            RustUi.tell("notes", { failed: path });
        }
    }
}
