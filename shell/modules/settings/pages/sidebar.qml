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
}
