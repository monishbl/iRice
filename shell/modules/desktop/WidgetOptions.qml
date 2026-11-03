import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// A widget's options, from its right-click menu (Widget Options…): what the
// widget shows, set on the widget instead of in Settings. Which widget:
// Panels.widgetOptionsKind.
//   photos      the folder its pictures come from
//   worldClock  the cities
//   stocks      the symbols
//   stack       Smart Rotate (every Smart Stack)
//   system      how resource graphs are drawn (also the island, the menu
//               bar's popovers and Control Center)
// Enter or Escape closes (a field keeps what's typed: it's saved as it
// loses focus).
Variants {
    model: Quickshell.screens

    component Label: Text {
        width: parent?.width ?? 0
        wrapMode: Text.Wrap
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
    }
    component Hint: Text {
        width: parent?.width ?? 0
        wrapMode: Text.Wrap
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    PanelLoader {
        id: slot
        required property var modelData
        panel: "widgetOptions"
        screen: modelData

        Dialog {
            id: dialog
            screen: slot.modelData
            dialogName: "widgetOptions"
            contentWidth: 420
            padding: Theme.material ? 24 : 20

            readonly property string kind: Panels.widgetOptionsKind
            readonly property var cfg: Config.options.desktop
            readonly property string title: ({ photos: "Photos", worldClock: "World Clock", stocks: "Stocks", stack: "Widget Stack", system: "System" })[kind] ?? "Widget"
            onShownChanged: if (shown) Qt.callLater(() => (field.visible ? field : done).forceActiveFocus())

            Column {
                width: parent.width
                spacing: 16

                Text {
                    text: dialog.title + " options"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.material ? Theme.font.large : Theme.font.title; weight: Theme.material ? Font.Normal : Font.DemiBold }
                }

                // What the widget shows, typed in (photos, world clock, stocks)
                Column {
                    visible: field.visible
                    width: parent.width
                    spacing: 8
                    Label {
                        text: dialog.kind === "photos" ? "Folder" : dialog.kind === "worldClock" ? "Cities" : "Symbols"
                    }
                    Row {
                        width: parent.width
                        spacing: 8
                        TextField {
                            id: field
                            visible: dialog.kind === "photos" || dialog.kind === "worldClock" || dialog.kind === "stocks"
                            width: parent.width - (choose.visible ? choose.width + 8 : 0)
                            accessibleName: dialog.kind === "photos" ? "Photos folder" : dialog.kind === "worldClock" ? "World clock cities" : "Stock symbols"
                            placeholder: dialog.kind === "photos" ? "~/Pictures" : dialog.kind === "worldClock" ? "Mumbai|Asia/Kolkata, London|Europe/London" : "AAPL, ^GSPC, BTC-USD"
                            text: dialog.kind === "photos" ? dialog.cfg.photosFolder : dialog.kind === "worldClock" ? dialog.cfg.worldClocks.join(", ") : dialog.cfg.stocks.join(", ")
                            onCommitted: t => {
                                if (dialog.kind === "photos")
                                    dialog.cfg.photosFolder = t.trim().replace(/^~/, Quickshell.env("HOME"));
                                else if (dialog.kind === "worldClock")
                                    dialog.cfg.worldClocks = t.split(",").map(x => x.trim()).filter(x => x !== "");
                                else if (dialog.kind === "stocks")
                                    dialog.cfg.stocks = t.split(",").map(x => x.trim().toUpperCase()).filter(x => x !== "");
                            }
                        }
                        // The photos folder: the file chooser (this closes for it)
                        IconButton {
                            id: choose
                            visible: dialog.kind === "photos"
                            anchors.verticalCenter: parent.verticalCenter
                            width: 36
                            height: 36
                            icon: "folder-open"
                            iconSize: 18
                            tooltip: "Choose the folder"
                            onClicked: {
                                Panels.close();
                                FilePicker.pick({ folder: true, title: "Pictures for the Photos widget", start: Config.options.desktop.photosFolder || "~/Pictures" }, p => Config.options.desktop.photosFolder = p);
                            }
                        }
                    }
                    Hint {
                        text: dialog.kind === "photos" ? "A picture from this folder, changing every 15 minutes. Leave it empty for ~/Pictures."
                            : dialog.kind === "worldClock" ? "City|Area/Zone, separated by commas."
                            : "Symbols separated by commas, as on Yahoo Finance (e.g. RELIANCE.NS)."
                    }
                }

                // Smart Stacks
                Item {
                    visible: dialog.kind === "stack"
                    width: parent.width
                    height: Math.max(rotateText.implicitHeight, rotateSwitch.height)
                    Column {
                        id: rotateText
                        anchors { left: parent.left; right: rotateSwitch.left; rightMargin: 16; verticalCenter: parent.verticalCenter }
                        spacing: 2
                        Label { text: "Smart Rotate" }
                        Hint { text: "Brings forward what matters now: music, the next event, a low battery. For every Widget Stack." }
                    }
                    Switch {
                        id: rotateSwitch
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        checked: Config.options.desktop.smartRotate
                        onToggled: Config.options.desktop.smartRotate = !checked
                    }
                }

                // System: resource graphs
                Column {
                    visible: dialog.kind === "system"
                    width: parent.width
                    spacing: 8
                    Label { text: "Graphs" }
                    Segmented {
                        width: Math.min(parent.width, 280)
                        accessibleName: "Graphs"
                        options: ["Area", "Bars", "Line"]
                        current: Math.max(0, ["area", "bars", "line"].indexOf(Config.options.resources.graphStyle))
                        onSelected: i => Config.options.resources.graphStyle = ["area", "bars", "line"][i]
                    }
                    Hint { text: "Also how the island, the menu bar's popovers and Quick Settings draw CPU, memory and network." }
                }

                Item {
                    width: parent.width
                    height: done.height
                    SettingsLikeButton {
                        id: done
                        anchors.right: parent.right
                        text: "Done"
                        onActivated: {
                            if (field.visible && field.editing)
                                field.committed(field.text);
                            Panels.close();
                        }
                    }
                }
            }
        }
    }

    // A filled button: verb + object, 36 px (Material: a 40 px pill)
    component SettingsLikeButton: Rectangle {
        id: button
        property string text
        signal activated
        implicitWidth: label.implicitWidth + (Theme.material ? 48 : 32)
        implicitHeight: Theme.material ? 40 : 36
        radius: Theme.material ? height / 2 : Theme.radius.control
        color: Theme.material ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
            : mouse.pressed ? Qt.darker(Theme.accent, 1.15) : mouse.containsMouse ? Qt.lighter(Theme.accent, 1.08) : Theme.accent
        activeFocusOnTab: true
        Keys.onReturnPressed: button.activated()
        Keys.onEnterPressed: button.activated()
        Keys.onSpacePressed: button.activated()
        Accessible.role: Accessible.Button
        Accessible.name: text
        Accessible.onPressAction: button.activated()
        FocusRing {
            radius: button.radius + 4
        }
        Text {
            id: label
            anchors.centerIn: parent
            text: button.text
            color: Theme.material ? Theme.m3.onPrimary : Theme.color.onAccent
            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold }
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: button.activated()
        }
    }
}
