import QtQuick
import qs.common

// Another page's settings shown as a part of this one (a merged page: Menu
// Bar & Dock, Accounts…), under a heading of its own. The part is that
// page's file, loaded as it is; SettingsMap.parts makes search find its rows
// here, and an older link to that page scrolls to the heading (`title`).
//   PagePart { title: "Dock"; source: Qt.resolvedUrl("dock.qml") }
Column {
    id: root

    property string title // (also what Settings' reveal() finds)
    property url source

    width: parent?.width ?? 0
    spacing: Theme.material ? 12 : 10

    Text {
        visible: root.title !== ""
        topPadding: Theme.material ? 16 : 12
        leftPadding: Theme.material ? 0 : 4
        text: root.title
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.material ? 22 : Theme.font.title; weight: Theme.material ? Font.Normal : Font.DemiBold }
        Accessible.role: Accessible.Heading
        Accessible.name: text
    }
    Loader {
        width: root.width
        source: root.source
    }
}
