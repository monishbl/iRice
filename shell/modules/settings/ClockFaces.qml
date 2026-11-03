import QtQuick
import qs.common
import qs.modules.lock as LockUi
import qs.services

// Settings → Lock Screen in the Material look: the lock clock's style
// (lock.clockFace), Android 17's faces picked from a carousel, each drawn
// live over the wallpaper. Loaded only in that look (lock.qml).
Item {
    id: carousel
    height: 212
    readonly property var faces: [
        { id: "dynamic", name: "Dynamic" },
        { id: "stacked", name: "Stacked" },
        { id: "wide", name: "Wide" },
        { id: "overlap", name: "Overlap" },
        { id: "analog", name: "Analog" },
        { id: "weather", name: "Weather" }
    ]
    readonly property int chosen: Math.max(0, faces.findIndex(f => f.id === Config.options.lock.clockFace))

    ListView {
        id: faceList
        anchors.fill: parent
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        orientation: ListView.Horizontal
        spacing: 8
        clip: true
        model: carousel.faces
        // (the chosen one centred, its neighbours peeking in)
        readonly property real itemWidth: 176
        highlightRangeMode: ListView.StrictlyEnforceRange
        preferredHighlightBegin: (width - itemWidth) / 2
        preferredHighlightEnd: (width + itemWidth) / 2
        highlightMoveDuration: Theme.motion.panel
        snapMode: ListView.SnapToItem
        currentIndex: carousel.chosen
        // (a swipe picks the one it settles on)
        onMovementEnded: Config.options.lock.clockFace = carousel.faces[currentIndex].id
        activeFocusOnTab: true
        Keys.onLeftPressed: Config.options.lock.clockFace = carousel.faces[Math.max(0, carousel.chosen - 1)].id
        Keys.onRightPressed: Config.options.lock.clockFace = carousel.faces[Math.min(carousel.faces.length - 1, carousel.chosen + 1)].id
        Accessible.role: Accessible.List
        Accessible.name: "Clock style: " + carousel.faces[carousel.chosen].name

        delegate: Item {
            id: card
            required property var modelData
            required property int index
            readonly property bool isChosen: index === carousel.chosen
            // (smaller and fainter the further from the centre)
            readonly property real off: Math.abs(x + width / 2 - faceList.contentX - faceList.width / 2) / faceList.itemWidth
            width: faceList.itemWidth
            height: faceList.height
            scale: Math.max(0.84, 1 - off * 0.16)
            opacity: Math.max(0.55, 1 - off * 0.45)

            Rectangle {
                id: frame
                anchors.fill: parent
                anchors.margins: 6 // (room for the ring)
                radius: 28
                clip: true
                color: Theme.m3.surfaceContainerHighest
                Image {
                    anchors.fill: parent
                    source: Wallpaper.source
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: Qt.size(352, 376)
                    asynchronous: true
                    opacity: 0.85
                }
                Rectangle {
                    anchors.fill: parent
                    color: Qt.alpha("black", 0.28)
                }
                LockUi.LockClock {
                    anchors.centerIn: parent
                    anchors.verticalCenterOffset: -12
                    width: implicitWidth
                    height: implicitHeight
                    face: card.modelData.id
                    time: new Date()
                    size: card.modelData.id === "analog" ? 64 : card.modelData.id === "overlap" ? 54 : card.modelData.id === "dynamic" ? 50 : 58
                    weight: Config.options.lock.clockWeight
                    widthAxis: Config.options.lock.clockWidth
                    round: Config.options.lock.clockRound
                    color: Theme.m3.primaryFixed
                    color2: Theme.m3.primaryFixedDim
                    color3: Theme.m3.tertiaryFixed
                }
                Text {
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 14 }
                    text: card.modelData.name
                    color: "white"
                    font { family: Theme.font.family; pixelSize: 13; weight: Font.Medium }
                }
            }
            Rectangle {
                anchors.fill: frame
                anchors.margins: -4
                radius: frame.radius + 4
                color: "transparent"
                border.width: 3
                border.color: Theme.m3.primary
                visible: card.isChosen
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: Config.options.lock.clockFace = card.modelData.id
            }
            Accessible.role: Accessible.ListItem
            Accessible.name: card.modelData.name
            Accessible.selected: card.isChosen
        }
    }
}
