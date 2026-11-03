import QtQuick
import QtQuick as QQ
import qs.common

// A row that opens a subpage (Settings navigation: back arrow, breadcrumb):
//   SubpageLink { label: "Glass"; detail: "Material, lens and frost"; page: "appearance/glass" }
// `page` is "<section>/<subpage>", the file pages/<section>-<subpage>.qml,
// or another page, for a setting that lives there (each setting has one
// home): `target` names its row, which Settings scrolls to and flashes.
Item {
    id: root

    property string label
    property string detail
    property string icon
    property string value // a short summary on the right, e.g. "Liquid"
    required property string page
    property string target // (a row or group on that page to show)
    function go() {
        Panels.settingsPage = page;
        if (target !== "")
            Panels.settingsTarget = target;
    }

    width: parent?.width ?? 0
    implicitHeight: Math.max(44, texts.implicitHeight + 20)

    Squircle {
        anchors.fill: parent
        anchors.margins: 2
        radius: 8
        color: mouse.pressed ? Theme.color.fillHover : mouse.containsMouse ? Theme.color.fill : "transparent"
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }
    Icon {
        id: lead
        visible: root.icon !== ""
        anchors { left: parent.left; leftMargin: 14; verticalCenter: parent.verticalCenter }
        name: root.icon
        size: 18
        color: Theme.color.secondary
    }
    Column {
        id: texts
        anchors { left: lead.visible ? lead.right : parent.left; leftMargin: lead.visible ? 10 : 14; right: tail.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 2
        Text {
            width: parent.width
            text: root.label
            wrapMode: Text.Wrap
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        Text {
            width: parent.width
            visible: root.detail !== ""
            text: root.detail
            wrapMode: Text.Wrap
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }
    QQ.Row {
        id: tail
        anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 6
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.value !== ""
            text: root.value
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: "caret-right"
            bold: true
            size: 12
            color: Theme.color.tertiary
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.go()
    }
    Keys.onReturnPressed: root.go()
    activeFocusOnTab: true
}
