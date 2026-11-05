import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import qs.common
import qs.services

// Control Center drawn in Rust (rust.surfaces: controlcenter,
// native/tessera-ui/src/surfaces/controlcenter): while it's open, this tells
// tessera-ui what only this shell knows (stdin lines through RustUi.tell):
// each control's state (ModuleDefs: its label, icon, on, status, whether
// it can be used here), the player, you, and the open page's rows as plain
// blocks; and does what a click there asks (RustUi.ccCall: cc.act, cc.block,
// cc.pick…). The sliders, the microphone, the Sound and Microphone pages and
// the battery are tessera-ui's own (the sound server, the backlight, UPower).
// The open page and Edit controls stay in step with Panels.ccPage and
// Panels.ccEditing both ways.
Scope {
    id: root

    readonly property bool on: RustUi.drawing("controlcenter")
    readonly property bool open: on && Panels.open === "controlCenter"

    ModuleDefs {
        id: defs
    }

    // ── What the controls say ────────────────────────────────────────────
    property int tick: 0 // (a playing track's position: once a second)
    Timer {
        running: root.open && (Media.player?.isPlaying ?? false)
        interval: 1000
        repeat: true
        onTriggered: root.tick++
    }
    function localArt(url) {
        return /^file:\/\//.test(url ?? "") ? url.replace(/^file:\/\//, "").replace(/\?.*$/, "") : "";
    }
    function menuOf(items) {
        return items.map(it => ({ text: it.text ?? "", checked: !!it.checked, sep: !!it.separator, danger: !!it.danger }));
    }
    readonly property var said: {
        if (!open)
            return null;
        void tick;
        const mods = {};
        for (const id in defs.all) {
            const d = defs.all[id];
            const m = { label: d.label ?? id, icon: d.icon ?? "", on: !!d.on, status: d.status ?? "", usable: defs.usable(id), why: defs.whyNot(id) };
            if (id === "resources") {
                m.title = "CPU " + Math.round(Resources.cpu * 100) + "%";
                m.sub = "Memory " + Math.round(Resources.memory * 100) + "%";
            } else if (id === "weather") {
                m.title = Weather.ready ? Weather.temp + Weather.unit : "—";
                m.sub = Weather.condition || Weather.place || "";
            }
            if (d.menu)
                m.menu = menuOf(d.menu());
            mods[id] = m;
        }
        const p = Media.player;
        return {
            modules: mods,
            media: {
                has: !!p,
                identity: p?.identity ?? "",
                title: p?.trackTitle ?? "",
                artist: p?.trackArtist ?? "",
                playing: p?.isPlaying ?? false,
                art: localArt(Media.artUrl(p)),
                prev: p?.canGoPrevious ?? false,
                next: p?.canGoNext ?? false,
                toggle: p?.canTogglePlaying ?? false,
                seek: p?.canSeek ?? false,
                position: p?.position ?? 0,
                length: (p?.lengthSupported ?? false) ? (p?.length ?? 0) : 0
            },
            cast: Cast.devices.length > 0 || Cast.active,
            casting: Cast.active,
            avatar: localArt(Account.pictureUrl),
            user: Account.name,
            // (Sound's outputs that switch the card over first)
            profiles: page === "sound" ? AudioProfiles.alternatives.map(a => ({ name: a.name, kind: a.kind ?? "", busy: AudioProfiles.switching === a.profile })) : []
        };
    }
    property string sentState: ""
    onSaidChanged: Qt.callLater(sendState)
    function sendState() {
        if (!open || !said)
            return;
        const s = JSON.stringify(said);
        if (s === sentState)
            return;
        sentState = s;
        RustUi.tell("controlcenter", { state: said });
    }

    // ── The open page, as blocks ─────────────────────────────────────────
    readonly property string page: open ? Panels.ccPage : ""
    readonly property var pageModel: page === "" || page === "sound" || page === "mic" ? null : build(page)
    property string sentPage: ""
    onPageModelChanged: Qt.callLater(sendPage)
    function sendPage() {
        if (!open || !pageModel)
            return;
        const s = JSON.stringify(pageModel);
        if (s === sentPage)
            return;
        sentPage = s;
        RustUi.tell("controlcenter", { page: pageModel });
    }
    function build(name) {
        switch (name) {
        case "wifi":
            return wifiPage();
        case "bluetooth":
            return btPage();
        case "focus":
            return focusPage();
        case "caffeine":
            return caffeinePage();
        case "sharing":
            return sharingPage();
        case "mirroring":
            return mirroringPage();
        case "cast":
            return castPage();
        case "resources":
            return resourcesPage();
        }
        return { name: name, title: name, blocks: [] };
    }
    function blockOf(id) {
        return (pageModel?.blocks ?? []).find(b => b.id === id) ?? null;
    }
    function row(o) {
        return Object.assign({ t: "row" }, o);
    }
    function note(text, tone, center) {
        return { t: "note", text: text, tone: tone ?? "secondary", center: !!center };
    }
    function head(text, more) {
        return { t: "head", text: text, h: more ? 1 : 0 };
    }

    // Wi-Fi (WifiPage, WifiList): connected first, then known, then by signal
    readonly property var wifiDevice: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    property var networks: []
    property var devices: []
    property string asking: "" // the network waiting for a password
    property string hint: ""
    function refreshLists() {
        networks = (wifiDevice?.networks.values ?? []).slice().sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 30);
        const seen = new Set();
        devices = (Bluetooth.defaultAdapter?.devices.values ?? []).filter(d => d.paired || !!d.name).slice().sort((a, b) => (b.connected - a.connected) || (b.paired - a.paired) || (a.name ?? "").localeCompare(b.name ?? "")).filter(d => {
            if (d.paired || !d.name)
                return true;
            if (seen.has(d.name))
                return false;
            seen.add(d.name);
            return true;
        }).slice(0, 40);
    }
    Timer {
        running: root.page === "wifi" || root.page === "bluetooth"
        interval: 4000
        repeat: true
        onTriggered: if (root.asking === "") root.refreshLists()
    }
    // (scanning and discovery only while their page shows)
    Binding {
        target: root.wifiDevice
        property: "scannerEnabled"
        value: true
        when: root.page === "wifi" && root.wifiDevice !== null
        restoreMode: Binding.RestoreBindingOrValue
    }
    Binding {
        target: Bluetooth.defaultAdapter
        property: "discovering"
        value: true
        when: root.page === "bluetooth" && (Bluetooth.defaultAdapter?.enabled ?? false)
        restoreMode: Binding.RestoreBindingOrValue
    }
    function enterprise(n) {
        return [WifiSecurityType.Wpa2Eap, WifiSecurityType.WpaEap, WifiSecurityType.Wpa3SuiteB192, WifiSecurityType.Leap, WifiSecurityType.DynamicWep].includes(n?.security);
    }
    function secured(n) {
        return n.security !== WifiSecurityType.Open && n.security !== WifiSecurityType.Unknown;
    }
    function liveNet(name) {
        return (wifiDevice?.networks.values ?? []).find(x => x.name === name) ?? null;
    }
    function wifiMenu(n) {
        const items = [];
        if (n.connected)
            items.push({ text: "Disconnect", action: () => n.disconnect() });
        else if (n.known || n.security === WifiSecurityType.Open)
            items.push({ text: "Connect", action: () => n.connect() });
        if (!n.connected && secured(n))
            items.push({ text: enterprise(n) ? "Connect with sign-in…" : "Connect with password…", action: () => enterprise(n) ? Panels.openSettings("network") : root.asking = n.name });
        items.push({ separator: true });
        if ((n.connected || n.known) && !enterprise(n))
            items.push({ text: "Share network…", action: () => Panels.openSettings("network") });
        items.push({ text: "Copy name", action: () => Quickshell.execDetached(["wl-copy", "--", n.name]) });
        if (n.known)
            items.push({ text: "Forget network", danger: true, action: () => {
                    if (typeof n.forget === "function")
                        n.forget();
                    else
                        WifiJoin.forget(n.name);
                    WifiJoin.loadKnown();
                    root.refreshLists();
                } });
        items.push({ separator: true });
        items.push({ text: "Network details…", action: () => Panels.openSettings("network") });
        return items;
    }
    function wifiPage() {
        const blocks = [];
        const on = Networking.wifiEnabled;
        if (Backends.wifiUnmanaged) {
            blocks.push({ t: "notice", icon: "wifi-slash", title: Backends.wifiTitle, status: Backends.wifiReason, act: Backends.canStartNm ? "Start NetworkManager" : Backends.wifiApp !== "" ? "Open " + Backends.wifiAppName : "", busy: Health.fixing === "networkmanager", id: "fix", run: () => Backends.canStartNm ? Backends.startNm() : Backends.openWifiApp() });
        } else {
            if (!on || networks.length === 0)
                blocks.push(note(!wifiDevice ? "No Wi-Fi adapter" : !on ? "Wi-Fi is off" : "Looking for networks…", "secondary", true));
            for (const n of on ? networks : []) {
                const items = wifiMenu(n);
                blocks.push(row({
                    id: "n:" + n.name,
                    icon: n.signalStrength > 0.66 ? "wifi-high" : n.signalStrength > 0.33 ? "wifi-medium" : "wifi-low",
                    hl: n.connected,
                    title: n.name,
                    status: n.connected ? "Connected" : n.state === ConnectionState.Connecting || WifiJoin.joining(n.name) ? "Connecting…" : WifiJoin.failedFor(n.name) !== "" ? "Couldn't connect" : n.known ? "Known network" : "",
                    trail: secured(n) ? "lock-simple" : "",
                    act: n.connected && !enterprise(n) ? "qr-code" : "",
                    busy: n.stateChanging || WifiJoin.joining(n.name),
                    menu: menuOf(items),
                    pick: i => items[i]?.action?.(),
                    run: (what, text) => {
                        const live = root.liveNet(n.name) ?? n;
                        if (what === "cancel")
                            root.asking = "";
                        else if (what === "action" || what === "other")
                            Panels.openSettings("network");
                        else if (what === "commit") {
                            const ok = text.length >= 8 && text.length <= 63 || [5, 10, 13, 26].includes(text.length);
                            if (!ok) {
                                root.hint = text === "" ? "Type the password first." : "Wi-Fi passwords are at least 8 characters.";
                                return;
                            }
                            live.connectWithPsk(text);
                            root.asking = "";
                        } else if (live.connected)
                            live.disconnect();
                        else if (live.known || live.security === WifiSecurityType.Open)
                            live.connect();
                        else if (root.enterprise(live))
                            Panels.openSettings("network");
                        else
                            root.asking = root.asking === n.name ? "" : n.name;
                    }
                }));
                if (asking === n.name)
                    blocks.push({ t: "pass", id: "n:" + n.name, status: hint });
                else if (WifiJoin.failedFor(n.name) !== "")
                    blocks.push(note(WifiJoin.failedFor(n.name), "danger"));
            }
            if (on)
                blocks.push(row({ id: "join", icon: "plus", title: "Join other network…", run: () => Panels.openSettings("network") }));
        }
        return { name: "wifi", title: "Wi-Fi", switch: !Backends.wifiUnmanaged, switchOn: Networking.wifiEnabled, refresh: !Backends.wifiUnmanaged, blocks: blocks,
            onSwitch: () => Networking.wifiEnabled = !Networking.wifiEnabled,
            onRefresh: () => {
                if (root.wifiDevice) {
                    root.wifiDevice.scannerEnabled = false;
                    root.wifiDevice.scannerEnabled = true;
                }
                root.refreshLists();
            } };
    }

    // Bluetooth (BluetoothPage, BluetoothList)
    function btIcon(d) {
        const i = d.icon ?? "";
        return i.includes("headset") || i.includes("headphone") ? "headphones" : i.includes("phone") ? "device-mobile" : i.includes("keyboard") ? "keyboard" : i.includes("mouse") ? "mouse" : i.includes("audio") ? "speaker-hifi" : "bluetooth";
    }
    function btPage() {
        const a = Bluetooth.defaultAdapter;
        const blocks = [];
        if (Backends.btState !== "ok" && Backends.btState !== "") {
            blocks.push({ t: "notice", id: "fix", icon: "bluetooth-slash", title: Backends.btTitle, status: Backends.btReason, act: Backends.btFixLabel, busy: Backends.btFixing, run: () => Backends.fixBluetooth() });
        } else {
            if (!a?.enabled || devices.length === 0)
                blocks.push(note(!a ? "No Bluetooth adapter. If this computer has one, check that it isn't switched off in its settings or with a key." : !a.enabled ? "Bluetooth is off" : "Looking for devices…", "secondary", true));
            for (const d of a?.enabled ? devices : []) {
                const items = [];
                if (d.connected)
                    items.push({ text: "Disconnect", action: () => d.disconnect() });
                else if (d.paired)
                    items.push({ text: "Connect", action: () => d.connect() });
                if (d.pairing)
                    items.push({ text: "Stop pairing", action: () => d.cancelPair() });
                else if (!d.paired)
                    items.push({ text: "Pair", action: () => d.pair() });
                if (d.paired)
                    items.push(d.trusted ? { text: "Don't connect automatically", action: () => d.trusted = false } : { text: "Connect automatically", action: () => d.trusted = true });
                items.push({ separator: true });
                items.push({ text: "Copy address", action: () => Quickshell.execDetached(["wl-copy", "--", d.address]) });
                if (d.paired)
                    items.push({ text: "Forget device", danger: true, action: () => {
                            d.forget();
                            root.refreshLists();
                        } });
                items.push({ separator: true });
                items.push({ text: "Bluetooth settings…", action: () => Panels.openSettings("network", "Bluetooth") });
                blocks.push(row({
                    id: "d:" + d.address,
                    icon: btIcon(d),
                    pic: Devices.pictureOf(d), // (a drawing in assets/devices)
                    hl: d.connected,
                    title: d.name || d.address,
                    status: d.connected ? (d.batteryAvailable ? "Connected · " + Math.round(d.battery * 100) + "%" : "Connected") : d.pairing ? "Pairing…" : d.paired ? "Not connected" : "",
                    busy: d.state === BluetoothDeviceState.Connecting || d.state === BluetoothDeviceState.Disconnecting || d.pairing,
                    menu: menuOf(items),
                    pick: i => items[i]?.action?.(),
                    run: () => d.connected ? d.disconnect() : d.paired ? d.connect() : d.pair()
                }));
            }
        }
        return { name: "bluetooth", title: "Bluetooth", switch: Backends.btState === "ok", switchOn: a?.enabled ?? false, refresh: Backends.btState === "ok", blocks: blocks,
            onSwitch: () => {
                if (a)
                    a.enabled = !a.enabled;
            },
            onRefresh: () => {
                if (a?.enabled) {
                    a.discovering = false;
                    a.discovering = true;
                }
                root.refreshLists();
            } };
    }

    // Focus (FocusPage): pick how long, then a mode
    property int focusDuration: 0
    function focusPage() {
        const blocks = [];
        for (const m of Focus.modes) {
            const isActive = Focus.active === m.id;
            blocks.push(row({ id: "f:" + m.id, icon: m.icon, title: m.name, hl: isActive, status: isActive ? Focus.statusText : m.schedule?.enabled ? "Scheduled " + Focus.fmt(m.schedule.from) + "–" + Focus.fmt(m.schedule.to) : "", trail: isActive ? "check" : "", run: () => isActive ? Focus.deactivate() : Focus.activate(m.id, root.focusDuration, false) }));
        }
        blocks.push({ t: "gap", h: 12 });
        blocks.push({ t: "head", text: "Turn on" });
        blocks.push({ t: "seg", id: "len", options: ["Until off", "For 1 hour", "Until evening"], cur: focusDuration === 3600000 ? 1 : focusDuration === -1 ? 2 : 0, run: (what, i) => root.focusDuration = [0, 3600000, -1][i] ?? 0 });
        blocks.push({ t: "gap", h: 6 });
        blocks.push(row({ id: "settings", icon: "gear-six", title: "Focus settings…", run: () => Panels.openSettings("focus") }));
        return { name: "focus", title: "Focus", blocks: blocks };
    }

    // Caffeine (CaffeinePage)
    function caffeinePage() {
        const blocks = [];
        for (const c of [{ minutes: 60, title: "For 1 hour" }, { minutes: 120, title: "For 2 hours" }, { minutes: 0, title: "Until turned off" }]) {
            const isActive = System.keepAwake && System.keepAwakeMinutes === c.minutes;
            blocks.push(row({ id: "c:" + c.minutes, icon: c.minutes > 0 ? "timer" : "coffee", title: c.title, hl: isActive, status: isActive ? System.keepAwakeStatus : "", trail: isActive ? "check" : "", run: () => isActive ? System.setKeepAwake(false, 0) : System.setKeepAwake(true, c.minutes) }));
        }
        blocks.push({ t: "gap", h: 6 });
        blocks.push(note("The screen won't dim, lock or sleep while you're away. Closing the lid still sleeps.", "tertiary"));
        return { name: "caffeine", title: "Caffeine", switch: true, switchOn: System.keepAwake, switchLabel: "Keep the screen awake", blocks: blocks, onSwitch: () => System.setKeepAwake(!System.keepAwake, 0) };
    }

    // Sharing (SharingPage)
    readonly property var levels: ["off", "10min", "everyone"]
    property bool sharingLook: false
    onPageChanged: {
        asking = "";
        hint = "";
        refreshLists();
        if (page === "sound")
            AudioProfiles.refresh();
        const want = page === "sharing";
        if (want !== sharingLook) {
            sharingLook = want;
            Sharing.look(want);
        }
        const mir = page === "mirroring";
        if (mir !== mirroringLook) {
            mirroringLook = mir;
            Mirroring.look(mir);
        }
    }
    function sharingPage() {
        const blocks = [];
        const seg = (id, cur, set) => ({ t: "seg", id: id, options: ["No one", "10 minutes", "Everyone"], cur: Math.max(0, levels.indexOf(cur)), run: (what, i) => set(levels[i]) });
        blocks.push(seg("all", Sharing.level, l => Sharing.setLevel(l)));
        blocks.push(head("LocalSend", true));
        blocks.push(seg("localsend", Config.options.sharing.receiving, l => Config.options.sharing.receiving = l));
        blocks.push(note("Computers and phones with the LocalSend app", "tertiary"));
        blocks.push(head("Quick Share", true));
        blocks.push(seg("quickshare", Config.options.sharing.quickShare, l => Config.options.sharing.quickShare = l));
        const needs = QuickShare.bluetoothNeeded ?? false, btOn = QuickShare.bluetoothOn ?? true;
        blocks.push(note(needs && !btOn ? "Turn on Bluetooth so Android phones can find this computer." : "Android phones nearby", needs && !btOn ? "danger" : "tertiary"));
        blocks.push(head("Your name", true));
        blocks.push({ t: "field", id: "name", title: Config.options.sharing.name, status: (Quickshell.env("USER") ?? "Tessera") + "'s computer", run: (what, text) => {
                if (what === "commit")
                    Config.options.sharing.name = text.trim();
            } });
        blocks.push({ t: "gap", h: 6 });
        blocks.push(row({ id: "send", icon: "paper-plane-tilt", title: "Send files…", status: "To a phone or computer nearby", dis: !(LocalSend.available && !FilePicker.busy), run: () => Panels.pickShare() }));
        blocks.push(head("Nearby", true));
        for (const d of Sharing.devices) {
            const sending = Sharing.progressFor(d);
            blocks.push(row({ id: "s:" + d.name, icon: d.type === "mobile" ? "device-mobile" : d.type === "laptop" ? "laptop" : "desktop", title: d.name, status: sending ? (sending.waiting ? "Waiting for them to accept…" : Math.round(100 * (sending.total > 0 ? sending.done / sending.total : 0)) + "% sent") : d.prefer === "quickshare" ? "Quick Share" : "LocalSend", trail: "paper-plane-tilt", run: () => {
                    if (!sending)
                        Sharing.pickFor(d);
                } }));
        }
        if (Sharing.devices.length === 0)
            blocks.push(note("No devices nearby. Make sure they're on the same Wi-Fi with LocalSend open, or Quick Share visible to everyone.", "tertiary"));
        const errors = [LocalSend.error, QuickShare.error].filter(e => !!e).join(" ");
        if (errors !== "")
            blocks.push(note(errors, "danger"));
        return { name: "sharing", title: "Sharing", switch: true, switchOn: Sharing.on, switchLabel: "Visible to nearby devices", blocks: blocks, onSwitch: () => Sharing.setLevel(Sharing.on ? "off" : "10min") };
    }

    // Screen Mirroring (MirroringPage)
    property bool mirroringLook: false
    function mirroringPage() {
        const blocks = [];
        for (const d of Mirroring.displays) {
            const here = Mirroring.target === d.uuid;
            blocks.push(row({ id: "m:" + d.uuid, icon: d.protocol === "chromecast" ? "television-simple" : "screencast", hl: here && Mirroring.state_ === "streaming", title: d.name, status: here ? (Mirroring.state_ === "streaming" ? "Mirroring" : "Connecting…") : Mirroring.protocolName(d.protocol), busy: here && Mirroring.state_ === "connecting", trail: here ? "stop" : "", run: () => here ? Mirroring.stop() : Mirroring.start(d) }));
        }
        if (Mirroring.displays.length === 0 || Mirroring.error !== "" || !Mirroring.p2p || (Needs.checked && !Needs.networkManager))
            blocks.push(note(!Mirroring.available ? "Screen mirroring needs GNOME Network Displays. Install it in Settings → Displays." : Mirroring.error !== "" ? Mirroring.error : Needs.checked && !Needs.networkManager ? "Miracast TVs need NetworkManager (for Wi-Fi Direct), which isn't running here. Chromecasts and Miracast over the network still work." : Mirroring.displays.length === 0 ? "Looking for TVs… Turn on the TV's screen mirroring (Miracast, Screen Mirroring or Cast), nearby." : "This computer's Wi-Fi can't do Wi-Fi Direct: Miracast TVs need it. Chromecasts and network Miracast still work.", Mirroring.error !== "" ? "danger" : "tertiary"));
        return { name: "mirroring", title: "Screen Mirroring", blocks: blocks };
    }

    // Cast (CastPage)
    function castPage() {
        const blocks = [];
        const p = Media.player;
        blocks.push(note(Cast.active ? "Playing on " + (Cast.device?.name ?? "") + ": " + Cast.title : !p ? "Play something first, then choose where it goes." : Cast.canCastPlaying ? "Cast “" + (p.trackTitle || p.identity) + "” to:" : "“" + (p.identity ?? "This app") + "” doesn't say what it's playing, so it can't be cast. Cast a video or song from Files instead."));
        for (const d of Cast.devices) {
            const here = Cast.active && Cast.device?.id === d.id;
            blocks.push(row({ id: "t:" + d.id, icon: Cast.iconFor(d), hl: here, title: d.name, status: here ? (Cast.state_ === "playing" ? "Playing" : Cast.state_ === "paused" ? "Paused" : "Connecting…") : d.model, trail: here ? "stop" : "", busy: here && (Cast.state_ === "connecting" || Cast.state_ === "loading"), dis: !(here || (Cast.canCastPlaying && !!p)), run: () => here ? Cast.stop() : Cast.castPlaying(d) }));
        }
        if (Needs.checked && !Needs.avahi)
            blocks.push(row({ id: "avahi", icon: "wifi-high", title: Needs.installing || Health.fixing === "avahi" ? "Turning on…" : "Turn on network discovery", status: "TVs are found with avahi, which isn't running", busy: Needs.installing || Health.fixing === "avahi", run: () => Needs.turnOnAvahi() }));
        if (Cast.devices.length === 0 && (!Needs.checked || Needs.avahi))
            blocks.push(note(Tools.has("avahi-browse") ? "No Chromecast or Google TV on this network. Check that it's on and on the same Wi-Fi." : "Finding TVs needs avahi (avahi-browse).", "tertiary"));
        return { name: "cast", title: "Cast", blocks: blocks };
    }

    // CPU & Memory (ResourcesPage: graphs in the QML; readings here)
    function resourcesPage() {
        const blocks = [];
        blocks.push(row({ id: "cpu", icon: "cpu", title: "Processor", status: Math.round(Resources.cpu * 100) + "%" }));
        blocks.push(row({ id: "mem", icon: "memory", title: "Memory", status: Math.round(Resources.memory * 100) + "%" }));
        blocks.push({ t: "gap", h: 2 });
        blocks.push(note("The last minute, once a second. Network counts every interface but loopback, containers and VPN tunnels." + (Resources.gpuAvailable ? "" : " Your GPU driver doesn't report its load."), "tertiary"));
        return { name: "resources", title: "CPU & Memory", blocks: blocks };
    }

    // ── Opening, and Panels' page and editor, both ways ──────────────────
    onOpenChanged: {
        sentState = "";
        sentPage = "";
        if (open) {
            refreshLists();
            RustUi.tell("controlcenter", { ccPage: Panels.ccPage, editing: Panels.ccEditing });
            sendState();
            sendPage();
        } else if (on && Panels.screen) {
            Panels.ccEditing = false;
        }
    }
    Connections {
        target: Panels
        function onCcPageChanged() {
            if (root.on)
                RustUi.tell("controlcenter", { ccPage: Panels.ccPage });
        }
        function onCcEditingChanged() {
            if (root.on)
                RustUi.tell("controlcenter", { editing: Panels.ccEditing });
        }
    }

    // ── What a click there asks ──────────────────────────────────────────
    Connections {
        target: RustUi
        function onCcCall(name, a) {
            switch (name) {
            case "cc.act":
                defs.all[a[0]]?.act?.();
                break;
            case "cc.page":
                Panels.ccPage = String(a[0] ?? "");
                break;
            case "cc.editing":
                Panels.ccEditing = a[0] === true;
                break;
            case "cc.layout":
                if (Array.isArray(a[0]))
                    Config.options.controlCenter.layout = a[0].map(String);
                break;
            case "cc.media": {
                const p = Media.player;
                const w = String(a[0] ?? "");
                if (w === "previous")
                    p?.previous();
                else if (w === "next")
                    p?.next();
                else if (w === "toggle")
                    p?.togglePlaying();
                else if (w.startsWith("seek:") && p?.canSeek)
                    p.position = Math.max(0, Math.min(1, Number(w.slice(5)) || 0)) * p.length;
                break;
            }
            case "cc.block": {
                // (Sound is drawn there; its card-switching outputs are this shell's)
                if (a[0] === "sound" && String(a[1]).startsWith("prof:")) {
                    const alt = AudioProfiles.alternatives[Number(String(a[1]).slice(5))];
                    if (alt)
                        AudioProfiles.pick(alt);
                    break;
                }
                const m = root.pageModel;
                if (!m || m.name !== a[0])
                    break;
                const what = String(a[2] ?? "");
                if (what === "switch")
                    m.onSwitch?.();
                else if (what === "refresh")
                    m.onRefresh?.();
                else {
                    const b = root.blockOf(a[1]);
                    if (what.startsWith("seg:"))
                        b?.run?.("seg", Number(what.slice(4)));
                    else
                        b?.run?.(what, String(a[3] ?? ""));
                }
                break;
            }
            case "cc.pick":
                if ((a[0] ?? "") === "")
                    defs.menuItems(a[1], p => Panels.ccPage = p)[a[2]]?.action?.();
                else
                    root.blockOf(a[1])?.pick?.(a[2]);
                break;
            }
        }
    }
}
