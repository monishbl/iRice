import QtQuick

// The desktop widget catalogue and the grid it snaps to. Shared by the
// desktop and the gallery so they always agree.
QtObject {
    readonly property int cell: 164 // a small widget is cell × cell
    readonly property int gap: 16
    readonly property var units: ({ s: [1, 1], m: [2, 1], l: [2, 2] })
    readonly property var sizeNames: ({ s: "Small", m: "Medium", l: "Large" })

    readonly property var kinds: [
        { id: "clock", file: "ClockWidget.qml", label: "Clock", icon: "clock", desc: "An analog face, or the time and date.", sizes: ["s", "m"] },
        { id: "worldClock", file: "WorldClockWidget.qml", label: "World Clock", icon: "globe-hemisphere-west", desc: "The time in other cities. Choose them in Settings → Desktop.", sizes: ["s", "m"] },
        { id: "weather", file: "WeatherWidget.qml", label: "Weather", icon: "cloud-sun", desc: "Conditions where you are, and the next hours.", sizes: ["s", "m"] },
        { id: "calendar", file: "CalendarWidget.qml", label: "Calendar", icon: "calendar-blank", desc: "Today, the month, and what's coming up.", sizes: ["s", "m", "l"] },
        { id: "reminders", file: "RemindersWidget.qml", label: "Reminders", icon: "list-checks", desc: "Your to-dos. Tick them off right on the desktop.", sizes: ["s", "m", "l"] },
        { id: "batteries", file: "BatteriesWidget.qml", label: "Batteries", icon: "battery-high", desc: "This computer and connected Bluetooth devices.", sizes: ["s", "m"] },
        { id: "nowPlaying", file: "NowPlayingWidget.qml", label: "Now Playing", icon: "music-notes", desc: "What's playing, with controls.", sizes: ["s", "m"] },
        { id: "system", file: "SystemWidget.qml", label: "System", icon: "cpu", desc: "CPU and memory; medium adds temperature and swap.", sizes: ["s", "m"] },
        { id: "photos", file: "PhotosWidget.qml", label: "Photos", icon: "image", desc: "A picture from your Pictures folder, changing every 15 minutes.", sizes: ["s", "m", "l"], bleed: true },
        { id: "notes", file: "NotesWidget.qml", label: "Note", icon: "note-pencil", desc: "A note you can type into on the desktop.", sizes: ["s", "m", "l"] }
    ]
    function kind(id) {
        return kinds.find(k => k.id === id) ?? null;
    }
    function pixelSize(size) {
        const u = units[size] ?? units.s;
        return Qt.size(u[0] * cell + (u[0] - 1) * gap, u[1] * cell + (u[1] - 1) * gap);
    }

    // "kind:size:col:row" <-> { kind, size, col, row }
    function parse(entry) {
        const [kind, size, col, row] = entry.split(":");
        return { kind: kind, size: units[size] ? size : "s", col: Number(col) || 0, row: Number(row) || 0 };
    }
    function format(w) {
        return w.kind + ":" + w.size + ":" + w.col + ":" + w.row;
    }
}
