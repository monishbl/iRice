pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Match apps (Settings → Appearance): GTK, Qt, terminal and browser colours
// follow iRice's accent or the wallpaper, light or dark, through matugen
// (assets/scripts/app-theme.sh). Re-applied a moment after any of those
// change; turning it off empties the generated colour files. Desktop light/dark
// preferences and Dolphin's palette follow iRice even when Match apps is off.
// In the Material look, matching apps (accent or wallpaper alike) takes the
// shell's own Material palette, Theme.m3: matugen's scheme is an older spec
// whose colours differ from the shell's, so the script fills the templates
// with these roles itself (app-theme-render.py). Off stays off.
//
// The rest of the system follows the look here too: window corners, border
// and shadow (Hyprland), the window controls the irice-glass plugin draws,
// and the pointer (Material pointer).
Singleton {
    id: root

    readonly property var cfg: Config.options.appearance
    readonly property string source: cfg.appTheme // off | accent | wallpaper
    // (Material needs no matugen: the shell has the colours)
    readonly property bool available: Tools.has("matugen") || Theme.material
    // Material: every colour role, for the script ("" in Glass, or when off)
    readonly property string roles: Theme.material && source !== "off" ? JSON.stringify(Theme.m3) : ""
    // (The accent only matters while matching is on: dragging it with Match
    // apps off mustn't rewrite and reload every app's palette)
    readonly property string key: [Config.ready, source, available, Theme.dark, source !== "off" ? Theme.accent : "", source === "wallpaper" ? Wallpaper.path : "", cfg.appThemeTargets.join(","), roles].join("|")
    property string applied: ""

    onKeyChanged: if (Tools.ready) settle.restart()
    Connections {
        target: Tools
        function onReadyChanged() { settle.restart(); }
    }
    Timer {
        id: settle
        interval: 1500 // after a slider or a wallpaper crossfade settles
        onTriggered: {
            if (!Config.ready || !Tools.ready || root.key === root.applied)
                return;
            run.running = false;
            run.command = [decodeURIComponent(Qt.resolvedUrl("../assets/scripts/app-theme.sh").toString().replace("file://", "")),
                root.source === "off" || !root.available ? "off" : Theme.dark ? "dark" : "light", String(Theme.accent).slice(0, 7), root.source, Wallpaper.path, root.cfg.appThemeTargets.join(","), Theme.dark ? "dark" : "light", root.roles];
            run.running = true;
            root.applied = root.key;
        }
    }
    Process {
        id: run
        onExited: code => UiState.data.appThemeOn = code === 0 && root.available && root.source !== "off"
    }

    // ── Windows follow the look ─────────────────────────────────────────
    // Material: round corners (a circular arc, rounding_power 2, not Glass's
    // squircle), a 1 px border, a faint tint of primary on the focused window
    // (20%) and fainter outlineVariant (15%) on the rest, and a lower, softer
    // shadow. Glass's values are hypr/irice/look.lua's, put back only if
    // this shell changed them.
    // look.lua sets Material's corners and shadow at login and after a config
    // reload; System re-applies the corner radius then (with Glass's power),
    // so this follows a moment later. Never touches what game mode turns off.
    readonly property string windowLook: !Config.ready ? "" : Theme.material ? ["material", Theme.m3.primary, Theme.m3.outlineVariant, Theme.dark].join("|") : "glass"
    property string windowApplied: ""
    onWindowLookChanged: windowSettle.restart()
    Connections {
        target: System
        function onWindowRadiusChanged() {
            if (Theme.material)
                windowSettle.restart();
        }
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "configreloaded" && Theme.material)
                windowSettle.restart();
        }
    }
    Timer {
        id: windowSettle
        interval: 250 // after System's own corner update
        onTriggered: root.applyWindows()
    }
    function applyWindows() {
        if (windowLook === "")
            return;
        if (!Theme.material) {
            if (windowApplied === "" || windowApplied === "glass")
                return;
            System.hyprEval('hl.config({ general = { col = { active_border = "rgba(ffffff38)", inactive_border = "rgba(ffffff10)" } }, decoration = { rounding_power = 4.0, shadow = { range = 24, render_power = 3, offset = { 0, 6 }, color = "rgba(00000045)" } } })');
        } else {
            const hex = c => String(c).slice(1, 7);
            System.hyprEval('hl.config({ general = { col = { active_border = "rgba(' + hex(Theme.m3.primary) + '33)", inactive_border = "rgba(' + hex(Theme.m3.outlineVariant) + '26)" } }, decoration = { rounding_power = 2.0, shadow = { range = 18, render_power = 3, offset = { 0, 4 }, color = "rgba(000000' + (Theme.dark ? "59" : "2e") + ')" } } })');
        }
        windowApplied = windowLook;
    }

    // Title bars (the optional hyprbars plugin) are styled as the config
    // loads (hypr/irice/titlebars.lua reads the look): reload it once the
    // new look is saved
    readonly property bool materialLook: Theme.material
    onMaterialLookChanged: if (Config.ready && Decorations.titlebarsActive) titlebarReload.restart()
    Timer {
        id: titlebarReload
        interval: 800 // (Config writes the file 300 ms after a change)
        onTriggered: Quickshell.execDetached(["sh", "-c", "hyprctl reload config-only >/dev/null"])
    }

    // Window controls the irice-glass plugin draws: round icon buttons on a
    // tonal pill in Material, traffic lights in Glass (its default, so Glass
    // only sends once Material has been on). A plugin built before this
    // answers "unknown verb" and keeps its traffic lights.
    readonly property string controlsStyle: Theme.material ? "material " + String(Qt.alpha(Theme.m3.surfaceContainerHigh, 1)).slice(1, 7) /* (opaque: Transparency gives the containers an alpha, "#aarrggbb") */ + " " + String(Theme.m3.onSurfaceVariant).slice(1, 7) : "glass"
    property bool controlsSent: false
    onControlsStyleChanged: sendControlsStyle()
    Connections {
        target: GlassPlugin
        function onReady() { root.sendControlsStyle(); }
    }
    function sendControlsStyle() {
        if (!GlassPlugin.available || (!Theme.material && !controlsSent))
            return;
        GlassPlugin.command("controls-style", "irice-glass controls-style " + controlsStyle);
        controlsSent = true;
    }

    // ── The pointer in the Material look ──────────────────────────────
    // Bibata (end4's, Material-like) when Settings → Appearance → Windows →
    // "Material pointer" is on and it's installed; otherwise the pointer
    // chosen for both looks. hypr/irice/cursor.lua sets the Glass one at login.
    readonly property string cursorTheme: Config.options.appearance.materialCursor || cfg.cursor === "bibata" ? "Bibata-Modern-Classic" : "macOS"
    readonly property string cursorFollows: Config.ready ? cursorTheme : ""
    onCursorFollowsChanged: if (cursorFollows !== "") cursorSettle.restart()
    Timer {
        id: cursorSettle
        interval: 300 // after System's own update, which knows only macOS and Bibata
        onTriggered: Quickshell.execDetached(["sh", "-c", "found() { for d in /usr/share/icons \"$HOME/.local/share/icons\" \"$HOME/.icons\"; do [ -d \"$d/$1\" ] && return 0; done; return 1; }; t=$1; found \"$t\" || t=macOS; h=$t; [ \"$t\" = macOS ] && found macOS-hypr && h=macOS-hypr; hyprctl setcursor \"$h\" 24 >/dev/null; gsettings set org.gnome.desktop.interface cursor-theme \"$t\" 2>/dev/null", "sh", root.cursorTheme])
    }
}
