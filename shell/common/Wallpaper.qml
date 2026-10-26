pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Current wallpaper plus colours derived from it: an accent for tinting, and
// whether the menu bar strip is light (so menu bar text can flip to dark).
Singleton {
    id: root

    readonly property string bundledDir: decodeURIComponent(Qt.resolvedUrl("../assets/wallpapers").toString().replace("file://", ""))
    readonly property var cfg: Config.options.wallpaper
    readonly property string stillPath: cfg.path || bundledDir + "/tahoe-dusk.jpg"
    // The picture showing now: the chosen one, or the dynamic one for this time
    // of day / appearance. A live wallpaper uses its first frame here (for glass).
    readonly property string path: livePoster !== "" ? livePoster : cfg.dynamic === "time" ? timePicture : cfg.dynamic === "appearance" ? (Theme.dark ? (cfg.darkPath || bundledDir + "/tahoe-dusk.jpg") : (cfg.lightPath || bundledDir + "/tahoe-day.jpg")) : stillPath
    // Each path segment encoded, so names with # % ? still load
    readonly property url source: fileUrl(path)
    function fileUrl(p) {
        return p ? "file://" + p.split("/").map(encodeURIComponent).join("/") : "";
    }

    // Time of day: a folder's pictures in name order. The first half spans
    // sunrise to sunset, the rest the night. The bundled set gets phases that
    // match its pictures: dawn around sunrise, dusk around sunset.
    property var folderPictures: []
    readonly property string timePicture: {
        const m = Daylight.minute, r = Daylight.sunrise, st = Daylight.sunset;
        if (!cfg.dynamicFolder) {
            const p = n => bundledDir + "/tahoe-" + n + ".jpg";
            // Minutes since an event, wrapped, so phases crossing midnight work
            const since = t => (m - t + 1440) % 1440;
            if (since(r - 60) < 150) return p("dawn"); // sunrise -60 .. +90
            if (since(st - 60) < 150) return p("dusk"); // sunset -60 .. +90
            if (since(r + 90) < ((st - 60) - (r + 90) + 1440) % 1440) return p("day");
            return p("night");
        }
        const pics = folderPictures;
        if (pics.length === 0)
            return stillPath;
        const dayN = Math.ceil(pics.length / 2), nightN = pics.length - dayN;
        if (m >= r && m < st || nightN === 0)
            return pics[Math.min(dayN - 1, Math.floor(((m - r + 1440) % 1440) / Math.max(1, st - r) * dayN))];
        const nightLen = 1440 - (st - r);
        return pics[dayN + Math.min(nightN - 1, Math.floor(((m - st + 1440) % 1440) / nightLen * nightN))];
    }
    Process {
        id: folderList
        command: ["sh", "-c", "find -L \"$1\" -maxdepth 1 -type f \\( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' \\) | sort", "sh", root.cfg.dynamicFolder]
        stdout: StdioCollector {
            onStreamFinished: root.folderPictures = text.split("\n").filter(l => l !== "")
        }
    }

    // Re-list whenever the folder (or the mode) changes; set imperatively so
    // the command already has the new folder when it starts
    readonly property string timeFolder: cfg.dynamic === "time" ? cfg.dynamicFolder : ""
    onTimeFolderChanged: {
        folderPictures = [];
        folderList.running = false;
        if (timeFolder !== "")
            folderList.running = true;
    }

    // Live wallpaper poster: one frame, extracted once with ffmpeg (if present)
    property string livePoster: ""
    readonly property string liveVideo: cfg.live
    onLiveVideoChanged: updatePoster()
    Component.onCompleted: updatePoster()
    function updatePoster() {
        livePoster = "";
        if (liveVideo === "")
            return;
        const out = cacheDir + "/live-poster-" + Qt.md5(liveVideo) + ".jpg";
        posterProc.running = false; // a run for an older video is abandoned
        posterProc.out = out;
        posterProc.video = liveVideo;
        posterProc.command = ["sh", "-c", "[ -s \"$2\" ] || ffmpeg -loglevel error -y -ss 1 -i \"$1\" -frames:v 1 -q:v 3 \"$2\"; [ -s \"$2\" ]", "sh", liveVideo, out];
        posterProc.running = true;
    }
    Process {
        id: posterProc
        property string out
        property string video
        // Only if it's still the video in use (it may have changed meanwhile)
        onExited: code => root.livePoster = code === 0 && video === root.liveVideo && video !== "" ? out : ""
    }
    readonly property string directory: Config.options.wallpaper.directory || Quickshell.env("HOME") + "/Pictures/Wallpapers"

    // ── Wallpaper sets in the wallpaper folder (and one level down) ─────
    //   pair: "<Name> Light" + "<Name> Dark" pictures, switching with the theme
    //   time: a sub-folder of pictures in name order, following the day
    // [{ kind: "pair"|"time", name, light, dark, folder, preview }]
    property var sets: []
    function rescanSets() {
        setScan.running = false;
        setScan.running = true;
    }
    onDirectoryChanged: rescanSets()
    Process {
        id: setScan
        running: true
        command: [decodeURIComponent(Qt.resolvedUrl("../assets/scripts/wallpaper-sets.sh").toString().replace("file://", "")), root.directory]
        stdout: StdioCollector {
            onStreamFinished: {
                // Newest macOS first, then the rest by name
                const order = ["tahoe beach", "tahoe", "sequoia", "sonoma", "ventura", "monterey", "big sur", "catalina", "mojave"];
                const rank = n => {
                    const i = order.indexOf(n.toLowerCase());
                    return i < 0 ? order.length : i;
                };
                root.sets = text.split("\n").filter(l => l !== "").map(l => {
                    const [kind, name, a, b] = l.split("\t");
                    return kind === "pair" ? { kind: kind, name: name, light: a, dark: b, preview: a } : { kind: kind, name: name, folder: a, preview: b };
                }).sort((x, y) => rank(x.name) - rank(y.name) || x.name.localeCompare(y.name));
            }
        }
    }
    function useSet(s) {
        Config.options.wallpaper.live = "";
        if (s.kind === "pair") {
            Config.options.wallpaper.lightPath = s.light;
            Config.options.wallpaper.darkPath = s.dark;
            Config.options.wallpaper.dynamic = "appearance";
        } else {
            Config.options.wallpaper.dynamicFolder = s.folder;
            Config.options.wallpaper.dynamic = "time";
        }
    }
    function isCurrentSet(s) {
        return s.kind === "pair" ? cfg.dynamic === "appearance" && cfg.lightPath === s.light && cfg.darkPath === s.dark : cfg.dynamic === "time" && cfg.dynamicFolder === s.folder;
    }

    // Choosing a picture turns dynamic and live wallpapers off
    function set(p) {
        Config.options.wallpaper.dynamic = "off";
        Config.options.wallpaper.live = "";
        Config.options.wallpaper.path = p;
    }


    // Blurred backdrop for liquid glass, baked per screen by the background
    // window. The version bump makes every glass window reload it.
    readonly property string cacheDir: (Quickshell.env("XDG_CACHE_HOME") || Quickshell.env("HOME") + "/.cache") + "/irice"
    property var backdropVersions: ({})
    function backdropPath(screenName) {
        return cacheDir + "/backdrop-" + screenName + ".png";
    }
    function backdropUrl(screenName) {
        const v = backdropVersions[screenName];
        return v === undefined ? "" : "file://" + backdropPath(screenName) + "?v=" + v;
    }
    function backdropBaked(screenName) {
        const v = Object.assign({}, backdropVersions);
        v[screenName] = (v[screenName] ?? 0) + 1;
        backdropVersions = v;
    }

    readonly property color accent: pickAccent(palette.colors)
    // Set by the background window, which samples the menu bar strip
    property bool topIsLight: false

    function luminance(c) {
        return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
    }

    // Most saturated of the dominant colours, so dark wallpapers still tint.
    function pickAccent(colors) {
        let best = Qt.color("#808080"), score = -1;
        for (const c of colors) {
            const s = c.hsvSaturation * (0.4 + c.hsvValue);
            if (s > score) {
                score = s;
                best = c;
            }
        }
        return best;
    }

    Process {
        running: true
        command: ["mkdir", "-p", root.cacheDir]
    }

    ColorQuantizer {
        id: palette
        source: root.source
        depth: 3
        rescaleSize: 64
    }
}
