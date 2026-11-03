import QtQuick
import qs.common

// A Settings slider with a word under each end ("Slow" … "Fast"):
//   WordSlider { from: 0; to: 1; low: "Slow"; high: "Fast"; value: x; onMoved: v => x = v }
Column {
    id: root

    property alias from: slider.from
    property alias to: slider.to
    property alias value: slider.value
    property string low
    property string high
    signal moved(real v)

    width: 240
    spacing: 2
    Slider {
        id: slider
        width: parent.width
        onMoved: v => root.moved(v)
    }
    Item {
        width: parent.width
        height: lowText.implicitHeight
        Text { id: lowText; text: root.low; color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption } }
        Text { anchors.right: parent.right; text: root.high; color: Theme.color.tertiary; font { family: Theme.font.family; pixelSize: Theme.font.caption } }
    }
}
