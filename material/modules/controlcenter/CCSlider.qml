import QtQuick
import qs.common

// Tahoe-style slider: a thick rounded track that fills with the label colour,
// icon inside at the left. Drag, click or scroll to change. Material: the
// Expressive slider at its medium size (QSSlider), the icon in its track.
Item {
    id: root

    property string label
    property string icon
    property real value: 0
    property bool hasMore: false
    signal moved(real value)
    signal more

    implicitHeight: (label !== "" ? title.implicitHeight + 8 : 0) + 48

    Icon {
        visible: root.hasMore
        anchors { right: parent.right; top: parent.top }
        name: "caret-right"
        bold: true
        size: 12
        color: Theme.color.secondary
        MouseArea {
            anchors { fill: parent; margins: -8 }
            cursorShape: Qt.PointingHandCursor
            onClicked: root.more()
        }
    }

    Text {
        id: title
        visible: root.label !== ""
        text: root.label
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
    }

    Loader {
        active: Theme.material
        visible: active
        anchors.bottom: parent.bottom
        width: parent.width
        height: 48
        sourceComponent: QSSlider {
            trackHeight: 40
            icon: root.icon
            accessibleName: root.label !== "" ? root.label : root.icon
            value: root.value
            onMoved: v => root.moved(v)
        }
    }

}
