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
            label: "Picture"
            detail: "Any image on your computer, or pick one below"
            SettingsButton {
                text: FilePicker.busy ? "Choosing…" : "Choose Picture…"
                enabled: !FilePicker.busy
                onClicked: FilePicker.pick({ title: "Choose a wallpaper", start: "~/Pictures", filter: "Images|*.jpg *.jpeg *.png *.webp" }, p => Wallpaper.set(p))
            }
        }
        Row {
            label: "Folder"
            detail: "Images in this folder appear below and in the wallpaper picker."
            PathField {
                folder: true
                value: Config.options.wallpaper.directory
                placeholder: "~/Pictures/Wallpapers"
                start: "~/Pictures"
                onPicked: p => Config.options.wallpaper.directory = p
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
                            source: Wallpaper.fileUrl(setTile.modelData.preview)
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(300, 180)
                            asynchronous: true
                        }
                        Image {
                            visible: setTile.modelData.kind === "pair"
                            x: parent.width / 2
                            width: parent.width / 2
                            height: parent.height
                            source: setTile.modelData.kind === "pair" ? Wallpaper.fileUrl(setTile.modelData.dark) : ""
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
            PathField {
                folder: true
                value: Config.options.wallpaper.dynamicFolder
                placeholder: "Built-in Tahoe set"
                start: "~/Pictures"
                onPicked: p => Config.options.wallpaper.dynamicFolder = p
            }
        }
        Row {
            visible: Config.options.wallpaper.dynamic === "appearance"
            label: "Light picture"
            PathField {
                value: Config.options.wallpaper.lightPath
                placeholder: "Built-in Tahoe day"
                filter: "Images|*.jpg *.jpeg *.png *.webp"
                start: "~/Pictures"
                onPicked: p => Config.options.wallpaper.lightPath = p
            }
        }
        Row {
            visible: Config.options.wallpaper.dynamic === "appearance"
            label: "Dark picture"
            PathField {
                value: Config.options.wallpaper.darkPath
                placeholder: "Built-in Tahoe dusk"
                filter: "Images|*.jpg *.jpeg *.png *.webp"
                start: "~/Pictures"
                onPicked: p => Config.options.wallpaper.darkPath = p
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
            PathField {
                value: Config.options.wallpaper.live
                placeholder: "~/Videos/wallpaper.mp4"
                filter: "Videos|*.mp4 *.webm *.mkv *.mov"
                start: "~/Videos"
                onPicked: p => Config.options.wallpaper.live = p
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
        property bool passwordless: false // iRice's root helper is installed: no password
        property string status: ""
        footer: installed ? status : "Install the iRice login screen with ./install.sh --greeter."
        Process {
            running: true
            command: ["test", "-d", "/usr/share/irice"]
            onExited: code => loginGroup.installed = code === 0
        }
        Process {
            running: true
            command: ["test", "-x", "/usr/lib/irice/irice-root"]
            onExited: code => loginGroup.passwordless = code === 0
        }
        // Through iRice's root helper (it checks it's an image, reads it as
        // you), or before an update installed that, cp with the password
        Process {
            id: copyWallpaper
            command: loginGroup.passwordless ? ["pkexec", "/usr/lib/irice/irice-root", "wallpaper", Wallpaper.path]
                : ["pkexec", "cp", Wallpaper.path, "/usr/share/irice/wallpaper.jpg"]
            onExited: code => loginGroup.status = code === 0 ? "The login screen now uses this wallpaper."
                : loginGroup.passwordless ? "Didn't change the login screen. Pick a JPEG, PNG or WebP wallpaper and try again."
                : "Didn't change the login screen. Approve the password prompt to try again."
        }
        Row {
            visible: loginGroup.installed
            label: "Use this wallpaper on the login screen"
            detail: loginGroup.passwordless ? "Copies it to the login screen." : "Asks for your password to update the login screen's copy."
            Rectangle {
                width: setLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: Theme.color.fill
                Text { id: setLabel; anchors.centerIn: parent; text: "Use wallpaper"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: {
                        // The dev sandbox must not change the real login screen
                        if (System.sandboxed) {
                            loginGroup.status = "Sandbox: would copy this wallpaper to the login screen.";
                            return;
                        }
                        copyWallpaper.running = true;
                    } }
            }
        }
    }

    FolderListModel {
        id: folder
        folder: Wallpaper.fileUrl(Wallpaper.directory)
        nameFilters: ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.JPG", "*.PNG"]
        showDirs: false
    }
    FolderListModel {
        id: bundled
        folder: Wallpaper.fileUrl(Wallpaper.bundledDir)
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
                    source: Wallpaper.fileUrl(thumb.modelData)
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
