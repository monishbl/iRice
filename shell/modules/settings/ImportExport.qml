import QtQuick
import qs.common
import qs.services
import qs.modules.settings // (after QtQuick: Row is the settings row)

// Import and export for Keyboard Shortcuts and Trackpad, one row per section
// (keybinds = changes to defaults, shortcuts = your own, gestures), plus
// "Export all" and "Import from end4". Files are
//   { "irice-keybinds": 1, "keybinds"?, "shortcuts"?, "gestures"? }
// Imports show what will change first; Apply makes it so, with Undo after.
// The file dialog is zenity or kdialog when installed, else a path field.
Column {
    id: root

    property var sections: ["keybinds", "shortcuts", "gestures"]
    property bool end4: true
    property string title: "Import and export"

    readonly property var names: ({ keybinds: "Changes to default shortcuts", shortcuts: "Your shortcuts", gestures: "Gestures" })
    readonly property var details: ({
            keybinds: "Keys you changed or turned off",
            shortcuts: "Shortcuts you added",
            gestures: "Trackpad gestures you changed or added"
        })

    property string pathMode: "" // "" | export | import (no file dialog installed)
    property var pathSections: []
    property var preview: null
    property string error: ""
    property string status: ""
    property bool busy: false
    property string undoSnap: ""

    width: parent?.width ?? 0
    spacing: 12

    Timer {
        id: undoTimer
        interval: 8000
        onTriggered: { root.undoSnap = ""; root.status = ""; }
    }

    function defaultPath(secs) {
        return "~/Documents/" + (secs.length === 1 && secs[0] === "gestures" ? "irice-gestures.json" : "irice-shortcuts.json");
    }
    function clear() {
        error = "";
        status = "";
        preview = null;
        pathMode = "";
    }
    // A file dialog; cb(path) on a choice, nothing on Cancel, the path field without one
    function pick(save, secs, cb) {
        const start = Keymap.expand(defaultPath(secs));
        let cmd = null;
        if (Keymap.hasZenity)
            cmd = save ? ["zenity", "--file-selection", "--save", "--confirm-overwrite", "--title=Export shortcuts", "--filename=" + start, "--file-filter=JSON files | *.json"] : ["zenity", "--file-selection", "--title=Import shortcuts", "--filename=" + Keymap.home + "/Documents/", "--file-filter=JSON files | *.json"];
        else if (Keymap.hasKdialog)
            cmd = save ? ["kdialog", "--title", "Export shortcuts", "--getsavefilename", start, "*.json"] : ["kdialog", "--title", "Import shortcuts", "--getopenfilename", Keymap.home + "/Documents", "*.json"];
        if (!cmd) {
            pathSections = secs;
            pathMode = save ? "export" : "import";
            pathField.text = defaultPath(secs);
            Qt.callLater(pathField.focusInput);
            return;
        }
        busy = true;
        Keymap.run(Launch.wrap(cmd), (code, out) => {
            root.busy = false;
            const p = out.trim().split("\n")[0] ?? "";
            if (code === 0 && p !== "")
                cb(p);
            else if (code !== 1) {
                // The dialog didn't start: fall back to typing a path
                root.pathSections = secs;
                root.pathMode = save ? "export" : "import";
                pathField.text = root.defaultPath(secs);
            }
        });
    }

    function exportSections(secs) {
        clear();
        pick(true, secs, p => doExport(p, secs));
    }
    function doExport(path, secs) {
        busy = true;
        Keymap.exportTo(path, secs, (ok, err) => {
            root.busy = false;
            root.pathMode = "";
            if (ok)
                root.status = "Exported to " + path.replace(Keymap.home, "~");
            else
                root.error = "Couldn't write " + path + (err ? " (" + err + ")" : "") + ". Choose a folder you can write to, such as ~/Documents.";
        });
    }
    function importSections(secs) {
        clear();
        pick(false, secs, p => doImport(p, secs));
    }
    function doImport(path, secs) {
        busy = true;
        Keymap.readFile(path, (ok, text, err) => {
            root.busy = false;
            root.pathMode = "";
            if (!ok) {
                root.error = "Couldn't read " + path + (err ? " (" + err.replace(/^cat: /, "") + ")" : "") + ". Check the path and that the file exists.";
                return;
            }
            const p = Keymap.previewFile(text, secs);
            if (p.error)
                root.error = p.error;
            else
                root.preview = p;
        });
    }
    function importEnd4() {
        clear();
        busy = true;
        const binds = sections.includes("keybinds") || sections.includes("shortcuts");
        Keymap.previewEnd4(binds, sections.includes("gestures"), p => {
            root.busy = false;
            if (p.error)
                root.error = p.error;
            else
                root.preview = p;
        });
    }
    function apply() {
        const snap = Keymap.snapshot();
        Keymap.applyPreview(preview);
        preview = null;
        undoSnap = snap;
        status = "Imported.";
        undoTimer.restart();
    }

    Group {
        width: parent.width
        title: root.title
        footer: Keymap.hasZenity || Keymap.hasKdialog ? "" : "Install zenity or kdialog for a file dialog; until then, type the file's path."

        Repeater {
            model: root.sections
            Row {
                id: secRow
                required property string modelData
                label: root.names[modelData]
                detail: root.details[modelData]
                Flow {
                    spacing: 8
                    width: imp.implicitWidth + exp.implicitWidth + 8
                    SettingsButton { id: imp; text: "Import…"; enabled: !root.busy; onClicked: root.importSections([secRow.modelData]) }
                    SettingsButton { id: exp; text: "Export…"; enabled: !root.busy; onClicked: root.exportSections([secRow.modelData]) }
                }
            }
        }
        Row {
            visible: root.sections.length > 1
            label: "Everything"
            detail: "All of the above in one file"
            Flow {
                spacing: 8
                width: impAll.implicitWidth + expAll.implicitWidth + 8
                SettingsButton { id: impAll; text: "Import…"; enabled: !root.busy; onClicked: root.importSections(root.sections) }
                SettingsButton { id: expAll; text: "Export all"; enabled: !root.busy; onClicked: root.exportSections(root.sections) }
            }
        }
        Row {
            visible: root.end4
            label: "end4 (illogical-impulse)"
            detail: root.sections.includes("keybinds") ? "Reads ~/.config/hypr/hyprland/keybinds.lua and custom/keybinds.lua, and the gestures in general.lua" : "Reads the gestures in ~/.config/hypr/hyprland/general.lua and custom/general.lua"
            SettingsButton {
                text: "Import from end4"
                busy: root.busy
                enabled: !root.busy
                onClicked: root.importEnd4()
            }
        }
    }

    // ── No file dialog: a path ───────────────────────────────────────────
    Group {
        visible: root.pathMode !== ""
        width: parent.width
        Row {
            label: root.pathMode === "export" ? "Save as" : "Open file"
            Flow {
                spacing: 8
                width: pathField.width + go.implicitWidth + cancelPath.implicitWidth + 16
                TextField {
                    id: pathField
                    width: 260
                    placeholder: "~/Documents/irice-shortcuts.json"
                    commitOnBlur: false
                    onCommitted: go.clicked()
                }
                SettingsButton {
                    id: go
                    primary: true
                    text: root.pathMode === "export" ? "Export" : "Import"
                    enabled: pathField.text.trim() !== "" && !root.busy
                    onClicked: root.pathMode === "export" ? root.doExport(pathField.text.trim(), root.pathSections) : root.doImport(pathField.text.trim(), root.pathSections)
                }
                SettingsButton { id: cancelPath; text: "Cancel"; onClicked: root.pathMode = "" }
            }
        }
    }

    // ── Errors and results ───────────────────────────────────────────────
    Group {
        visible: root.error !== ""
        width: parent.width
        Row {
            label: "Couldn't import or export"
            detail: root.error
            IconButton {
                icon: "x"
                tooltip: "Dismiss"
                onClicked: root.error = ""
            }
        }
    }
    Group {
        visible: root.status !== "" && root.preview === null
        width: parent.width
        Row {
            label: root.status
            SettingsButton {
                visible: root.undoSnap !== ""
                text: "Undo"
                icon: "arrow-counter-clockwise"
                onClicked: {
                    Keymap.restore(root.undoSnap);
                    root.undoSnap = "";
                    root.status = "Import undone.";
                    undoTimer.restart();
                }
            }
        }
    }

    // ── Preview ──────────────────────────────────────────────────────────
    Group {
        id: previewGroup
        visible: root.preview !== null
        width: parent.width
        title: root.preview?.title ?? ""
        footer: root.preview?.note ?? ""

        property bool showAll: false
        property bool showSkipped: false
        readonly property var lines: root.preview?.lines ?? []
        readonly property var skipped: root.preview?.skipped ?? []
        Connections {
            target: root
            function onPreviewChanged() {
                previewGroup.showAll = false;
                previewGroup.showSkipped = false;
            }
        }

        Row {
            label: root.preview?.summary ?? ""
            detail: (root.preview?.total ?? 0) > 0 ? "Nothing changes until you apply." : ""
            Flow {
                spacing: 8
                width: applyBtn.implicitWidth + cancelBtn.implicitWidth + 8
                SettingsButton {
                    id: applyBtn
                    primary: true
                    visible: (root.preview?.total ?? 0) > 0
                    text: "Apply changes"
                    onClicked: root.apply()
                }
                SettingsButton { id: cancelBtn; text: (root.preview?.total ?? 0) > 0 ? "Cancel" : "Close"; onClicked: root.preview = null }
            }
        }
        Repeater {
            model: previewGroup.showAll ? previewGroup.lines : previewGroup.lines.slice(0, 8)
            Item {
                required property var modelData
                width: parent?.width ?? 0
                height: lineText.implicitHeight + 14
                Icon {
                    id: lineIcon
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    name: parent.modelData.kind === "added" ? "plus" : parent.modelData.kind === "off" ? "power" : "pencil-simple"
                    size: 12
                    color: Theme.color.tertiary
                }
                Text {
                    id: lineText
                    anchors { left: lineIcon.right; leftMargin: 10; right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    text: parent.modelData.text
                    wrapMode: Text.Wrap
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
            }
        }
        Row {
            visible: previewGroup.lines.length > 8 || previewGroup.skipped.length > 0
            label: ""
            Flow {
                spacing: 8
                width: moreBtn.implicitWidth + skippedBtn.implicitWidth + 8
                SettingsButton {
                    id: moreBtn
                    visible: previewGroup.lines.length > 8
                    text: previewGroup.showAll ? "Show fewer" : "Show all " + previewGroup.lines.length
                    onClicked: previewGroup.showAll = !previewGroup.showAll
                }
                SettingsButton {
                    id: skippedBtn
                    visible: previewGroup.skipped.length > 0
                    text: previewGroup.showSkipped ? "Hide skipped" : "See skipped (" + previewGroup.skipped.length + ")"
                    onClicked: previewGroup.showSkipped = !previewGroup.showSkipped
                }
            }
        }
        Repeater {
            model: previewGroup.showSkipped ? previewGroup.skipped : []
            Item {
                required property var modelData
                width: parent?.width ?? 0
                height: skipText.implicitHeight + 12
                Text {
                    id: skipText
                    anchors { left: parent.left; leftMargin: 36; right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    text: parent.modelData.text
                    wrapMode: Text.Wrap
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
            }
        }
    }
}
