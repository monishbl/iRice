import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services
import "../../../services/StartupSounds.js" as StartupSounds

// Every sound to log in to, by the style whose instrument plays it; any of
// them whichever style is on (StartupSounds.js). Choosing one also drops a
// file of your own.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22
    readonly property string pageTitle: "Sound when you log in"
    readonly property string current: Config.options.sounds.startupFile !== "" ? "file" : Config.options.sounds.startupSound

    function use(id) {
        Config.options.sounds.startupSound = id;
        Config.options.sounds.startupFile = "";
        Sounds.play("startup", true);
    }

    component Choice: Row {
        id: choice
        required property var modelData
        readonly property bool inUse: page.current === modelData.id
        label: modelData.name
        detail: modelData.about
        QQ.Row {
            spacing: 8
            SettingsButton {
                text: "Play"
                icon: "play"
                onClicked: Sounds.playFile(choice.modelData.id === "" ? Sounds.own + "startup.ogg" : Quickshell.shellDir + "/assets/sounds/startup/" + choice.modelData.id + ".ogg", Config.options.sounds.startupVolume)
            }
            SettingsButton {
                text: choice.inUse ? "In use" : "Use"
                icon: choice.inUse ? "check" : ""
                primary: !choice.inUse
                enabled: !choice.inUse
                onClicked: page.use(choice.modelData.id)
            }
        }
    }

    Group {
        width: parent.width
        title: "Original"
        footer: "Any of these can play when you log in, whichever sound style you use."
        Choice { modelData: StartupSounds.all[0] }
    }
    Repeater {
        model: StartupSounds.styles
        Group {
            id: styleGroup
            required property var modelData
            width: page.width
            title: modelData.name
            Repeater {
                model: StartupSounds.all.filter(c => c.style === styleGroup.modelData.name)
                Choice {}
            }
        }
    }
}
