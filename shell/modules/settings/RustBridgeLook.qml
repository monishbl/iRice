import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.services

// Settings drawn in Rust: the "look" page group's side in this shell
// (RustBridge.qml): Appearance, its Glass page, Wallpaper, Desktop &
// Widgets, Menu Bar & Dock and Lock & Login Screen
// (native/tessera-ui/src/surfaces/settings/pages/look). While Settings is
// open (`live`) it tells the pages what only this shell knows (`feed`, the
// pages' cx.feed["look"]) and does what they ask (`act`: cx.ask("look",
// what, args)).
Scope {
    id: root

    property bool live: false

    readonly property var feed: live ? ({
            cooldownGlass: CoolDown.glass,
            // Appearance
            sunrise: Daylight.sunrise,
            sunset: Daylight.sunset,
            path: Wallpaper.path,
            seed: String(Wallpaper.seed),
            accent: String(Wallpaper.accent).slice(0, 7),
            tone: String(Wallpaper.tone).slice(0, 7),
            emojiAvailable: EmojiFont.available,
            emojiPackage: EmojiFont.packageFor(EmojiFont.style),
            glassPlugin: GlassPlugin.available,
            glassPluginVersion: GlassPlugin.version,
            appThemeAvailable: AppTheme.available,
            // Wallpaper
            pickerBusy: FilePicker.busy,
            sets: Wallpaper.sets,
            livePoster: Wallpaper.livePoster,
            directory: Wallpaper.directory,
            rootHelper: root.passwordless,
            loginStatus: root.loginStatus,
            // Desktop & Widgets
            files: Files.installed.map(m => ({ bin: m.bin, name: m.name })),
            weather: { ready: Weather.ready, place: Weather.place, temp: Weather.temp, condition: Weather.condition, error: Weather.error, icon: Weather.icon },
            shelfCount: Shelf.count,
            shelfOpened: Shelf.opened,
            // Lock & Login Screen
            login: { installed: LoginScreen.installed, status: LoginScreen.status },
            boot: { designs: BootSplash.designs, chosen: BootSplash.chosen, pending: BootSplash.pending, busy: BootSplash.busy, blocked: BootSplash.blocked, error: BootSplash.error, done: BootSplash.done, checked: BootSplash.checked, splash: BootSplash.splash, original: BootSplash.original },
            screens: Quickshell.screens.map(s => ({ name: s.name, model: s.model ?? "", width: s.width, height: s.height }))
        }) : null
    onFeedChanged: if (live) RustUi.tell("settings", { feed: { look: feed } })

    // The login screen's copy of the wallpaper (wallpaper.qml's): through
    // Tessera's root helper (it checks it's an image, reads it as you), or,
    // before an update installed that, cp with the password
    property bool passwordless: false
    property string loginStatus: ""
    Process {
        running: root.live
        command: ["sh", "-c", RootHelper.pick + "[ -x \"$h\" ]"]
        onExited: code => root.passwordless = code === 0
    }
    Process {
        id: copyWallpaper
        command: root.passwordless ? RootHelper.command(["wallpaper", Wallpaper.path])
            : ["sh", "-c", "d=/usr/share/tessera; [ -d \"$d\" ] || d=/usr/share/irice; exec pkexec cp \"$1\" \"$d/wallpaper.jpg\"", "sh", Wallpaper.path]
        onExited: code => root.loginStatus = code === 0 ? "The login screen now uses this wallpaper."
            : root.passwordless ? "Didn't change the login screen. Pick a JPEG, PNG or WebP wallpaper and try again."
            : "Didn't change the login screen. Approve the password prompt to try again."
    }
    // Icon theme: the shell restarts once the setting is saved
    Timer {
        id: repair
        interval: 800
        onTriggered: System.repairShell()
    }

    function act(what, args) {
        switch (what) {
        case "editControls": // (Quick Settings' Edit Controls)
            Panels.settingsOpen = false;
            Panels.ccEditing = true;
            Panels.show("controlCenter");
            break;
        case "switchLook": // (a picture of the desktop in each; switch-look.sh restarts the shell in it)
            if ((args[0] === "glass" || args[0] === "material") && Config.options.appearance.look !== args[0]) {
                Config.options.appearance.look = args[0];
                Quickshell.execDetached(["setsid", "-f", Quickshell.shellDir + "/assets/scripts/switch-look.sh", args[0]]);
            }
            break;
        case "repairShell":
            repair.restart();
            break;
        case "installEmoji":
            EmojiFont.install(String(args[0] ?? EmojiFont.style));
            break;
        case "buildPlugin":
            Launch.terminal(["sh", "-c", "\"$1\" --load", "sh", ((Quickshell.env("TESSERA_ROOT") || Quickshell.env("IRICE_ROOT")) || Paths.data + "/src") + "/tools/build-plugin.sh"], true);
            break;
        case "setWallpaper":
            if (typeof args[0] === "string" && args[0].startsWith("/"))
                Wallpaper.set(args[0]);
            break;
        case "loginWallpaper":
            // (the dev sandbox must not change the real login screen)
            if (System.sandboxed) {
                root.loginStatus = "Sandbox: would copy this wallpaper to the login screen.";
                return;
            }
            copyWallpaper.running = true;
            break;
        case "editWidgets":
            Panels.settingsOpen = false;
            Panels.screen = Quickshell.screens[0];
            Panels.widgetEditing = true;
            break;
        case "shelf":
            Shelf.opened ? Shelf.hide() : Shelf.show("settings", null);
            break;
        case "previewLogin":
            LoginScreen.preview();
            break;
        case "bootRefresh":
            BootSplash.refresh();
            break;
        case "bootSet":
            BootSplash.set(String(args[0] ?? ""));
            break;
        case "playIntro":
            if (!Config.options.appearance.reduceMotion)
                LockFx.playIntro();
            break;
        case "lock":
            System.power("lock");
            break;
        default:
            console.warn("settings (look): no act " + what);
        }
    }
}
