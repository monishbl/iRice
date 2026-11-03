import QtQuick
import "phosphor.js" as Phosphor
import "material-symbols.js" as Symbols

// Icon by Phosphor name, e.g. Icon { name: "wifi-high"; fill: true }. In the
// Material look it draws the matching Material Symbols Rounded glyph (filled
// for `fill`, as Material marks a selected icon; tools/material-symbols-map.txt),
// and the Phosphor one where there's no match.
Text {
    id: root

    property string name
    property bool fill: false
    property bool bold: false
    property int size: 16

    readonly property string symbol: Theme.material ? (Symbols.glyphs[name] ?? "") : ""
    text: symbol !== "" ? symbol : (Phosphor.glyphs[name] ?? "")
    font.family: symbol !== "" ? "iRice Symbols" : fill ? "Phosphor-Fill" : bold ? "Phosphor-Bold" : "Phosphor"
    // (Material Symbols are drawn a little larger: their glyphs sit inside a
    // padded square, Phosphor's fill it)
    font.pixelSize: symbol !== "" ? Math.round(size * 1.2) : size
    font.variableAxes: symbol !== "" ? { "FILL": fill ? 1 : 0, "wght": bold ? 600 : 400, "opsz": Math.max(20, Math.min(48, size * 1.2)), "GRAD": 0 } : {}
    color: Theme.color.label
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
}
