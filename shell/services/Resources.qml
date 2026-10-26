pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// CPU, memory, swap and temperature, sampled once a second — only while
// something shows it (`active`), so it costs nothing otherwise.
Singleton {
    id: root

    // Sampling runs while any consumer shows it
    property bool overlayActive: false
    property bool menubarActive: false
    property bool islandActive: false // the island only needs a slow sample (alerts when it stays high)
    property int desktopUsers: 0 // System widgets on the desktop (slow sampling is enough)
    readonly property bool active: overlayActive || menubarActive || islandActive || desktopUsers > 0
    readonly property bool fast: overlayActive || menubarActive
    property real cpu: 0 // 0..1
    property real memory: 0
    property real swap: 0
    property real memUsedGb: 0
    property real memTotalGb: 0
    property real temperature: 0 // °C, 0 if unknown
    property var cpuHistory: []
    property var memHistory: []
    readonly property int historyLength: 60

    property var lastCpu: null

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
            stat.reload();
            meminfo.reload();
            temp.running = true;
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
        }
    }

    // The CPU package temperature (what throttling is based on): Intel's
    // x86_pkg_temp zone, else coretemp / k10temp / zenpower's package
    // reading, else the hottest CPU-ish zone
    Process {
        id: temp
        command: ["sh", "-c", "for z in /sys/class/thermal/thermal_zone*; do [ \"$(cat $z/type 2>/dev/null)\" = x86_pkg_temp ] && { cat $z/temp; exit; }; done; for h in /sys/class/hwmon/hwmon*; do case \"$(cat $h/name 2>/dev/null)\" in coretemp|k10temp|zenpower) cat $h/temp1_input 2>/dev/null && exit;; esac; done; for z in /sys/class/thermal/thermal_zone*; do case \"$(cat $z/type 2>/dev/null)\" in cpu*|acpitz|TCPU) cat $z/temp;; esac; done 2>/dev/null | sort -n | tail -1"]
        stdout: StdioCollector {
            onStreamFinished: root.temperature = (parseInt(text.trim()) || 0) / 1000
        }
    }
}
