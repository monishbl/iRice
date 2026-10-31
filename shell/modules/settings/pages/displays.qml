import QtQuick
import QtQuick as QQ
import qs.common
import qs.modules.settings
import qs.services

// Displays: arrangement (drag to place, edges snap together), then the
// chosen display's resolution, refresh rate, scale, rotation and mirroring.
// services/Displays.qml applies and saves; risky changes ask to be kept.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    Component.onCompleted: Displays.refresh()

    readonly property var mons: Displays.monitors
    property string selected: ""
    readonly property var current: mons.find(m => m.name === selected) ?? mons.find(m => m.enabled) ?? mons[0] ?? null
    readonly property var placed: mons.filter(m => m.enabled && !m.mirror) // shown in the arrangement
    readonly property int enabledCount: mons.filter(m => m.enabled).length

    // ── Keep these settings? ─────────────────────────────────────────────
    Squircle {
        visible: Displays.confirming
        width: parent.width
        height: visible ? confirmRow.implicitHeight + 24 : 0
        radius: 12
        color: Theme.dark ? Qt.rgba(1, 1, 1, 0.05) : "white"
        border.width: 1
        border.color: Theme.accent

        QQ.Row {
            id: confirmRow
            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; leftMargin: 14; rightMargin: 14 }
            spacing: 12
            Column {
                width: parent.width - buttons.width - 12
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2
                Text {
                    text: "Keep these display settings?"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: "Going back to the previous settings in " + Displays.confirmLeft + (Displays.confirmLeft === 1 ? " second." : " seconds.")
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; features: ({ "tnum": 1 }) }
                }
            }
            QQ.Row {
                id: buttons
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                SettingsButton { text: "Go back"; onClicked: Displays.revert() }
                SettingsButton { text: "Keep settings"; primary: true; onClicked: Displays.keep() }
            }
        }
    }

    // ── Loading / nothing found ──────────────────────────────────────────
    Text {
        visible: page.mons.length === 0
        width: parent.width
        wrapMode: Text.Wrap
        text: Displays.loaded ? "No displays found. Hyprland didn't report any outputs." : "Looking for displays…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.body }
    }

    // ── Arrangement ──────────────────────────────────────────────────────
    Group {
        width: parent.width
        visible: page.mons.length > 0
        footer: page.placed.length > 1 ? "Drag displays to match how they sit on your desk; edges snap together. Click one to change it below." : "Connect another display to arrange them."

        Item {
            id: canvas
            width: parent.width
            height: 220

            readonly property int pad: 20
            readonly property var box: {
                const ps = page.placed;
                if (ps.length === 0)
                    return { x: 0, y: 0, w: 1, h: 1 };
                let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
                for (const m of ps) {
                    const s = Displays.logicalSize(m);
                    x0 = Math.min(x0, m.x);
                    y0 = Math.min(y0, m.y);
                    x1 = Math.max(x1, m.x + s.w);
                    y1 = Math.max(y1, m.y + s.h);
                }
                return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
            }
            // Leave room to drag a display past the others
            readonly property real k: Math.min((width - 2 * pad) / (box.w * 1.6), (height - 2 * pad) / (box.h * 1.4))
            readonly property real offX: (width - box.w * k) / 2
            readonly property real offY: (height - box.h * k) / 2

            function toCanvasX(x) { return offX + (x - box.x) * k; }
            function toCanvasY(y) { return offY + (y - box.y) * k; }

            // Where a dropped display goes: touching the nearest edge of
            // another, top or bottom (left or right) edges lined up when close
            function snap(name, lx, ly) {
                const me = page.placed.find(m => m.name === name);
                const s = Displays.logicalSize(me);
                const others = page.placed.filter(m => m.name !== name).map(m => {
                    const o = Displays.logicalSize(m);
                    return { x: m.x, y: m.y, w: o.w, h: o.h };
                });
                if (others.length === 0)
                    return { x: me.x, y: me.y };
                const near = 120; // logical px that count as "lined up"
                const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
                const align = (v, a, b) => Math.abs(v - a) < near ? a : Math.abs(v - b) < near ? b : v;
                const overlaps = (x, y) => others.some(o => x < o.x + o.w && x + s.w > o.x && y < o.y + o.h && y + s.h > o.y);
                let best = null;
                for (const o of others) {
                    const ys = align(clamp(ly, o.y - s.h + 1, o.y + o.h - 1), o.y, o.y + o.h - s.h);
                    const xs = align(clamp(lx, o.x - s.w + 1, o.x + o.w - 1), o.x, o.x + o.w - s.w);
                    for (const c of [{ x: o.x + o.w, y: ys }, { x: o.x - s.w, y: ys }, { x: xs, y: o.y + o.h }, { x: xs, y: o.y - s.h }]) {
                        if (overlaps(c.x, c.y))
                            continue;
                        const d = Math.hypot(c.x - lx, c.y - ly);
                        if (!best || d < best.d)
                            best = { x: Math.round(c.x), y: Math.round(c.y), d: d };
                    }
                }
                return best ?? { x: me.x, y: me.y };
            }

            function drop(name, cx, cy) {
                const p = snap(name, (cx - offX) / k + box.x, (cy - offY) / k + box.y);
                const pos = {};
                for (const m of page.placed)
                    pos[m.name] = m.name === name ? p : { x: m.x, y: m.y };
                // Keep the top-left corner at 0,0
                let minX = Infinity, minY = Infinity;
                for (const n in pos) {
                    minX = Math.min(minX, pos[n].x);
                    minY = Math.min(minY, pos[n].y);
                }
                const list = [];
                for (const m of page.placed) {
                    const nx = pos[m.name].x - minX, ny = pos[m.name].y - minY;
                    if (nx !== m.x || ny !== m.y)
                        list.push({ name: m.name, changes: { x: nx, y: ny } });
                }
                Displays.setMany(list);
            }

            Repeater {
                model: page.placed

                Squircle {
                    id: tile
                    required property var modelData
                    readonly property var size: Displays.logicalSize(modelData)
                    readonly property bool chosen: page.current && page.current.name === modelData.name

                    function home() {
                        x = Qt.binding(() => canvas.toCanvasX(tile.modelData.x));
                        y = Qt.binding(() => canvas.toCanvasY(tile.modelData.y));
                    }
                    Component.onCompleted: home()
                    width: size.w * canvas.k
                    height: size.h * canvas.k
                    radius: 6
                    z: dragArea.drag.active ? 2 : 1
                    color: Theme.dark ? Qt.rgba(1, 1, 1, chosen ? 0.16 : 0.08) : Qt.rgba(0, 0, 0, chosen ? 0.1 : 0.05)
                    border.width: chosen ? 2 : 1
                    border.color: chosen ? Theme.accent : Theme.color.separator

                    // Menu bar strip on the one with the focused workspace
                    Squircle {
                        visible: tile.modelData.builtIn
                        x: 0
                        y: 0
                        width: parent.width
                        height: Math.max(3, parent.height * 0.06)
                        radius: 6
                        color: Theme.color.separator
                    }
                    Column {
                        anchors.centerIn: parent
                        width: parent.width - 12
                        spacing: 0
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            text: Displays.numberOf(tile.modelData.name)
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Math.max(13, Math.min(28, tile.height * 0.3)); weight: Font.DemiBold }
                        }
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            elide: Text.ElideRight
                            visible: tile.height > 60
                            text: tile.modelData.label
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                    MouseArea {
                        id: dragArea
                        anchors.fill: parent
                        cursorShape: page.placed.length > 1 ? (pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.ArrowCursor
                        drag.target: page.placed.length > 1 ? tile : null
                        drag.threshold: 4
                        onPressed: page.selected = tile.modelData.name
                        onReleased: {
                            if (dragArea.drag.active || Math.abs(tile.x - canvas.toCanvasX(tile.modelData.x)) > 2 || Math.abs(tile.y - canvas.toCanvasY(tile.modelData.y)) > 2)
                                canvas.drop(tile.modelData.name, tile.x, tile.y);
                            tile.home();
                        }
                    }
                }
            }
        }

        Row {
            label: "Identify"
            detail: "Shows each display's number on it for a moment"
            SettingsButton { text: "Identify Displays"; onClicked: Displays.identify() }
        }
    }

    // ── Which display ────────────────────────────────────────────────────
    Segmented {
        visible: page.mons.length > 1
        width: Math.min(parent.width, 120 * page.mons.length)
        anchors.horizontalCenter: parent.horizontalCenter
        options: page.mons.map(m => Displays.numberOf(m.name) + "  " + m.label)
        current: Math.max(0, page.mons.findIndex(m => page.current && m.name === page.current.name))
        onSelected: i => page.selected = page.mons[i].name
    }

    // ── The chosen display ───────────────────────────────────────────────
    Group {
        id: settingsGroup
        width: parent.width
        visible: page.current !== null
        title: page.current ? page.current.label + " (" + page.current.name + ")" : ""
        footer: !page.current ? "" : page.current.enabled ? page.current.description : "This display is off. Turn it on to change its resolution and arrangement."

        readonly property var m: page.current
        readonly property var resolutions: {
            if (!m)
                return [];
            const seen = {}, out = [];
            for (const md of m.modes) {
                const key = md.w + "x" + md.h;
                if (seen[key])
                    continue;
                seen[key] = true;
                out.push({ value: key, label: md.w + " × " + md.h, area: md.w * md.h });
            }
            out.sort((a, b) => b.area - a.area);
            if (out.length > 0)
                out[0].label += "  (Native)";
            return out;
        }
        readonly property string resolution: m ? m.width + "x" + m.height : ""
        readonly property var rates: {
            if (!m)
                return [];
            return m.modes.filter(md => md.w + "x" + md.h === resolution).sort((a, b) => b.r - a.r).map(md => ({ value: md.text, label: (Math.round(md.r * 100) / 100) + " Hz" }));
        }
        readonly property string rate: {
            if (!m)
                return "";
            // The listed mode closest to the current rate
            let best = "", d = Infinity;
            for (const md of m.modes)
                if (md.w === m.width && md.h === m.height && Math.abs(md.r - m.refresh) < d) {
                    d = Math.abs(md.r - m.refresh);
                    best = md.text;
                }
            return best;
        }
        readonly property var scales: {
            if (!m)
                return [];
            const list = [1, 1.25, 1.5, 1.75, 2];
            const cur = Math.round(m.scale * 100) / 100;
            if (!list.some(v => Math.abs(v - cur) < 0.01))
                list.push(cur);
            list.sort((a, b) => a - b);
            return list.map(v => {
                const s = Displays.logicalSize(m, { mode: m.mode, scale: v, transform: m.transform });
                return { value: v, label: Math.round(v * 100) + "%  ·  looks like " + s.w + " × " + s.h };
            });
        }

        Row {
            label: "Use this display"
            detail: page.enabledCount <= 1 && settingsGroup.m?.enabled ? "The only display that's on can't be turned off" : ""
            Switch {
                enabled: !(page.enabledCount <= 1 && settingsGroup.m?.enabled)
                opacity: enabled ? 1 : 0.45
                checked: settingsGroup.m?.enabled ?? false
                onToggled: if (enabled) Displays.set(settingsGroup.m.name, checked ? { enabled: false } : { enabled: true, mirror: "" })
            }
        }
        Row {
            visible: page.mons.length > 1 && (settingsGroup.m?.enabled ?? false)
            label: "Mirror"
            detail: "Show the same picture as another display"
            Dropdown {
                options: [{ value: "", label: "Off" }].concat(page.mons.filter(o => o.name !== settingsGroup.m?.name && o.enabled && !o.mirror).map(o => ({ value: o.name, label: Displays.numberOf(o.name) + "  " + o.label })))
                value: settingsGroup.m?.mirror ?? ""
                onChosen: v => Displays.set(settingsGroup.m.name, { mirror: v })
            }
        }
        Row {
            visible: (settingsGroup.m?.enabled ?? false) && !settingsGroup.m?.mirror
            label: "Resolution"
            Dropdown {
                options: settingsGroup.resolutions
                value: settingsGroup.resolution
                onChosen: v => {
                    // Fastest rate at that size
                    const best = settingsGroup.m.modes.filter(md => md.w + "x" + md.h === v).sort((a, b) => b.r - a.r)[0];
                    if (best)
                        Displays.set(settingsGroup.m.name, { mode: best.text });
                }
            }
        }
        Row {
            visible: (settingsGroup.m?.enabled ?? false) && !settingsGroup.m?.mirror && settingsGroup.rates.length > 0
            label: "Refresh rate"
            detail: CoolDown.savedRates?.[settingsGroup.m?.name ?? ""] !== undefined ? "Cool-down has lowered it for now" : ""
            Dropdown {
                options: settingsGroup.rates
                value: settingsGroup.rate
                onChosen: v => Displays.set(settingsGroup.m.name, { mode: v })
            }
        }
        Row {
            visible: (settingsGroup.m?.enabled ?? false) && !settingsGroup.m?.mirror
            label: "Scale"
            detail: "Larger makes text and controls bigger; smaller fits more on screen"
            Dropdown {
                implicitWidth: 260
                options: settingsGroup.scales
                value: settingsGroup.scales.find(o => Math.abs(o.value - (settingsGroup.m?.scale ?? 1)) < 0.01)?.value ?? 1
                onChosen: v => Displays.set(settingsGroup.m.name, { scale: v })
            }
        }
        Row {
            visible: (settingsGroup.m?.enabled ?? false) && !settingsGroup.m?.mirror
            label: "Rotation"
            Segmented {
                width: 260
                options: ["Standard", "90°", "180°", "270°"]
                current: (settingsGroup.m?.transform ?? 0) % 4
                onSelected: i => Displays.set(settingsGroup.m.name, { transform: i + ((settingsGroup.m.transform ?? 0) >= 4 ? 4 : 0) })
            }
        }
        // External monitors: brightness over DDC/CI (ddcutil)
        readonly property bool builtIn: /^(eDP|LVDS|DSI)/.test(m?.name ?? "")
        readonly property var ddc: Brightness.ddcDisplays.find(d => d.output === (m?.name ?? "")) ?? null
        Row {
            visible: settingsGroup.ddc !== null && (settingsGroup.m?.enabled ?? false)
            label: "Brightness"
            detail: Math.round((Brightness.ddcValues[settingsGroup.m?.name ?? ""] ?? 0.5) * 100) + "%, set on the display itself (DDC/CI)"
            Slider {
                width: 220
                value: Brightness.ddcValues[settingsGroup.m?.name ?? ""] ?? 0.5
                onMoved: v => Brightness.setDdc(settingsGroup.m.name, v)
            }
        }
        Row {
            visible: !settingsGroup.builtIn && (settingsGroup.m?.enabled ?? false) && !Brightness.ddcInstalled
            label: "Brightness"
            detail: "To change this display's brightness from iRice, install ddcutil (sudo pacman -S ddcutil), then log out and back in."
        }
        Row {
            visible: !settingsGroup.builtIn && (settingsGroup.m?.enabled ?? false) && Brightness.ddcInstalled && settingsGroup.ddc === null
            label: "Brightness"
            labelColor: Brightness.ddcProblem !== "" ? Theme.color.danger : Theme.color.label
            detail: Brightness.ddcProblem === "module" ? "ddcutil can't reach displays because the i2c-dev module isn't loaded. Run sudo modprobe i2c-dev (to load it at every start: echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf), then check again."
                : Brightness.ddcProblem === "access" ? "ddcutil isn't allowed to open /dev/i2c-*. Add yourself to the i2c group (sudo usermod -aG i2c $USER), log out and back in, then check again."
                : !Brightness.ddcScanned || Brightness.ddcScanning ? "Looking for DDC/CI support…"
                : "This display doesn't answer DDC/CI. Turn on DDC/CI in the display's own menu, then check again."
            SettingsButton { text: "Check Again"; enabled: !Brightness.ddcScanning; onClicked: Brightness.rescanDdc() }
        }
        Row {
            visible: (settingsGroup.m?.enabled ?? false) && !settingsGroup.m?.mirror
            label: "Flip"
            detail: "Mirror image, for teleprompters and projectors"
            Switch {
                checked: (settingsGroup.m?.transform ?? 0) >= 4
                onToggled: Displays.set(settingsGroup.m.name, { transform: (settingsGroup.m.transform % 4) + (checked ? 0 : 4) })
            }
        }
    }

    Group {
        width: parent.width
        title: "When a display connects"
        footer: "The new display fills from the side it shares with its neighbour, which comes from the arrangement above; a display that leaves pulls back from that side. Directional sound plays the connect sound from that side (with Sound effects on)."
        Row {
            label: "Animation"
            Segmented {
                width: 300
                readonly property var values: ["off", "extend", "liquid", "warp"]
                options: ["Off", "Extend", "Liquid", "Warp"]
                current: Math.max(0, values.indexOf(Config.options.displays.connectAnimation))
                onSelected: i => Config.options.displays.connectAnimation = values[i]
            }
        }
        Row {
            label: "Directional sound"
            detail: "Heard from the side the display is on"
            Switch { checked: Config.options.displays.directionalSound; onToggled: Config.options.displays.directionalSound = !checked }
        }
        Row {
            label: "Preview"
            detail: "On this display, as if one were plugged in on that side"
            QQ.Row {
                spacing: 8
                SettingsButton { text: "Left"; icon: "arrow-left"; onClicked: DisplayFx.preview(Config.options.displays.connectAnimation === "off" ? "extend" : Config.options.displays.connectAnimation, "left") }
                SettingsButton { text: "Above"; icon: "arrow-up"; onClicked: DisplayFx.preview(Config.options.displays.connectAnimation === "off" ? "extend" : Config.options.displays.connectAnimation, "top") }
                SettingsButton { text: "Right"; icon: "arrow-right"; onClicked: DisplayFx.preview(Config.options.displays.connectAnimation === "off" ? "extend" : Config.options.displays.connectAnimation, "right") }
            }
        }
    }

    Group {
        width: parent.width
        title: "Switching displays"
        footer: "Each display has its own spaces, numbered from 1: Super+number goes to that space on the display you're on."
        Row {
            label: "Move the pointer there"
            detail: "When a shortcut takes you to another display"
            Switch { checked: Config.options.workspaces.warpPointer; onToggled: Config.options.workspaces.warpPointer = !checked }
        }
        Row {
            label: "Show which display you moved to"
            detail: ({ glow: "A soft glow at its edges for a moment", dim: "It dims briefly", off: "Nothing shows" })[Config.options.workspaces.switchCue] ?? "A soft glow at its edges for a moment"
            Segmented {
                width: 200
                readonly property var values: ["off", "glow", "dim"]
                options: ["Off", "Glow", "Dim"]
                current: Math.max(0, values.indexOf(Config.options.workspaces.switchCue || "glow"))
                onSelected: i => Config.options.workspaces.switchCue = values[i]
            }
        }
    }

    Group {
        width: parent.width
        visible: page.mons.length > 0
        footer: "Settings are saved per display and come back when it's plugged in again."
        Row {
            label: "Restore defaults"
            detail: "Every display goes back to what Hyprland picks by itself"
            SettingsButton { text: "Restore Defaults"; enabled: (Config.options.displays.monitors ?? []).length > 0; onClicked: Displays.resetAll() }
        }
    }

    // ── Wireless displays (services/Mirroring) ──────────────────────────
    Group {
        id: wireless
        width: parent.width
        title: "Wireless displays"
        footer: !Mirroring.available ? "Show this screen on a TV: Miracast TVs and sticks, Windows' Miracast over the network, and Chromecasts. Needs GNOME Network Displays."
            : Mirroring.error !== "" ? Mirroring.error
            : Needs.checked && !Needs.networkManager ? "Miracast TVs need NetworkManager (for Wi-Fi Direct), which isn't running here. Chromecasts and Miracast over the network still work."
            : !Mirroring.p2p ? "This computer's Wi-Fi can't do Wi-Fi Direct, which Miracast TVs need. Chromecasts and network Miracast still work."
            : "Turn on the TV's screen mirroring (Miracast, Screen Mirroring or Cast). Control Center's Screen Mirroring tile does the same."
        // (looks while the page is up: from the moment it's installed, too)
        property bool lookingHere: false
        function lookIfAvailable() {
            if (Mirroring.available !== lookingHere) {
                lookingHere = Mirroring.available;
                Mirroring.look(lookingHere);
            }
        }
        Component.onCompleted: lookIfAvailable()
        Component.onDestruction: if (lookingHere) Mirroring.look(false)
        readonly property Connections availableWatch: Connections {
            target: Mirroring
            function onAvailableChanged() {
                wireless.lookIfAvailable();
            }
        }
        Row {
            visible: !Mirroring.available
            label: "Add a wireless display"
            detail: Mirroring.installError !== "" ? Mirroring.installError : "It asks for your password once."
            labelColor: Mirroring.installError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                text: Mirroring.installing ? "Installing…" : "Install"
                icon: "download-simple"
                busy: Mirroring.installing
                enabled: !Updates.running
                onClicked: Mirroring.install()
            }
        }
        Row {
            visible: Mirroring.available && Mirroring.displays.length === 0
            label: "Looking for TVs…"
            detail: "Nearby TVs and sticks appear here"
        }
        Repeater {
            model: Mirroring.available ? Mirroring.displays : []
            Row {
                id: tv
                required property var modelData
                readonly property bool here: Mirroring.target === modelData.uuid
                label: modelData.name
                detail: here ? (Mirroring.state_ === "streaming" ? "Mirroring" : "Connecting…") : Mirroring.protocolName(modelData.protocol)
                SettingsButton {
                    text: tv.here ? "Stop" : "Mirror"
                    icon: tv.here ? "stop" : "screencast"
                    onClicked: tv.here ? Mirroring.stop() : Mirroring.start(tv.modelData)
                }
            }
        }
    }
}
