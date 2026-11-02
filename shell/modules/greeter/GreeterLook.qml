import QtQuick
import Quickshell
import Quickshell.Io
import "../../common/MaterialColor.js" as MaterialColor

// Which look the login screen wears, Glass or Material. The greeter runs as
// greetd's "greeter" user, which can't read your settings (~/.config/irice is
// yours alone) and doesn't load Theme or Config, so it reads a small copy
// next to its own files, /usr/share/irice/look.json:
//   { "look": "material", "seed": "#4f6f9c", "style": "tonalSpot", "dark": true }
// (seed: the colour Material You built your scheme from, as Theme.materialSeed
// has it). Missing or unreadable: Glass, as before. IRICE_GREETER_LOOK=glass
// or material overrides it, for trying it with IRICE_GREETER_MOCK=1.
QtObject {
    id: root

    readonly property string file: Quickshell.env("IRICE_GREETER_LOOK_FILE") || Quickshell.shellDir + "/../look.json"
    property var saved: ({})

    readonly property string chosen: {
        const forced = Quickshell.env("IRICE_GREETER_LOOK");
        if (forced === "material" || forced === "glass")
            return forced;
        return saved.look === "material" ? "material" : "glass";
    }
    readonly property bool material: chosen === "material"
    // (Material's baseline seed when none was saved)
    readonly property string seed: /^#[0-9a-fA-F]{6}$/.test(String(saved.seed ?? "")) ? saved.seed : "#6750A4"
    readonly property string style: ["tonalSpot", "vibrant", "expressive", "neutral", "content", "fidelity", "monochrome"].includes(saved.style) ? saved.style : "tonalSpot"
    readonly property bool dark: saved.dark !== false
    // Every Material colour role, as Theme.m3 has them in the session
    readonly property var m3: material ? MaterialColor.MCU.scheme(seed, dark, style, 0) : ({})
    readonly property string wallpaper: Quickshell.env("IRICE_GREETER_WALLPAPER") || ""

    // Read before the windows are made, so the first frame is the right look
    property FileView reader: FileView {
        path: root.file
        blockLoading: true
        printErrors: false
        onLoaded: {
            try {
                const j = JSON.parse(text());
                root.saved = j && typeof j === "object" ? j : {};
            } catch (e) {
                root.saved = {};
            }
        }
    }
}
