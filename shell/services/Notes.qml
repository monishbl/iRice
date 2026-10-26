pragma Singleton
import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import qs.common

// Notes are Markdown files, one per note, in a folder (~/Documents/Notes
// unless Settings → Desktop says otherwise), so any editor or sync tool can
// use them. Spotlight ("note …", "save …"), the quick note window and the
// desktop list read and write here.
Singleton {
    id: root

    readonly property string folder: Config.options.notes?.folder || (Quickshell.env("HOME") + "/Documents/Notes")

    // The newest notes first: [{ name, path, modified }]
    FolderListModel {
        id: files
        folder: "file://" + root.folder
        nameFilters: ["*.md", "*.markdown", "*.txt"]
        showDirs: false
        sortField: FolderListModel.Time
    }
    readonly property int count: files.count
    function list(limit) {
        const out = [];
        for (let i = 0; i < files.count && (!limit || out.length < limit); i++)
            out.push({ name: files.get(i, "fileBaseName"), path: files.get(i, "filePath"), modified: files.get(i, "fileModified") });
        return out;
    }
    function search(q, limit) {
        const t = q.toLowerCase();
        return list(0).filter(n => n.name.toLowerCase().includes(t)).slice(0, limit || 5);
    }

    // A file name from the note's first line: "2026-10-27 Call the bank.md"
    function fileName(title) {
        const clean = title.replace(/[\/\\:*?"<>|#\n\r\t]+/g, " ").replace(/\s+/g, " ").trim().slice(0, 60).trim();
        return Qt.formatDateTime(new Date(), "yyyy-MM-dd HHmm") + (clean ? " " + clean : "") + ".md";
    }
    function titleOf(text) {
        const first = text.trim().split("\n")[0].replace(/^#+\s*/, "");
        return first.length > 60 ? first.slice(0, 57).trim() + "…" : first;
    }

    // Saves a new note; returns its path. `open` opens it in the default editor.
    function create(text, open) {
        const body = text.trim();
        if (body === "")
            return "";
        const title = titleOf(body);
        const name = fileName(title);
        // The first line is the heading, the rest the body
        const lines = body.split("\n");
        const rest = lines.slice(1).join("\n").trim();
        const content = body.startsWith("#") ? body + "\n" : "# " + lines[0].trim() + "\n" + (rest ? "\n" + rest + "\n" : "");
        const path = folder + "/" + name;
        // (never overwrites: the name has the time, and -C refuses an existing file)
        Quickshell.execDetached(["sh", "-c", "mkdir -p \"$1\" && set -C && printf '%s' \"$3\" > \"$1/$2\"", "sh", folder, name, content]);
        Island.post({ kind: "custom", icon: "note-pencil", title: "Note saved", subtitle: title, color: Theme.accents.yellow });
        if (open)
            Qt.callLater(() => root.open(path));
        return path;
    }
    // "save https://…" keeps the link (and any words around it) as a note
    function saveLink(url, words) {
        const note = (words ? words.trim() + "\n\n" : "") + "<" + url + ">";
        return create("Saved link: " + (words?.trim() || url.replace(/^https?:\/\//, "").split("/")[0]) + "\n\n" + note, false);
    }
    function open(path) {
        Launch.open("file://" + path);
    }
    function openFolder() {
        Quickshell.execDetached(["mkdir", "-p", folder]);
        Files.openFolder(folder);
    }
}
