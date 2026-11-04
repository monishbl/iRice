import QtQuick

// One service's wish for an tessera-events source (services/Events), kept on
// while `active`. `source` is a source's name ("udev", "pulse"…) with its
// `args` ({ subsystems: ["usb"] }), or "inhibit:<what>" for a logind
// inhibitor held while active (args: { why }).
QtObject {
    id: w

    property string source: ""
    property bool active: true
    property var args: ({})
    property int wid: 0

    function update() {
        if (wid > 0)
            Events.want(wid, source, active, args);
    }
    onActiveChanged: update()
    onArgsChanged: update()
    onSourceChanged: update()
    Component.onCompleted: {
        wid = Events.join();
        update();
    }
    Component.onDestruction: if (wid > 0) Events.want(wid, source, false, null)
}
