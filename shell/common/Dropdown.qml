import QtQuick
import QtQuick.Controls as QQC

// Pop-up button: shows the current choice, opens a list of { value, label }.
// The list is a Popup, drawn above the whole window: as a child of the row
// it sat under every row after it, which drew over it and took its clicks.
Squircle {
    id: root

    property var options: []
    property var value
    property string accessibleName
    signal chosen(var value)

    // Keyboard: Space, Enter or Down opens the list; in it the arrows move,
    // Enter chooses, Esc closes. Closed, Up and Down step through the options
    // directly (as macOS pop-up buttons do with the arrow keys and no list).
    activeFocusOnTab: true
    readonly property int currentIndex: options.findIndex(o => o.value === value)
    property int highlighted: -1
    function openMenu() {
        menu.x = root.width - menu.width; // right edges line up (set now: a window popup is placed once, as it opens)
        highlighted = Math.max(0, currentIndex);
        menu.open();
    }
    function step(d) {
        const i = Math.max(0, Math.min(options.length - 1, (currentIndex < 0 ? 0 : currentIndex + d)));
        if (options[i] && options[i].value !== value)
            chosen(options[i].value);
    }
    Keys.onPressed: event => {
        const k = event.key;
        if (k === Qt.Key_Space || k === Qt.Key_Return || k === Qt.Key_Enter || (k === Qt.Key_Down && event.modifiers & Qt.AltModifier)) {
            openMenu();
            event.accepted = true;
        } else if (k === Qt.Key_Up || k === Qt.Key_Down) {
            step(k === Qt.Key_Up ? -1 : 1);
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.ComboBox
    Accessible.name: accessibleName
    Accessible.description: currentLabel
    Accessible.onPressAction: openMenu()
    FocusRing {
        radius: root.radius + 4
    }

    readonly property string currentLabel: options.find(o => o.value === value)?.label ?? String(value ?? "")

    implicitWidth: Math.max(140, label.implicitWidth + 40)
    implicitHeight: 28
    radius: Theme.radius.control - 2
    color: mouse.containsMouse || menu.visible ? Theme.color.fillHover : Theme.color.fill

    Text {
        id: label
        anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
        text: root.currentLabel
        color: Theme.color.label
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }
    Icon {
        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
        name: "caret-up-down"
        bold: true
        size: 11
        color: Theme.color.secondary
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: {
            if (menu.visible)
                return menu.close();
            root.openMenu();
        }
    }

    QQC.Popup {
        id: menu
        y: root.height + 4
        width: Math.max(root.width, 200)
        height: Math.min(280, column.implicitHeight + 10)
        padding: 5
        margins: 12 // stays inside the window (moves up when near the bottom)
        modal: false
        closePolicy: QQC.Popup.CloseOnEscape | QQC.Popup.CloseOnPressOutsideParent
        background: GlassPopupBackground {}
        focus: true
        // Keyboard focus goes back to the button when the list closes
        onClosed: if (root.activeFocusOnTab) root.forceActiveFocus()

        contentItem: Flickable {
            id: flick
            clip: true
            focus: true
            contentHeight: column.implicitHeight
            Accessible.role: Accessible.List
            Keys.onPressed: event => {
                const k = event.key, n = root.options.length;
                if (k === Qt.Key_Up || k === Qt.Key_Down) {
                    root.highlighted = Math.max(0, Math.min(n - 1, root.highlighted + (k === Qt.Key_Up ? -1 : 1)));
                } else if (k === Qt.Key_Home || k === Qt.Key_End) {
                    root.highlighted = k === Qt.Key_Home ? 0 : n - 1;
                } else if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) {
                    const o = root.options[root.highlighted];
                    menu.close();
                    if (o)
                        root.chosen(o.value);
                } else if (k === Qt.Key_Escape) {
                    menu.close();
                } else {
                    return;
                }
                event.accepted = true;
                // keep the highlighted one in view
                const y = root.highlighted * 28;
                if (y < contentY)
                    contentY = y;
                else if (y + 28 > contentY + height)
                    contentY = y + 28 - height;
            }
            Column {
                id: column
                width: menu.availableWidth
                Repeater {
                    model: root.options
                    Squircle {
                        required property var modelData
                        required property int index
                        readonly property bool lit: itemMouse.containsMouse || index === root.highlighted
                        width: column.width
                        height: 28
                        radius: 7
                        color: lit ? Theme.accent : "transparent"
                        Accessible.role: Accessible.ListItem
                        Accessible.name: modelData.label
                        Accessible.selected: modelData.value === root.value
                        Accessible.onPressAction: {
                            menu.close();
                            root.chosen(modelData.value);
                        }
                        Icon {
                            x: 6
                            anchors.verticalCenter: parent.verticalCenter
                            visible: parent.modelData.value === root.value
                            name: "check"
                            bold: true
                            size: 11
                            color: parent.lit ? Theme.color.onAccent : Theme.color.label
                        }
                        Text {
                            x: 24
                            anchors.verticalCenter: parent.verticalCenter
                            text: parent.modelData.label
                            color: parent.lit ? Theme.color.onAccent : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                        MouseArea {
                            id: itemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onEntered: root.highlighted = parent.index
                            onClicked: {
                                menu.close();
                                root.chosen(parent.modelData.value);
                            }
                        }
                    }
                }
            }
            ScrollIndicator {
                target: flick
                x: flick.width - width
                height: flick.height
            }
        }
    }
}
