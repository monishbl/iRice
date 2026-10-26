import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Widgets
import Quickshell.Io
import qs.common
import qs.modules.settings

Column {
    id: root
    width: parent?.width ?? 0
    spacing: 22

    ClippingRectangle {
        width: Math.min(parent.width, 420)
        height: width * 9 / 16
        radius: 14
        color: Theme.color.fill
        Image {
            anchors.fill: parent
            source: Wallpaper.source
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(840, 472)
            asynchronous: true
        }
    }

    Group {
        width: parent.width
        Row {
            label: "Folder"
            detail: "Images in this folder appear below and in the wallpaper picker."
            TextField {
                width: 260
                text: Config.options.wallpaper.directory
                placeholder: "~/Pictures/Wallpapers"
                onCommitted: t => Config.options.wallpaper.directory = t.replace(/^~/, Quickshell.env("HOME"))
            }
        }
    }

    // Light/dark pairs and time-of-day folders found in the wallpaper folder
    Group {
        width: parent.width
        visible: Wallpaper.sets.length > 0
        title: "Wallpaper sets"
        footer: "Pairs named “Name Light” and “Name Dark” switch with light and dark mode; a sub-folder of pictures follows the time of day."
        Flow {
            width: parent.width
            spacing: 12
            padding: 4
            Repeater {
                model: Wallpaper.sets
                Column {
                    id: setTile
                    required property var modelData
                    readonly property bool current: Wallpaper.isCurrentSet(modelData)
                    spacing: 6
                    ClippingRectangle {
                        width: 150
                        height: 90
                        radius: 12
                        color: Theme.color.fill
                        border.width: setTile.current ? 3 : 0
                        border.color: Theme.accent
                        // Pairs: light on the left, dark on the right
                        Image {
                            width: setTile.modelData.kind === "pair" ? parent.width / 2 : parent.width
                            height: parent.height
                            source: "file://" + setTile.modelData.preview
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(300, 180)
                            asynchronous: true
                        }
                        Image {
                            visible: setTile.modelData.kind === "pair"
                            x: parent.width / 2
                            width: parent.width / 2
                            height: parent.height
                            source: setTile.modelData.kind === "pair" ? "file://" + setTile.modelData.dark : ""
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(300, 180)
                            asynchronous: true
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Wallpaper.useSet(setTile.modelData)
                        }
                    }
                    Text {
                        width: 150
                        text: setTile.modelData.name
                        elide: Text.ElideRight
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                    }
                    Text {
                        text: setTile.modelData.kind === "pair" ? "Light & dark" : "Time of day"
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                }
            }
        }
    }

    Group {
        width: parent.width
        title: "Dynamic"
        footer: Config.options.wallpaper.dynamic === "time" ? "Follows sunrise and sunset for your weather location. Choosing a picture below turns this off." : Config.options.wallpaper.dynamic === "appearance" ? "Switches with light and dark mode (and Auto appearance)." : ""
        Row {
            label: "Change automatically"
            Segmented {
                width: 300
                readonly property var values: ["off", "time", "appearance"]
                options: ["Off", "Time of day", "Light & dark"]
                current: Math.max(0, values.indexOf(Config.options.wallpaper.dynamic))
                onSelected: i => Config.options.wallpaper.dynamic = values[i]
            }
        }
        Row {
            visible: Config.options.wallpaper.dynamic === "time"
            label: "Pictures"
            detail: "A folder of pictures in name order: the first half spans the day, the rest the night. Empty uses the built-in Tahoe set."
            TextField {
                width: 260
                text: Config.options.wallpaper.dynamicFolder
                placeholder: "Built-in Tahoe set"
                onCommitted: t => Config.options.wallpaper.dynamicFolder = t.trim().replace(/^~/, Quickshell.env("HOME"))
            }
        }
        Row {
            visible: Config.options.wallpaper.dynamic === "appearance"
            label: "Light picture"
            TextField {
                width: 260
                text: Config.options.wallpaper.lightPath
                placeholder: "Built-in Tahoe day"
                onCommitted: t => Config.options.wallpaper.lightPath = t.trim().replace(/^~/, Quickshell.env("HOME"))
            }
        }
        Row {
            visible: Config.options.wallpaper.dynamic === "appearance"
            label: "Dark picture"
            TextField {
                width: 260
                text: Config.options.wallpaper.darkPath
                placeholder: "Built-in Tahoe dusk"
                onCommitted: t => Config.options.wallpaper.darkPath = t.trim().replace(/^~/, Quickshell.env("HOME"))
            }
        }
    }

    Group {
        width: parent.width
        title: "Live wallpaper"
        footer: "Plays muted and loops. It pauses while windows cover most of the screen, so it only costs power while you can see it."
        Row {
            label: "Video"
            detail: Wallpaper.liveVideo !== "" && Wallpaper.livePoster === "" ? "Couldn't read this video. Check the path, or install ffmpeg." : "An .mp4, .webm or .mkv file"
            TextField {
                width: 260
                text: Config.options.wallpaper.live
                placeholder: "~/Videos/wallpaper.mp4"
                onCommitted: t => Config.options.wallpaper.live = t.trim().replace(/^~/, Quickshell.env("HOME"))
            }
        }
        Row {
            label: "Pause on battery"
            Switch {
                checked: Config.options.wallpaper.livePauseOnBattery
                onToggled: Config.options.wallpaper.livePauseOnBattery = !checked
            }
        }
    }

    // Login screen (./install.sh --greeter keeps its own copy of the wallpaper)
    Group {
        id: loginGroup
        width: parent.width
        title: "Login screen"
        property bool installed: false
        property string status: ""
        footer: installed ? status : "Install the iRice login screen with ./install.sh --greeter."
        Process {
            running: true
            command: ["test", "-d", "/usr/share/irice"]
            onExited: code => loginGroup.installed = code === 0
        }
        Process {
            id: copyWallpaper
            command: ["pkexec", "cp", Wallpaper.path, "/usr/share/irice/wallpaper.jpg"]
            onExited: code => loginGroup.status = code === 0 ? "The login screen now uses this wallpaper." : "Didn't change the login screen. Approve the password prompt to try again."
        }
        Row {
            visible: loginGroup.installed
            label: "Use this wallpaper on the login screen"
            detail: "Asks for your password to update the login screen's copy."
            Rectangle {
                width: setLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: Theme.color.fill
                Text { id: setLabel; anchors.centerIn: parent; text: "Use wallpaper"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: copyWallpaper.running = true }
            }
        }
    }

    FolderListModel {
        id: folder
        folder: "file://" + Wallpaper.directory
        nameFilters: ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.JPG", "*.PNG"]
        showDirs: false
    }
    FolderListModel {
        id: bundled
        folder: "file://" + Wallpaper.bundledDir
        nameFilters: ["*.jpg"]
        showDirs: false
    }

    Flow {
        width: parent.width
        spacing: 12
        Repeater {
            model: {
                const out = [];
                for (let i = 0; i < folder.count; i++) out.push(folder.get(i, "filePath"));
                for (let i = 0; i < bundled.count; i++) out.push(bundled.get(i, "filePath"));
                return out.filter(p => !!p);
            }
            ClippingRectangle {
                id: thumb
                required property string modelData
                width: (root.width - 24) / 3
                height: width * 9 / 16
                radius: 10
                color: Theme.color.fill
                border.width: Wallpaper.path === modelData ? 3 : 0
                border.color: Theme.accent
                Image {
                    anchors.fill: parent
                    source: "file://" + thumb.modelData
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: Qt.size(320, 180)
                    asynchronous: true
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Wallpaper.set(thumb.modelData)
                }
            }
        }
    }
}
