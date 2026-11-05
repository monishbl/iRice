import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Services.UPower
import qs.common
import qs.services

// While tessera-ui draws the desktop (rust.surfaces: desktop), what only
// this shell knows reaches it here, one line on its stdin (RustUi.tell):
// the weather, this month's events, to-dos, Screen Time, the player, the
// batteries Bluetooth reports, Spotlight's actions (the Shortcuts widget),
// the notes' text, each stack's and Shortcuts widget's choices, Edit
// Widgets, Today's greeting. And what it asks of this shell (RustUi's
// "desktop" call) is done here: ticking a to-do, the media keys, a
// shortcut, saving a note, opening Notification Center, Today, Widget
// Options, the wallpaper picker, Quick Preview, Share.
// (native/tessera-ui/src/surfaces/desktop/feed.rs reads what this sends)
Item {
    id: root

    required property var screen

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }
    readonly property date monthFirst: new Date(clock.date.getFullYear(), clock.date.getMonth(), 1)
    readonly property date monthNext: new Date(clock.date.getFullYear(), clock.date.getMonth() + 1, 1)
    function ev(e) {
        return { title: e.title ?? "", color: String(e.color ?? ""), allDay: !!e.allDay, start: e.start.getTime(), end: e.end.getTime() };
    }
    readonly property var batteries: {
        const l = [];
        if (Island.hasBattery)
            l.push({ icon: Island.charging ? "lightning" : "laptop", picture: "", value: Island.level, name: "This computer" });
        for (const d of Bluetooth.devices.values) {
            if (!d.connected)
                continue;
            const parts = FastPair.levelsFor(d.address);
            if (parts.length > 0) {
                for (const p of parts)
                    l.push({ icon: Devices.btIcon(d), picture: Devices.pictureOf(d), value: p.value, name: ({ L: "Left", R: "Right", Case: "Case" })[p.label] ?? p.label });
            } else if (d.batteryAvailable) {
                l.push({ icon: Devices.btIcon(d), picture: Devices.pictureOf(d), value: d.battery, name: d.name || d.deviceName });
            }
        }
        return l.slice(0, 4);
    }
    readonly property var feed: {
        Calendar.local;
        Calendar.remote;
        const busy = Calendar.busyDays(monthFirst, new Date(monthNext.getTime() - 86400000));
        for (const t of Todos.list)
            if (t.due && !t.done) {
                const d = new Date(t.due);
                busy[d.getFullYear() + "-" + d.getMonth() + "-" + d.getDate()] = true;
            }
        const dev = UPower.displayDevice;
        return {
            editing: Panels.widgetEditing && Panels.screen === root.screen,
            editScreen: root.screen?.name ?? "",
            greeting: Today.greeting(clock.date),
            weather: { ready: Weather.ready, error: Weather.error, place: Weather.place, temp: Weather.temp, unit: Weather.unit, high: Weather.high, low: Weather.low, condition: Weather.condition, icon: Weather.icon, hourly: Weather.hourly.map(h => ({ time: String(h.time), temp: h.temp, icon: h.icon })), rainChance: Weather.rainChance },
            events: Calendar.between(monthFirst, monthNext).map(ev),
            busy: Object.keys(busy),
            upcoming: Calendar.upcoming(1).map(ev),
            todos: Todos.list.map(t => ({ key: t.key, text: t.text, done: !!t.done, due: t.due || 0, dueLabel: Todos.dueLabel(t), overdue: Todos.overdue(t) })),
            screenTime: { total: ScreenTime.todayTotal, week: ScreenTime.week, apps: ScreenTime.todayApps.slice(0, 3).map(a => ({ id: a.id, name: Apps.entryFor(a.id)?.name ?? a.id, icon: String(Apps.iconFor(Apps.entryFor(a.id) ?? a.id)), seconds: a.seconds })) },
            media: { has: !!Media.player, playing: !!Media.player?.isPlaying, title: Media.player?.trackTitle ?? "", artist: Media.player?.trackArtist ?? "", art: String(Media.artUrl(Media.player) ?? "") },
            batteries: root.batteries,
            power: { has: Island.hasBattery, level: dev ? (dev.percentage > 1 ? dev.percentage / 100 : dev.percentage) : 1, charging: dev?.state === UPowerDeviceState.Charging, onBattery: UPower.onBattery },
            actions: Search.actions.map(a => ({ title: a.title, glyph: a.glyph ?? "" })),
            notes: Object.assign({ "": UiState.data.desktopNote ?? "" }, UiState.data.desktopNotes ?? {}),
            stacks: UiState.data.stacks ?? {},
            shortcutButtons: UiState.data.shortcutButtons ?? {}
        };
    }
    function send() {
        RustUi.tell("desktop", { feed: root.feed });
    }
    onFeedChanged: later.restart()
    Timer {
        id: later
        interval: 150
        onTriggered: root.send()
    }
    Component.onCompleted: later.restart()
    Connections {
        target: RustUi
        // (sent again when tessera-ui starts, or starts drawing the desktop)
        function onDrawnChanged() {
            later.restart();
        }
        function onDesktopCall(what, a) {
            root.call(what, a);
        }
    }
    // The gallery (and Today's Pin) adds widgets to the desktop drawn there
    Connections {
        target: Panels
        function onAddWidget(kind, size) {
            if (Panels.screen === root.screen)
                RustUi.tell("desktop", { add: [kind, size] });
        }
    }

    function call(what, a) {
        switch (what) {
        case "todo": {
            const t = Todos.list.find(x => x.key === a[0]);
            if (t)
                Todos.update(t, { done: !t.done });
            break;
        }
        case "todoAdd":
            if (String(a[0] ?? "").trim() !== "")
                Todos.add(String(a[0]));
            break;
        case "media":
            if (a[0] === "previous")
                Media.player?.previous();
            else if (a[0] === "next")
                Media.player?.next();
            else
                Media.player?.togglePlaying();
            break;
        case "shortcut":
            Search.actions.find(x => x.title === a[0])?.run();
            break;
        case "note":
            if (a[0] === "")
                UiState.data.desktopNote = String(a[1] ?? "");
            else
                UiState.data.desktopNotes = Object.assign({}, UiState.data.desktopNotes, { [a[0]]: String(a[1] ?? "") });
            break;
        case "stacks":
            UiState.data.stacks = Object.assign({}, UiState.data.stacks, { [a[0]]: a[1] ?? [] });
            break;
        case "buttons":
            UiState.data.shortcutButtons = Object.assign({}, UiState.data.shortcutButtons, { [a[0]]: a[1] ?? [] });
            break;
        case "calendar":
            Panels.ncTab = 0;
            Panels.show("notificationCenter", Quickshell.screens[0]);
            break;
        case "today":
            if (a[0] === true)
                Today.optionsOpen = true;
            Today.show("");
            break;
        case "options":
            Panels.widgetOptionsKind = String(a[0] ?? "");
            Panels.show("widgetOptions", root.screen);
            break;
        case "edit":
            Panels.close();
            Panels.screen = root.screen;
            Panels.widgetEditing = true;
            break;
        case "endEdit":
            Panels.widgetEditing = false;
            break;
        case "noRoom":
            Island.post({ kind: "custom", icon: "squares-four", title: "No room on the desktop", subtitle: "Remove or shrink a widget first", color: Theme.accents.orange });
            break;
        case "show":
            if (a[0] === "wallpaper")
                Panels.show("wallpaper", root.screen);
            break;
        case "open":
            Files.open(String(a[0]));
            break;
        case "reveal":
            Files.reveal(String(a[0]));
            break;
        case "quickLook":
            Panels.quickLook(String(a[0]));
            break;
        case "share":
            Panels.share(a[0] ?? [], root.screen);
            break;
        case "sound":
            if (a[0] === "trash")
                Sounds.play("trash");
            break;
        }
    }
}
