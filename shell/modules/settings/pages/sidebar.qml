import QtQuick
import qs.common
import qs.modules.settings

Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        title: "Anime"
        footer: "Images come from yande.re, Konachan or Danbooru. Explicit results are off unless you switch them on."
        Row {
            label: "Show Anime tab"
            Switch { checked: Config.options.sidebar.booru; onToggled: Config.options.sidebar.booru = !checked }
        }
        Row {
            label: "Allow explicit results"
            Switch { checked: Config.options.sidebar.booruNsfw; onToggled: Config.options.sidebar.booruNsfw = !checked }
        }
    }

    Group {
        width: parent.width
        title: "GIFs"
        footer: "The emoji picker's GIF tab (Super+.) uses KLIPY. Get a free app key at partner.klipy.com/api-keys and paste it here."
        Row {
            label: "KLIPY app key"
            detail: Config.options.emoji.klipyKey !== "" ? "Key saved" : "No key"
            TextField {
                width: 240
                secret: true
                placeholder: Config.options.emoji.klipyKey !== "" ? "Replace key" : "Paste key"
                onCommitted: t => {
                    if (t.trim() !== "") {
                        Config.options.emoji.klipyKey = t.trim();
                        text = "";
                    }
                }
            }
        }
        Row {
            label: "Content filter"
            Segmented {
                width: 260
                readonly property var values: ["off", "low", "medium", "high"]
                options: ["Off", "Low", "Medium", "High"]
                current: Math.max(0, values.indexOf(Config.options.emoji.gifFilter))
                onSelected: i => Config.options.emoji.gifFilter = values[i]
            }
        }
    }
}
