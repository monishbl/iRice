pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// CPU, memory, swap, temperature, network and GPU, sampled once a second —
// only while something shows it (`active`), so it costs nothing otherwise.
// The last minute of each is kept for graphs (`stats`, ResourceGraph).
Singleton {
    id: root

    // Sampling runs while any consumer shows it
    property bool overlayActive: false
    property bool menubarActive: false
    property bool islandActive: false // the island only needs a slow sample (alerts when it stays high)
    property int desktopUsers: 0 // things showing CPU/memory now (System widgets, the open island); slow sampling is enough
    property int graphUsers: 0 // graphs on show now (System widgets, hover popovers, Control Center): once a second
    readonly property bool active: overlayActive || menubarActive || islandActive || desktopUsers > 0 || graphUsers > 0
    readonly property bool fast: overlayActive || menubarActive || graphUsers > 0
    property real cpu: 0 // 0..1
    property real memory: 0
    property real swap: 0
    property real memUsedGb: 0
    property real memTotalGb: 0
    property real temperature: 0 // °C, 0 if unknown
    property real netDown: 0 // bytes/s, all real interfaces together
    property real netUp: 0
    property real gpu: 0 // 0..1, where the driver reports it (gpuAvailable)
    readonly property bool gpuAvailable: gpuPath !== ""
    // The last minute, oldest first (one sample a second while graphs show)
    property var cpuHistory: []
    property var memHistory: []
    property var swapHistory: []
    property var tempHistory: []
    property var netDownHistory: []
    property var netUpHistory: []
    property var gpuHistory: []
    readonly property int historyLength: 60

    property var lastCpu: null
    property var lastNet: null // { rx, tx, t }
    property real lastTick: 0

    // What graphs show, in order. `max` is the graph's top (0: scale to the
    // largest value); `tone` names a Theme accent; `value` is 0..1 for gauges.
    readonly property var stats: [
        { key: "cpu", label: "CPU", icon: "cpu", tone: "blue", value: cpu, text: Math.round(cpu * 100) + "%", detail: temperature > 0 ? Math.round(temperature) + "°C" : "", history: cpuHistory, max: 1 },
        { key: "memory", label: "Memory", icon: "memory", tone: "green", value: memory, text: Math.round(memory * 100) + "%", detail: memUsedGb.toFixed(1) + " of " + memTotalGb.toFixed(1) + " GB", history: memHistory, max: 1 },
        { key: "swap", label: "Swap", icon: "swap", tone: "purple", value: swap, text: Math.round(swap * 100) + "%", detail: "", history: swapHistory, max: 1 },
        ...(tempPath !== "" ? [{ key: "temperature", label: "Temperature", icon: "thermometer-simple", tone: "orange", value: temperature / 100, text: Math.round(temperature) + "°", detail: "", history: tempHistory, max: 100 }] : []),
        { key: "netDown", label: "Download", icon: "arrow-down", tone: "teal", value: 0, text: rate(netDown), detail: "", history: netDownHistory, max: 0 },
        { key: "netUp", label: "Upload", icon: "arrow-up", tone: "pink", value: 0, text: rate(netUp), detail: "", history: netUpHistory, max: 0 },
        ...(gpuAvailable ? [{ key: "gpu", label: "GPU", icon: "graphics-card", tone: "indigo", value: gpu, text: Math.round(gpu * 100) + "%", detail: "", history: gpuHistory, max: 1 }] : [])
    ]
    // The stats as rows (ResourceRow): processor (with temperature), the GPU
    // where its driver reports load, memory (with swap), network (both ways)
    readonly property var groups: gpuAvailable ? ["processor", "graphics", "memory", "network"] : ["processor", "memory", "network"]
    function stat(key) {
        return stats.find(s => s.key === key) ?? stats[0];
    }
    function rate(b) {
        if (b < 1024)
            return Math.round(b) + " B/s";
        if (b < 1048576)
            return (b / 1024).toFixed(b < 10240 ? 1 : 0) + " KB/s";
        return (b / 1048576).toFixed(1) + " MB/s";
    }

    function push(list, v) {
        const out = list.concat([v]);
        return out.length > historyLength ? out.slice(out.length - historyLength) : out;
    }

    Timer {
        interval: root.fast ? 1000 : 5000
        running: root.active
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            // Back after a pause: the old minute isn't the last one any more
            const now = Date.now();
            if (now - root.lastTick > 15000) {
                root.cpuHistory = root.memHistory = root.swapHistory = root.tempHistory = root.netDownHistory = root.netUpHistory = root.gpuHistory = [];
                root.lastCpu = root.lastNet = root.lastGpu = null;
            }
            root.lastTick = now;
            stat.reload();
            meminfo.reload();
            netdev.reload();
            if (root.tempPath !== "")
                tempFile.reload();
            else if (!root.tempSearched)
                findTemp.running = true;
            if (root.gpuPath !== "")
                gpuFile.reload();
            else if (!root.gpuSearched)
                findGpu.running = true;
        }
    }

    FileView {
        id: stat
        path: "/proc/stat"
        onLoaded: {
            const f = text().split("\n")[0].trim().split(/\s+/).slice(1).map(Number);
            const idle = f[3] + f[4];
            const total = f.reduce((a, b) => a + b, 0);
            if (root.lastCpu) {
                const dt = total - root.lastCpu.total;
                root.cpu = dt > 0 ? 1 - (idle - root.lastCpu.idle) / dt : 0;
                root.cpuHistory = root.push(root.cpuHistory, root.cpu);
            }
            root.lastCpu = { idle: idle, total: total };
        }
    }

    FileView {
        id: meminfo
        path: "/proc/meminfo"
        onLoaded: {
            const t = text();
            const kb = k => parseInt(t.match(new RegExp("^" + k + ":\\s*(\\d+)", "m"))?.[1] ?? "0");
            const total = kb("MemTotal"), avail = kb("MemAvailable");
            const st = kb("SwapTotal"), sf = kb("SwapFree");
            root.memory = total > 0 ? 1 - avail / total : 0;
            root.swap = st > 0 ? 1 - sf / st : 0;
            root.memTotalGb = total / 1048576;
            root.memUsedGb = (total - avail) / 1048576;
            root.memHistory = root.push(root.memHistory, root.memory);
            root.swapHistory = root.push(root.swapHistory, root.swap);
        }
    }

    // Bytes in and out since the last sample, over every interface but
    // loopback, containers, bridges and tunnels (a VPN's traffic also goes
    // through the real interface, so it would count twice)
    FileView {
        id: netdev
        path: "/proc/net/dev"
        onLoaded: {
            let rx = 0, tx = 0;
            for (const line of text().split("\n").slice(2)) {
                const i = line.indexOf(":");
                if (i < 0)
                    continue;
                const name = line.slice(0, i).trim();
                if (/^(lo|veth|docker|br-|virbr|vnet|tun|tap|wg|tailscale|zt)/.test(name))
                    continue;
                const f = line.slice(i + 1).trim().split(/\s+/).map(Number);
                rx += f[0] || 0;
                tx += f[8] || 0;
            }
            const now = Date.now();
            const last = root.lastNet;
            if (last && now > last.t) {
                const dt = (now - last.t) / 1000;
                root.netDown = Math.max(0, rx - last.rx) / dt;
                root.netUp = Math.max(0, tx - last.tx) / dt;
                root.netDownHistory = root.push(root.netDownHistory, root.netDown);
                root.netUpHistory = root.push(root.netUpHistory, root.netUp);
            }
            root.lastNet = { rx: rx, tx: tx, t: now };
        }
    }

    // The CPU package temperature (what throttling is based on): Intel's
    // x86_pkg_temp zone, else coretemp / k10temp / zenpower's package
    // reading, else the hottest CPU-ish zone. The sensor file is found once;
    // each sample then just reads it (no process per sample).
    property string tempPath: ""
    property bool tempSearched: false
    Process {
        id: findTemp
        command: ["sh", "-c", "for z in /sys/class/thermal/thermal_zone*; do [ \"$(cat $z/type 2>/dev/null)\" = x86_pkg_temp ] && { echo $z/temp; exit; }; done; for h in /sys/class/hwmon/hwmon*; do case \"$(cat $h/name 2>/dev/null)\" in coretemp|k10temp|zenpower) [ -r $h/temp1_input ] && { echo $h/temp1_input; exit; };; esac; done; for z in /sys/class/thermal/thermal_zone*; do case \"$(cat $z/type 2>/dev/null)\" in cpu*|acpitz|TCPU) echo \"$(cat $z/temp) $z/temp\";; esac; done 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.tempSearched = true;
                root.tempPath = text.trim();
                if (root.tempPath !== "")
                    tempFile.reload();
            }
        }
    }
    FileView {
        id: tempFile
        path: root.tempPath
        blockLoading: false
        onLoaded: {
            root.temperature = (parseInt(text().trim()) || 0) / 1000;
            root.tempHistory = root.push(root.tempHistory, root.temperature);
        }
        // The sensor went away (a module reloaded, hwmon renumbered): find it again
        onLoadFailed: {
            root.tempPath = "";
            root.tempSearched = false;
        }
    }

    // GPU load. amdgpu reports it (gpu_busy_percent); Intel's i915 and xe
    // report how long the GPU has slept (RC6 / idle residency, in ms), so
    // busy = 1 - slept / elapsed. Found once; with neither there's no GPU graph.
    property string gpuPath: ""
    property string gpuKind: "" // busy | idle
    property bool gpuSearched: false
    property var lastGpu: null // { ms, t }
    Process {
        id: findGpu
        command: ["sh", "-c", "for f in /sys/class/drm/card*/device/gpu_busy_percent; do [ -r \"$f\" ] && { echo \"busy $f\"; exit; }; done; for f in /sys/class/drm/card*/gt/gt0/rc6_residency_ms /sys/class/drm/card*/device/tile0/gt0/gtidle/idle_residency_ms; do [ -r \"$f\" ] && { echo \"idle $f\"; exit; }; done; true"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.gpuSearched = true;
                const f = text.trim().split(" ");
                root.gpuKind = f[0] ?? "";
                root.gpuPath = f[1] ?? "";
                if (root.gpuPath !== "")
                    gpuFile.reload();
            }
        }
    }
    FileView {
        id: gpuFile
        path: root.gpuPath
        blockLoading: false
        onLoaded: {
            const n = parseInt(text().trim()) || 0;
            if (root.gpuKind === "busy") {
                root.gpu = Math.min(100, n) / 100;
            } else {
                const now = Date.now();
                const last = root.lastGpu;
                root.lastGpu = { ms: n, t: now };
                if (!last || now <= last.t)
                    return;
                root.gpu = Math.max(0, Math.min(1, 1 - (n - last.ms) / (now - last.t)));
            }
            root.gpuHistory = root.push(root.gpuHistory, root.gpu);
        }
        onLoadFailed: {
            root.gpuPath = "";
            root.gpuSearched = false;
        }
    }
}
