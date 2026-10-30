pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// Display brightness. Built-in panels through brightnessctl; external
// monitors through ddcutil (DDC/CI), when it's installed. `value`, `set()`
// and `available` act on the display that has focus when it's an external
// with DDC, else on the built-in backlight, so the keys, the slider and the
// OSD follow the display you're working on.
//
// Backlight: writes are coalesced so dragging a slider doesn't spawn a
// process per pixel. With several backlight devices (a laptop with both
// intel_backlight and acpi_video0, say), the one with the finest steps
// (largest max_brightness) is used, as it's normally the one that really
// drives the panel.
//
// DDC: `ddcutil detect` takes seconds, so it runs in the background a little
// after start and again when displays come and go; each display's value is
// read once when it's found and when it gets focus. Writes are coalesced per
// display (a DDC write takes about 50-200 ms).
Singleton {
    id: root

    // ── What the controls use ───────────────────────────────────────────
    readonly property var target: ddcDisplays.find(d => d.output === (Hyprland.focusedMonitor?.name ?? "")) ?? null
    readonly property bool external: target !== null
    readonly property real value: target ? (ddcValues[target.output] ?? 0.5) : backlightValue
    readonly property bool available: target !== null || backlightAvailable
    readonly property string displayName: target ? target.model : ""

    function set(v) {
        if (target)
            return setDdc(target.output, v);
        if (!backlightAvailable)
            return;
        backlightValue = Math.max(0.01, Math.min(1, v));
        pending = backlightValue;
        if (!writer.running)
            flush();
    }

    // ── Built-in backlight (brightnessctl) ──────────────────────────────
    property real backlightValue: 0.5
    property bool backlightAvailable: false
    property string device: ""
    property real pending: -1

    function flush() {
        if (pending < 0)
            return;
        writer.command = ["brightnessctl", "-q"].concat(device !== "" ? ["-d", device] : [], ["set", Math.round(pending * 100) + "%"]);
        pending = -1;
        writer.running = true;
    }

    Process {
        id: writer
        onExited: root.flush()
    }

    // device,class,current,percent,max per line
    Process {
        running: true
        command: ["brightnessctl", "-m", "-l", "-c", "backlight"]
        stdout: StdioCollector {
            onStreamFinished: {
                let best = null;
                for (const l of text.trim().split("\n")) {
                    const f = l.split(",");
                    if (f.length < 5 || !(parseInt(f[4]) > 0))
                        continue;
                    if (!best || parseInt(f[4]) > parseInt(best[4]))
                        best = f;
                }
                if (best) {
                    root.device = best[0];
                    root.backlightValue = parseInt(best[2]) / parseInt(best[4]);
                    root.backlightAvailable = true;
                }
            }
        }
    }

    // ── External monitors (ddcutil) ─────────────────────────────────────
    readonly property bool ddcInstalled: Tools.has("ddcutil")
    property var ddcDisplays: [] // [{ output: "DP-1", bus: 12, model: "BenQ GW2786TC" }]
    property var ddcValues: ({}) // output → 0..1
    property var ddcMax: ({}) // output → the monitor's own maximum (usually 100)
    // Why external brightness can't work, for Settings → Displays:
    // "" (fine or not checked), "module" (no /dev/i2c-*: i2c-dev isn't
    // loaded) or "access" (the user can't open /dev/i2c-*: not in group i2c)
    property string ddcProblem: ""
    property bool ddcScanned: false
    readonly property bool ddcScanning: detect.running

    function rescanDdc() {
        if (!ddcInstalled)
            return;
        if (detect.running)
            detectAgain = true;
        else
            detect.running = true;
    }
    property bool detectAgain: false

    onDdcInstalledChanged: if (ddcInstalled)
        startScan.restart()
    // A few seconds after start (not in the startup rush), and once the
    // displays have settled after one connects or leaves
    Timer {
        id: startScan
        interval: 4000
        onTriggered: root.rescanDdc()
    }
    Connections {
        target: Quickshell
        function onScreensChanged() {
            if (root.ddcInstalled)
                startScan.restart();
        }
    }

    Process {
        id: detect
        // First: are the i2c devices there, and may we open them?
        command: ["sh", "-c", "set -- /dev/i2c-*; [ -e \"$1\" ] || { echo 'PROBLEM module'; exit 0; }; ok=; for d; do [ -r \"$d\" ] && [ -w \"$d\" ] && ok=1; done; [ -n \"$ok\" ] || { echo 'PROBLEM access'; exit 0; }; exec ddcutil detect --brief 2>&1"]
        stdout: StdioCollector {
            onStreamFinished: root.parseDetect(text)
        }
        onExited: if (root.detectAgain) {
            root.detectAgain = false;
            running = true;
        }
    }
    function parseDetect(text) {
        ddcScanned = true;
        const p = text.match(/^PROBLEM (\w+)/);
        if (p) {
            ddcProblem = p[1];
            ddcDisplays = [];
            return;
        }
        ddcProblem = /permission denied|ddcutil requires module i2c-dev/i.test(text) ? (/i2c-dev/i.test(text) ? "module" : "access") : "";
        // Blocks start with "Display N" (usable) or "Invalid display"
        const found = [];
        for (const block of text.split(/\n\s*\n/)) {
            if (!/^\s*Display \d+/m.test(block))
                continue;
            const bus = block.match(/I2C bus:\s*\/dev\/i2c-(\d+)/);
            const conn = block.match(/DRM[ _]connector:\s*card\d+-(\S+)/);
            const mon = block.match(/Monitor:\s*([^\n]*)/);
            if (!bus || !conn)
                continue;
            const parts = (mon ? mon[1] : "").split(":");
            found.push({ output: conn[1], bus: parseInt(bus[1]), model: (parts[1] || conn[1]).trim() });
        }
        ddcDisplays = found;
        for (const d of found)
            readDdc(d.output);
    }

    // Reads, one at a time
    property var readQueue: []
    function readDdc(output) {
        if (reader.running && reader.output === output)
            return;
        if (readQueue.indexOf(output) < 0)
            readQueue = readQueue.concat([output]);
        nextRead();
    }
    function nextRead() {
        if (reader.running || ddcWriter.running || readQueue.length === 0)
            return;
        const d = ddcDisplays.find(x => x.output === readQueue[0]);
        readQueue = readQueue.slice(1);
        if (!d)
            return nextRead();
        reader.output = d.output;
        readAt = Object.assign({}, readAt, { [d.output]: Date.now() });
        reader.command = ["ddcutil", "--bus", String(d.bus), "--brief", "getvcp", "10"];
        reader.running = true;
    }
    Process {
        id: reader
        property string output: ""
        // "VCP 10 C 50 100" (current, max)
        stdout: StdioCollector {
            onStreamFinished: {
                const m = text.match(/VCP\s+10\s+C\s+(\d+)\s+(\d+)/);
                if (m && parseInt(m[2]) > 0) {
                    root.ddcMax = Object.assign({}, root.ddcMax, { [reader.output]: parseInt(m[2]) });
                    // (don't undo a change made while this read was running)
                    if (root.ddcPending[reader.output] === undefined)
                        root.ddcValues = Object.assign({}, root.ddcValues, { [reader.output]: parseInt(m[1]) / parseInt(m[2]) });
                }
            }
        }
        onExited: {
            root.flushDdc();
            root.nextRead();
        }
    }
    // Focus moved to an external: read it again (its own buttons may have
    // changed it)
    // (not again right after a read)
    property var readAt: ({}) // output → ms of its last read
    onTargetChanged: if (target && Date.now() - (readAt[target.output] ?? 0) > 10000)
        readDdc(target.output)

    property var ddcPending: ({}) // output → 0..1 waiting to be written
    function setDdc(output, v) {
        const x = Math.max(0, Math.min(1, v));
        ddcValues = Object.assign({}, ddcValues, { [output]: x });
        ddcPending = Object.assign({}, ddcPending, { [output]: x });
        flushDdc();
    }
    function flushDdc() {
        if (ddcWriter.running || reader.running)
            return;
        const outs = Object.keys(ddcPending);
        if (outs.length === 0)
            return nextRead();
        const out = outs[0];
        const v = ddcPending[out];
        const p = Object.assign({}, ddcPending);
        delete p[out];
        ddcPending = p;
        const d = ddcDisplays.find(x => x.output === out);
        if (!d)
            return flushDdc();
        ddcWriter.command = ["ddcutil", "--bus", String(d.bus), "--noverify", "setvcp", "10", String(Math.round(v * (ddcMax[out] ?? 100)))];
        ddcWriter.running = true;
    }
    Process {
        id: ddcWriter
        onExited: root.flushDdc()
    }

    // qs ipc call brightness rescan | status
    IpcHandler {
        target: "brightness"
        function rescan(): void {
            root.rescanDdc();
        }
        function status(): string {
            return JSON.stringify({ target: root.target, value: root.value, backlight: root.backlightAvailable, ddc: root.ddcDisplays, problem: root.ddcProblem, installed: root.ddcInstalled });
        }
    }
}
