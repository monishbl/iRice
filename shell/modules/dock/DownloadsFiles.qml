import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import qs.common
import qs.services

// The Downloads stack's files and what a click does, for both looks' stacks
// (DownloadsStack, MaterialDownloadsStack only draw them): the newest files
// in ~/Downloads, up to 12. Opening a file (sharing it, on a right-click) or
// the folder closes the stack.
QtObject {
    id: root

    required property var dock
    readonly property string dir: Quickshell.env("HOME") + "/Downloads"
    readonly property FolderListModel files: FolderListModel {
        folder: "file://" + root.dir
        sortField: FolderListModel.Time
        showDirs: false
        showHidden: false
    }
    readonly property int shown: Math.min(12, files.count)

    function path(i) {
        return files.get(i, "filePath") ?? "";
    }
    function open(path, button) {
        dock.stackOpen = false;
        // Right-click: share it with a nearby device
        if (button === Qt.RightButton)
            Panels.share([path]);
        else
            Files.open(path);
    }
    function openFolder() {
        dock.stackOpen = false;
        Files.openFolder(dir);
    }
}
