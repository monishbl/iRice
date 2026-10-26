import QtQuick
import QtQuick.Effects
import Qt.labs.folderlistmodel
import Quickshell
import qs.common

// Photos: a picture from a folder (Settings → Desktop, default ~/Pictures),
// fills the widget and changes every 15 minutes with a crossfade. Click for
// the next one.
Item {
    id: root

    property string size: "m"
    property bool preview: false
    property real radius: 20 // the frame's corner radius (photo fills the widget)
    readonly property string folder: Config.options.desktop.photosFolder || (Quickshell.env("HOME") + "/Pictures")

    FolderListModel {
        id: files
        folder: "file://" + root.folder
        nameFilters: ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.JPG", "*.JPEG", "*.PNG"]
        showDirs: false
        onStatusChanged: if (status === FolderListModel.Ready) root.next()
    }

    property bool useA: true
    function next() {
        if (files.count === 0)
            return;
        const url = "file://" + files.get(Math.floor(Math.random() * files.count), "filePath");
        if (useA)
            b.source = url;
        else
            a.source = url;
        useA = !useA;
    }
    Timer {
        running: !root.preview && files.count > 1
        repeat: true
        interval: 15 * 60 * 1000
        onTriggered: root.next()
    }

    Item {
        id: photos
        anchors.fill: parent
        visible: false
        layer.enabled: true
        Image {
            id: a
            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(root.width * 1.5, root.height * 1.5)
            asynchronous: true
            opacity: root.useA ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: 800 } }
        }
        Image {
            id: b
            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(root.width * 1.5, root.height * 1.5)
            asynchronous: true
            opacity: root.useA ? 0 : 1
            Behavior on opacity { NumberAnimation { duration: 800 } }
        }
    }
    Rectangle {
        id: mask
        anchors.fill: parent
        radius: root.radius
        visible: false
        layer.enabled: true
    }
    MultiEffect {
        anchors.fill: parent
        source: photos
        maskEnabled: true
        maskSource: mask
    }

    // Empty state
    Column {
        anchors.centerIn: parent
        width: parent.width - 32
        visible: files.count === 0
        spacing: 6
        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "image"; size: 28; color: Theme.color.tertiary }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: "Add pictures to " + root.folder.replace(Quickshell.env("HOME"), "~")
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: !root.preview && files.count > 1
        cursorShape: Qt.PointingHandCursor
        onClicked: root.next()
    }
}
