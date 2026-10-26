import QtQuick
import "phosphor.js" as Phosphor

// Phosphor icon by name, e.g. Icon { name: "wifi-high"; fill: true }.
Text {
    id: root

    property string name
    property bool fill: false
    property bool bold: false
    property int size: 16

    text: Phosphor.glyphs[name] ?? ""
    font.family: fill ? "Phosphor-Fill" : bold ? "Phosphor-Bold" : "Phosphor"
    font.pixelSize: size
    color: Theme.color.label
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
}
