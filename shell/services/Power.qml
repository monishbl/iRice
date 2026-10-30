pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.common
import "PowerPolicy.js" as Policy

// Battery & Power (Settings → Battery & Power): one place that decides the
// energy mode, and the battery's idle, lid and critical-battery behaviour.
//
// Energy mode: everyone who wants a say asks here instead of setting the
// power daemon themselves, and one answer wins (PowerPolicy.resolve):
// Cool-down when hot, then the battery saver, then your own choice or Game
// Mode, whichever came last. Each request has an owner, a reason and
// optionally an end. A change made outside iRice (powerprofilesctl) counts
// as your choice. The dev sandbox never sets the mode: it shares the
// machine's power daemon. Without power-profiles-daemon (TLP or
// auto-cpufreq manage power, or nothing does: Backends.ppd) there are no
// modes to set, and nothing here claims to switch them.
Singleton {
    id: root

    readonly property var cfg: Config.options.power
    readonly property bool sandboxed: System.sandboxed

    // ── Battery ─────────────────────────────────────────────────────────
    readonly property var battery: UPower.displayDevice
    readonly property bool hasBattery: battery?.isLaptopBattery ?? false
    readonly property real percent: !hasBattery ? -1 : battery.percentage > 1 ? battery.percentage : battery.percentage * 100
    readonly property bool onBattery: hasBattery && UPower.onBattery

    // ── Energy mode ─────────────────────────────────────────────────────
    readonly property var names: ({ "power-saver": "Low Power", "balanced": "Balanced", "performance": "Performance" })
    function profileOf(mode) {
        return mode === "power-saver" ? PowerProfile.PowerSaver : mode === "performance" ? PowerProfile.Performance : PowerProfile.Balanced;
    }
    function modeOf(profile) {
        return profile === PowerProfile.PowerSaver ? "power-saver" : profile === PowerProfile.Performance ? "performance" : "balanced";
    }
    // owner -> { owner, mode, priority, since, until, why }
    property var requests: ({})
    property real manualSince: 0 // when you last chose (Game Mode turned on later wins)
    function ask(owner, mode, why, until) {
        const r = Object.assign({}, requests);
        r[owner] = { owner: owner, mode: mode, priority: { cooldown: 40, battery: 30, manual: 20, game: 20 }[owner] ?? 10, since: requests[owner]?.mode === mode ? requests[owner].since : Date.now(), until: until ?? 0, why: why ?? "" };
        requests = r;
    }
    function release(owner) {
        if (!requests[owner])
            return;
        const r = Object.assign({}, requests);
        delete r[owner];
        requests = r;
    }
    function has(owner) {
        return !!requests[owner];
    }
    // Your choice (Control Center, the battery menu, Settings): saved, and
    // newer than Game Mode. It also ends the battery saver until you plug
    // in again.
    function setMode(mode) {
        manualSince = Date.now();
        Config.options.power.mode = mode;
        release("battery");
    }
    readonly property var all: {
        const list = Object.values(requests);
        list.push({ owner: "manual", mode: cfg.mode, priority: 20, since: manualSince, until: 0, why: "" });
        return list;
    }
    property real now: Date.now()
    Timer { // expiring requests (Game Mode for an hour, …)
        running: Object.values(root.requests).some(r => r.until > 0)
        interval: 30000
        repeat: true
        onTriggered: root.now = Date.now()
    }
    readonly property var winner: Policy.resolve(all, now)
    readonly property string mode: Policy.fit(winner.mode, PowerProfiles.hasPerformanceProfile)
    readonly property string modeName: names[mode] ?? mode
    // Why it's this mode, for Settings: "Low Power · battery at 18%"
    readonly property string why: winner.owner === "manual" || winner.owner === "default" ? "" : winner.why

    // Applied when the answer changes; what we set is remembered, so a
    // different profile showing up means someone else changed it
    property string applied: ""
    onModeChanged: apply()
    function apply() {
        if (!Config.ready || sandboxed || !Backends.ppd)
            return;
        applied = mode;
        if (PowerProfiles.profile !== profileOf(mode))
            PowerProfiles.profile = profileOf(mode);
    }
    Connections {
        target: PowerProfiles
        function onProfileChanged() {
            const seen = root.modeOf(PowerProfiles.profile);
            if (!root.sandboxed && root.applied !== "" && seen !== root.applied && Config.ready) {
                // Changed outside iRice: your choice now
                root.applied = seen;
                root.setMode(seen);
            }
        }
    }
    Connections {
        target: Config
        function onReadyChanged() {
            if (Config.ready) {
                Qt.callLater(root.apply);
                root.syncGuard();
                idleTimer.restart(); // (the timers as saved, not the defaults)
            }
        }
    }

    // Battery saver: once per discharge, at its level (20%) on battery, the
    // island asks "Turn on Low Power?". With "automatically" on, it's turned
    // on straight away and the island offers Undo instead. It lasts until
    // you plug in or choose a mode yourself.
    property bool saverDismissed: false // asked or undone this discharge: not again
    readonly property bool low: onBattery && percent >= 0 && percent <= cfg.batterySaverLevel
    onLowChanged: if (low) offerSaver()
    onOnBatteryChanged: if (!onBattery) {
        saverDismissed = false;
        release("battery");
    }
    function saverOn() {
        ask("battery", "power-saver", "battery at " + Math.round(percent) + "%");
    }
    function offerSaver() {
        if (!cfg.batterySaver || saverDismissed || has("battery") || mode === "power-saver" || !Backends.ppd)
            return;
        saverDismissed = true;
        const p = Math.round(percent) + "%";
        if (cfg.batterySaverAuto) {
            saverOn();
            Island.post({ kind: "custom", icon: "leaf", title: "Low Power on", subtitle: "Battery at " + p, color: Theme.accents.yellow, action: "Undo", onAction: () => root.release("battery") }, 8000);
        } else {
            Island.post({ kind: "custom", icon: "leaf", title: "Battery at " + p, subtitle: "Turn on Low Power?", color: Theme.accents.yellow, action: "Turn on", onAction: () => root.saverOn() }, 15000);
        }
    }
    onPercentChanged: if (has("battery")) saverOn() // (the reason shows the level)

    // Game Mode asks for Performance
    readonly property bool gameWanted: System.gameMode && cfg.gamePerformance && PowerProfiles.hasPerformanceProfile && Backends.ppd
    onGameWantedChanged: gameWanted ? ask("game", "performance", "Game Mode until you turn it off") : release("game")

    // ── Critical battery ────────────────────────────────────────────────
    // The shell warns (10%, then 5% with Sleep now); battery-guard.py, a
    // systemd user service, sleeps at 3% even when the shell isn't there.
    readonly property int stage: cfg.critical ? Policy.batteryStage(percent, onBattery) : 0
    property int warned: 0
    onStageChanged: {
        if (stage === 0) {
            warned = 0;
            return;
        }
        if (stage <= warned)
            return;
        warned = stage;
        const p = Math.round(percent) + "%";
        if (stage === 1)
            warn("battery-low", "Battery low", p + " left. Plug in soon.", "", null);
        else if (stage === 2)
            warn("battery-warning", "Battery very low", p + " left. It sleeps at 3% to keep your work.", "Sleep now", () => System.power("sleep"));
        else if (!guardActive && !sandboxed) // the guard does it when it's running
            System.power("sleep");
    }
    function warn(icon, title, text, action, act) {
        Sounds.play("battery");
        if (Island.enabled)
            Island.post(Object.assign({ kind: "custom", icon: icon, title: title, subtitle: text, color: Theme.color.danger }, action ? { action: action, onAction: act } : {}), 15000);
        else
            Quickshell.execDetached(["notify-send", "-u", "critical", "-a", "iRice", title, text]);
    }
    // A note when it's full (the charger can come out)
    Connections {
        target: root.battery
        function onStateChanged() {
            if (root.hasBattery && root.battery.state === UPowerDeviceState.FullyCharged)
                Quickshell.execDetached(["notify-send", "-a", "Battery", "-i", "battery-full-charged", "Fully charged", "You can unplug your computer."]);
        }
    }
    readonly property string guardScript: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/battery-guard-install.sh").toString().replace("file://", ""))
    property bool guardActive: false
    property string guardError: ""
    function syncGuard() {
        if (sandboxed || !hasBattery)
            return;
        guardProc.command = ["bash", guardScript, cfg.critical ? "enable" : "disable"];
        guardProc.running = true;
    }
    Process {
        id: guardProc
        stdout: StdioCollector {
            onStreamFinished: {
                root.guardActive = text.trim() === "active";
                root.guardError = root.cfg.critical && !root.guardActive ? "The battery guard isn't running. Check that systemd's user session is working (systemctl --user status irice-battery-guard)." : "";
            }
        }
    }
    Connections {
        target: root.cfg
        function onCriticalChanged() { root.syncGuard(); }
        function onIdleDimAcChanged() { root.idleLater.restart(); }
        function onIdleLockAcChanged() { root.idleLater.restart(); }
        function onIdleOffAcChanged() { root.idleLater.restart(); }
        function onIdleSleepAcChanged() { root.idleLater.restart(); }
        function onIdleDimBatteryChanged() { root.idleLater.restart(); }
        function onIdleLockBatteryChanged() { root.idleLater.restart(); }
        function onIdleOffBatteryChanged() { root.idleLater.restart(); }
        function onIdleSleepBatteryChanged() { root.idleLater.restart(); }
        function onPasswordAfterSleepChanged() { root.idleLater.restart(); }
    }

    // ── Idle ────────────────────────────────────────────────────────────
    // start-idle.sh's hypridle reads the timers from idle-listeners; a
    // change rewrites it and restarts hypridle (start-idle.sh's loop starts
    // it again with the new list). A hypridle.conf of your own wins.
    property alias idleLater: idleTimer
    Timer {
        id: idleTimer
        interval: 800 // after a slider settles
        onTriggered: root.writeIdle()
    }
    readonly property string idleFile: Config.dir + "/idle-listeners"
    readonly property string userHypridle: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/hypr/hypridle.conf"
    // (one an earlier installer copied in, unchanged, isn't yours: own-hypridle.sh)
    readonly property string ownHypridleScript: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/own-hypridle.sh").toString().replace("file://", ""))
    property bool ownHypridle: false
    function writeIdle() {
        const lines = Policy.idleListeners(cfg).map(l => l.source + " " + l.action + " " + l.timeout);
        lines.push("grace " + Math.max(0, cfg.passwordAfterSleep));
        idleWriter.command = ["sh", "-c", "printf '%s\\n' \"$2\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"; d=\"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/irice-idle-$HYPRLAND_INSTANCE_SIGNATURE\"; p=$(cat \"$d/hypridle.pid\" 2>/dev/null); [ -n \"$p\" ] && [ \"$(cat /proc/$p/comm 2>/dev/null)\" = hypridle ] && kill \"$p\"; \"$4\" \"$3\" && echo own || echo ok", "sh", idleFile, lines.join("\n"), userHypridle, ownHypridleScript];
        idleWriter.running = true;
    }
    Process {
        id: idleWriter
        stdout: StdioCollector {
            onStreamFinished: root.ownHypridle = text.trim() === "own"
        }
    }

    // ── Lid ─────────────────────────────────────────────────────────────
    // Closing the lid is logind's to handle (it sleeps), unless a setting
    // here says otherwise: then iRice holds a handle-lid-switch inhibitor
    // and acts itself. If the shell isn't running the inhibitor goes with
    // it and logind sleeps as usual, so the lid never does nothing by
    // accident.
    readonly property bool lidMine: !sandboxed && hasBattery && Policy.lidNeedsInhibit(cfg)
    Process {
        running: root.lidMine
        command: ["setpriv", "--pdeathsig", "TERM", "--", "systemd-inhibit", "--what=handle-lid-switch", "--who=iRice", "--why=Lid settings (Battery & Power)", "--mode=block", "setpriv", "--pdeathsig", "TERM", "--", "sleep", "infinity"]
    }
    Process {
        running: root.lidMine
        command: [Tools.watcher, "gdbus", "monitor", "--system", "--dest", "org.freedesktop.login1", "--object-path", "/org/freedesktop/login1"]
        stdinEnabled: true
        stdout: SplitParser {
            onRead: line => {
                const m = line.match(/'LidClosed': <(true|false)>/);
                if (m && m[1] === "true")
                    root.lidClosed();
            }
        }
    }
    function lidClosed() {
        const external = Quickshell.screens.filter(s => !/^(eDP|LVDS|DSI)/.test(s.name)).length;
        const act = Policy.lidAction(!onBattery, external, cfg);
        if (act === "sleep")
            System.power("sleep");
        else if (act === "lock")
            System.power("lock");
    }

    // ── Apps using significant energy ───────────────────────────────────
    // A look every 5 minutes (~70 ms of CPU) keeps an hour of which apps
    // used the CPU; the report only runs while Settings shows it.
    readonly property string energyScript: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/app-energy.py").toString().replace("file://", ""))
    Timer {
        running: root.cfg.appEnergy && Config.ready
        interval: 300000
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            sampler.command = ["python3", "-I", root.energyScript, "sample"];
            sampler.running = true;
        }
    }
    Process { id: sampler }
    property int energyUsers: 0 // pages showing the report
    property var energyApps: [] // [{ key, seconds }]
    property int energyMinutes: 0
    Timer {
        running: root.energyUsers > 0 && root.cfg.appEnergy
        interval: 10000
        repeat: true
        triggeredOnStart: true
        onTriggered: reporter.running = true
    }
    Process {
        id: reporter
        command: ["python3", "-I", root.energyScript, "report"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const r = JSON.parse(text);
                    root.energyApps = r.apps ?? [];
                    root.energyMinutes = r.minutes ?? 0;
                } catch (e) {}
            }
        }
    }
    function clearEnergy() {
        energyApps = [];
        Quickshell.execDetached(["python3", "-I", energyScript, "clear"]);
    }

    Component.onCompleted: {
        syncGuard();
        idleTimer.restart();
        if (low)
            Qt.callLater(offerSaver);
        if (gameWanted)
            ask("game", "performance", "Game Mode until you turn it off");
    }
}
