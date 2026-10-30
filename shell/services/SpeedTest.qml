pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Internet speed test, run only when asked (the Wi-Fi menu, the network
// graph, Settings → Network). Cloudflare's speed test through curl by
// default; Ookla's speedtest.net when chosen and installed. The work is in
// assets/scripts/speedtest.sh, which reports each step as it goes, so the
// numbers fill in live. The last results are kept in speedtest.json next to
// state.json. Also: the details of the link you're on (Wi-Fi signal, rate,
// band, channel), read when asked.
Singleton {
    id: root

    // stage: idle | latency | download | upload | done | error
    property string stage: "idle"
    readonly property bool running: stage === "latency" || stage === "download" || stage === "upload"
    readonly property string backend: Config.options.network.speedTest === "ookla" ? "ookla" : "cloudflare"

    // This run (or the last one), filled in as it goes; -1 = not measured yet
    property real ping: -1 // ms, the median idle round trip
    property real jitter: -1 // ms, how much it varies
    property real download: -1 // bits/s
    property real upload: -1
    property real loadedDown: -1 // ms, the round trip while downloading (bufferbloat)
    property real loadedUp: -1 // …while uploading
    property real live: 0 // bits/s right now, in the download or upload phase
    property real progress: 0 // 0..1 through the current phase
    property real usedBytes: 0 // data this run used
    property var server: ({}) // { name, location }
    property var client: ({}) // { ip, isp, location }
    property string resultUrl: "" // Ookla's page for the result
    property string error: "" // what went wrong, and how to fix it
    property real finishedAt: 0

    // Ookla's tools, looked up each time a speed test view opens (so one
    // installed while the shell runs shows up)
    property bool hasOokla: false // the official `speedtest`
    property bool hasOoklaPy: false // the Python `speedtest-cli`
    readonly property bool ooklaReady: hasOokla || hasOoklaPy
    readonly property string ooklaInstall: "Install Ookla's speedtest with “paru -S ookla-speedtest-bin” (or “sudo pacman -S speedtest-cli”), or switch to Cloudflare."

    // Last results, newest first:
    // [{ at, backend, download, upload, ping, jitter, loadedDown, loadedUp, server, isp, ip, network }]
    property var history: []
    readonly property var last: history[0] ?? null

    // The link: { iface, kind: wifi|wired|other, ssid, signal (dBm), quality
    // (0..1), rx, tx (Mbit/s), freq (MHz), band, channel, width, security, speed }
    property var link: ({})

    function format(bps) {
        if (bps < 0)
            return "—";
        if (bps >= 1e9)
            return (bps / 1e9).toFixed(bps >= 1e10 ? 0 : 2) + " Gbps";
        if (bps >= 1e6)
            return (bps / 1e6).toFixed(bps >= 1e8 ? 0 : 1) + " Mbps";
        return Math.round(bps / 1e3) + " kbps";
    }
    function formatMs(ms) {
        return ms < 0 ? "—" : (ms < 10 ? ms.toFixed(1) : Math.round(ms)) + " ms";
    }
    function formatBytes(b) {
        return b >= 1e9 ? (b / 1e9).toFixed(2) + " GB" : Math.round(b / 1e6) + " MB";
    }
    // How a line is for everyday use, from the last numbers
    function verdict(r) {
        if (!r || r.download < 0)
            return "";
        const lag = Math.max(r.loadedDown ?? -1, r.loadedUp ?? -1);
        if (r.download >= 100e6 && r.upload >= 20e6 && r.ping < 40 && lag < 150)
            return "Great for everything, including 4K video and calls";
        if (r.download >= 25e6 && r.upload >= 5e6 && r.ping < 80)
            return lag > 300 ? "Fast, but lag spikes when the line is busy" : "Good for HD video, calls and gaming";
        if (r.download >= 5e6)
            return "Fine for browsing and HD video; calls may stutter";
        return "Slow: pages and video will lag";
    }

    function start() {
        if (running)
            return;
        if (backend === "ookla" && !ooklaReady) {
            probe();
            error = "Ookla's speedtest isn't installed. " + ooklaInstall;
            stage = "error";
            return;
        }
        pings = [];
        loadedSamples = { download: [], upload: [] };
        ping = jitter = download = upload = loadedDown = loadedUp = -1;
        live = progress = usedBytes = 0;
        server = {};
        client = {};
        resultUrl = "";
        error = "";
        stage = "latency";
        phaseStart = Date.now();
        test.command = [Tools.watcher, script, backend === "ookla" ? (hasOokla ? "ookla" : "ookla-py") : "cloudflare"];
        test.running = true;
        readLink();
    }
    function cancel() {
        if (!running)
            return;
        cancelled = true;
        test.running = false;
    }

    // ── The run ─────────────────────────────────────────────────────────
    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/speedtest.sh").toString().replace("file://", ""))
    readonly property int phaseSecs: 9 // (as in the script)
    property var pings: []
    property var loadedSamples: ({ download: [], upload: [] })
    property real phaseStart: 0
    property bool cancelled: false

    function median(a) {
        if (a.length === 0)
            return -1;
        const s = a.slice().sort((x, y) => x - y);
        const m = Math.floor(s.length / 2);
        return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
    }
    function setPhase(p) {
        stage = p;
        phaseStart = Date.now();
        progress = 0;
        live = 0;
    }

    Process {
        id: test
        stdinEnabled: true
        stdout: SplitParser {
            onRead: line => root.onLine(line)
        }
        onExited: code => {
            if (root.cancelled) {
                root.cancelled = false;
                root.stage = root.download >= 0 ? "done" : "idle";
                if (root.download >= 0)
                    root.save();
                return;
            }
            if (root.stage === "error")
                return;
            if (root.running) { // ended without a result
                root.error = "The speed test stopped before it finished. Check your connection and try again.";
                root.stage = "error";
                return;
            }
        }
    }
    function onLine(line) {
        const sp = line.indexOf(" ");
        const kind = sp < 0 ? line : line.slice(0, sp);
        const rest = sp < 0 ? "" : line.slice(sp + 1);
        if (kind === "phase") {
            setPhase(rest);
        } else if (kind === "meta") {
            try {
                const m = JSON.parse(rest);
                client = { ip: m.clientIp ?? "", isp: m.asOrganization ?? "", location: [m.city, m.country].filter(x => x).join(", ") };
                server = { name: "Cloudflare " + (m.colo?.iata ?? ""), location: [m.colo?.city, m.colo?.cca2].filter(x => x).join(", ") };
            } catch (e) {}
        } else if (kind === "ping") {
            pings = pings.concat([+rest]);
            ping = median(pings);
            let d = 0;
            for (let i = 1; i < pings.length; i++)
                d += Math.abs(pings[i] - pings[i - 1]);
            jitter = pings.length > 1 ? d / (pings.length - 1) : -1;
            progress = pings.length / 16;
        } else if (kind === "live") {
            live = +rest;
            progress = Math.min(1, (Date.now() - phaseStart) / (phaseSecs * 1000));
        } else if (kind === "loaded") {
            const k = stage === "upload" ? "upload" : "download";
            loadedSamples[k].push(+rest);
            if (k === "upload")
                loadedUp = median(loadedSamples.upload);
            else
                loadedDown = median(loadedSamples.download);
        } else if (kind === "done") {
            const [what, bps, bytes] = rest.split(" ");
            usedBytes += +bytes;
            if (what === "download")
                download = +bps;
            else {
                upload = +bps;
                finish();
            }
        } else if (kind === "ookla") {
            onOokla(rest);
        } else if (kind === "error") {
            const sp2 = rest.indexOf(" ");
            const code = rest.slice(0, sp2);
            const msg = rest.slice(sp2 + 1);
            error = code === "offline" ? "You're offline: connect to Wi-Fi or a network, then test again."
                : code === "curl" ? "The speed test needs curl. Install it with “sudo pacman -S curl”."
                : code === "timeout" ? msg + ". Your connection may be very slow or blocked; try again, or switch the server in Settings → Network."
                : code === "ookla" ? (msg.includes("installed") ? "Ookla's speedtest isn't installed. " + ooklaInstall : "Ookla's speedtest failed: " + msg + ". Try again, or switch to Cloudflare.")
                : msg;
            stage = "error";
        }
    }
    // Ookla's official CLI: JSON lines (bandwidth in bytes/s); speedtest-cli:
    // one JSON result (bits/s)
    function onOokla(json) {
        let j;
        try {
            j = JSON.parse(json);
        } catch (e) {
            return;
        }
        if (j.type === "py") {
            const d = j.data;
            client = { ip: d.client?.ip ?? "", isp: d.client?.isp ?? "", location: d.client?.country ?? "" };
            server = { name: d.server?.sponsor ?? "", location: [d.server?.name, d.server?.country].filter(x => x).join(", ") };
            ping = d.ping ?? -1;
            download = d.download ?? -1;
            upload = d.upload ?? -1;
            usedBytes = (d.bytes_received ?? 0) + (d.bytes_sent ?? 0);
            resultUrl = d.share ?? "";
            finish();
            return;
        }
        const setServer = () => {
            if (j.server)
                server = { name: j.server.name ?? "", location: [j.server.location, j.server.country].filter(x => x).join(", ") };
            if (j.isp || j.interface)
                client = { ip: j.interface?.externalIp ?? "", isp: j.isp ?? "", location: "" };
        };
        if (j.type === "testStart") {
            setServer();
        } else if (j.type === "ping") {
            if (stage !== "latency")
                setPhase("latency");
            ping = j.ping?.latency ?? ping;
            jitter = j.ping?.jitter ?? jitter;
            progress = j.ping?.progress ?? progress;
        } else if (j.type === "download" || j.type === "upload") {
            if (stage !== j.type)
                setPhase(j.type);
            const p = j[j.type];
            live = (p?.bandwidth ?? 0) * 8;
            progress = p?.progress ?? progress;
            const lat = p?.latency?.iqm ?? -1;
            if (j.type === "download")
                loadedDown = lat;
            else
                loadedUp = lat;
        } else if (j.type === "result") {
            setServer();
            ping = j.ping?.latency ?? -1;
            jitter = j.ping?.jitter ?? -1;
            download = (j.download?.bandwidth ?? -0.125) * 8;
            upload = (j.upload?.bandwidth ?? -0.125) * 8;
            loadedDown = j.download?.latency?.iqm ?? -1;
            loadedUp = j.upload?.latency?.iqm ?? -1;
            usedBytes = (j.download?.bytes ?? 0) + (j.upload?.bytes ?? 0);
            resultUrl = j.result?.url ?? "";
            finish();
        } else if (j.type === "log" && j.level === "error") {
            error = "Ookla's speedtest failed: " + (j.message ?? "unknown error") + ". Try again, or switch to Cloudflare.";
            stage = "error";
        }
    }
    function finish() {
        live = 0;
        progress = 1;
        finishedAt = Date.now();
        stage = "done";
        save();
    }
    function save() {
        history = [{
            at: Date.now(),
            backend: backend,
            download: download,
            upload: upload,
            ping: ping,
            jitter: jitter,
            loadedDown: loadedDown,
            loadedUp: loadedUp,
            server: [server.name, server.location].filter(x => x).join(" · "),
            isp: client.isp ?? "",
            ip: client.ip ?? "",
            network: link.kind === "wifi" ? (link.ssid ?? "") : link.kind === "wired" ? "Ethernet" : ""
        }].concat(history).slice(0, 10);
        store.setText(JSON.stringify({ history: history }, null, 2));
    }
    function clearHistory() {
        history = [];
        store.setText(JSON.stringify({ history: [] }, null, 2));
    }

    readonly property string dir: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/irice"
    Process {
        running: true
        command: ["mkdir", "-p", root.dir]
    }
    FileView {
        id: store
        path: root.dir + "/speedtest.json"
        onLoaded: {
            try {
                const j = JSON.parse(text());
                if (Array.isArray(j.history))
                    root.history = j.history;
            } catch (e) {}
        }
    }

    // ── Ookla's tools ───────────────────────────────────────────────────
    function probe() {
        if (!probeProc.running)
            probeProc.running = true;
    }
    Process {
        id: probeProc
        command: ["sh", "-c", "command -v speedtest >/dev/null && speedtest --version 2>/dev/null | grep -qi ookla && echo ookla; command -v speedtest-cli >/dev/null && echo py"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.hasOokla = text.includes("ookla");
                root.hasOoklaPy = text.includes("py");
            }
        }
    }

    // ── The link ────────────────────────────────────────────────────────
    function readLink() {
        if (!linkProc.running)
            linkProc.running = true;
    }
    Process {
        id: linkProc
        command: ["sh", "-c", "dev=$(ip route get 1.1.1.1 2>/dev/null | awk '{ for (i = 1; i < NF; i++) if ($i == \"dev\") { print $(i + 1); exit } }'); echo \"iface=$dev\"; [ -z \"$dev\" ] && exit 0; " +
            "if [ -d \"/sys/class/net/$dev/wireless\" ] || [ -e \"/sys/class/net/$dev/phy80211\" ]; then echo kind=wifi; iw dev \"$dev\" link 2>/dev/null; " +
            "nmcli -t -f ACTIVE,SECURITY,SSID,SIGNAL,FREQ,RATE dev wifi list --rescan no 2>/dev/null | sed -n 's/^yes:/nm=/p' | head -n 1; " +
            "elif [ -e \"/sys/class/net/$dev/device\" ]; then echo kind=wired; echo \"speed=$(cat /sys/class/net/$dev/speed 2>/dev/null)\"; else echo kind=other; fi"]
        stdout: StdioCollector {
            onStreamFinished: {
                const l = {};
                for (const raw of text.split("\n")) {
                    const s = raw.trim();
                    let m;
                    if ((m = s.match(/^(iface|kind|security|speed)=(.*)$/)))
                        l[m[1]] = m[2];
                    else if (s.startsWith("nm=")) {
                        // NetworkManager's view: security, and the rest when iw
                        // isn't installed (nmcli -t escapes ":" inside fields)
                        const f = s.slice(3).replace(/\\:/g, "\u0001").split(":").map(x => x.replace(/\u0001/g, ":"));
                        l.security = f[0] === "--" ? "" : f[0];
                        l.nm = { ssid: f[1], percent: +f[2], freq: parseInt(f[3]), rate: parseInt(f[4]) };
                    } else if ((m = s.match(/^SSID: (.*)$/)))
                        l.ssid = m[1];
                    else if ((m = s.match(/^freq: ([\d.]+)/)))
                        l.freq = Math.round(+m[1]);
                    else if ((m = s.match(/^signal: (-?\d+)/)))
                        l.signal = +m[1];
                    else if ((m = s.match(/^(rx|tx) bitrate: ([\d.]+) MBit\/s(.*)$/))) {
                        l[m[1]] = +m[2];
                        const w = m[3].match(/(\d+)MHz/);
                        if (w)
                            l.width = +w[1];
                        l.standard = /EHT/.test(m[3]) ? "Wi-Fi 7" : /HE-/.test(m[3]) ? "Wi-Fi 6" : /VHT/.test(m[3]) ? "Wi-Fi 5" : /MCS/.test(m[3]) ? "Wi-Fi 4" : (l.standard ?? "");
                    }
                }
                if (l.nm) {
                    l.ssid = l.ssid || l.nm.ssid;
                    l.freq = l.freq || l.nm.freq || undefined;
                    l.rx = l.rx ?? (l.nm.rate || undefined);
                    if (l.signal === undefined && l.nm.percent > 0)
                        l.signal = Math.round(l.nm.percent / 2 - 100); // NetworkManager's own percent ↔ dBm mapping
                    delete l.nm;
                }
                if (l.freq) {
                    const f = l.freq;
                    l.band = f >= 5925 ? "6 GHz" : f >= 4900 ? "5 GHz" : "2.4 GHz";
                    l.channel = f === 2484 ? 14 : f < 2484 ? (f - 2407) / 5 : f >= 5925 ? (f - 5950) / 5 : (f - 5000) / 5;
                    if (l.band === "6 GHz" && l.standard !== "Wi-Fi 7")
                        l.standard = "Wi-Fi 6E";
                }
                if (l.signal !== undefined)
                    l.quality = Math.max(0, Math.min(1, (l.signal + 90) / 60)); // -90 dBm → 0, -30 → 1
                // (only NetworkManager says; without it the security isn't known)
                if (l.security === "")
                    l.security = "Open";
                if (l.security === undefined)
                    l.security = "";
                if (l.kind === "wifi" && !l.ssid)
                    l.kind = "other";
                root.link = l;
            }
        }
    }
    // Wi-Fi signal, in words
    function signalWords(dbm) {
        return dbm >= -55 ? "Excellent" : dbm >= -67 ? "Good" : dbm >= -75 ? "Fair" : "Weak";
    }
}
