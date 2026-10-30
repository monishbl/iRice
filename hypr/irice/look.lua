-- iRice look: gaps, borders, rounding, blur (for frosted glass) and motion.

-- KDE's platform theme loads the application palette before graphics views
-- are constructed. Applying a scheme later leaves Dolphin's cached item text
-- black until the next palette-change event. Respect an explicit user choice.
local qtTheme = os.getenv("QT_QPA_PLATFORMTHEME")
if not qtTheme or qtTheme == "" then hl.env("QT_QPA_PLATFORMTHEME", "kde") end

-- Apps run natively on Wayland where they can (as end4 had it). Without the
-- hint, Electron apps (WhatsApp, Discord, Claude, Slack, VS Code…) start under
-- XWayland: files dragged in from Wayland apps like Dolphin don't drop, and
-- their windows behave a little differently. Qt apps likewise prefer Wayland,
-- with X11 as the fallback. Plasma's menu prefix gives Dolphin its full
-- "Open with" list. An explicit choice in the environment wins.
local function default_env(name, value)
    local v = os.getenv(name)
    if not v or v == "" then hl.env(name, value) end
end
default_env("ELECTRON_OZONE_PLATFORM_HINT", "auto")
default_env("QT_QPA_PLATFORM", "wayland;xcb")
default_env("XDG_MENU_PREFIX", "plasma-")
-- Claude Desktop keeps its sign-in in its own encrypted file, not KWallet:
-- the wallet isn't unlocked at login here, so it asked for the wallet
-- password at every start
default_env("CLAUDE_PASSWORD_STORE", "basic")

hl.config({
    general = {
        gaps_in = 5,
        gaps_out = 10,
        border_size = 1,
        col = {
            active_border = "rgba(ffffff38)",
            inactive_border = "rgba(ffffff10)"
        },
        resize_on_border = true,
        layout = "dwindle"
    },
    decoration = {
        rounding = 20, -- Settings → Appearance → Window corners changes this live
        rounding_power = 4.0, -- continuous (squircle) corners, like macOS and iOS
        active_opacity = 1.0,
        inactive_opacity = 1.0,
        shadow = {
            enabled = true,
            range = 24,
            render_power = 3,
            offset = { 0, 6 },
            color = "rgba(00000045)"
        },
        blur = {
            enabled = true,
            xray = false, -- frosted glass must see the windows behind it
            new_optimizations = true,
            size = 8,
            passes = 3,
            noise = 0.02,
            contrast = 1.0,
            brightness = 1.0,
            vibrancy = 0.2,
            -- Pop-up menus (iRice's dropdowns, and apps' translucent menus) are
            -- frosted glass too; fully opaque menus look the same as before
            popups = true,
            popups_ignorealpha = 0.2 -- (not their soft shadows)
        }
    },
    animations = { enabled = true },
    dwindle = { preserve_split = true },
    misc = {
        disable_hyprland_logo = true,
        disable_splash_rendering = true,
        force_default_wallpaper = 0,
        -- Black until the wallpaper fades in (the shell's background is black too)
        background_color = 0xff000000,
        -- Give newly opened or activated app windows keyboard focus immediately.
        focus_on_activate = true,
        -- If the shell restarts while the screen is locked, its new copy takes
        -- the lock back (instead of Hyprland's "lockscreen app died" screen)
        allow_session_lock_restore = true
    }
})

-- Motion: quick, decelerating, a touch of spring on windows
hl.curve("irice", { type = "bezier", points = { { 0.2, 0.9 }, { 0.1, 1.0 } } })
hl.curve("iriceSpring", { type = "bezier", points = { { 0.25, 1.15 }, { 0.5, 1.0 } } })
hl.animation({ leaf = "windowsIn", enabled = true, speed = 3.5, bezier = "iriceSpring", style = "popin 92%" })
hl.animation({ leaf = "windowsOut", enabled = true, speed = 2.5, bezier = "irice", style = "popin 92%" })
hl.animation({ leaf = "windowsMove", enabled = true, speed = 3.5, bezier = "irice" })
hl.animation({ leaf = "fade", enabled = true, speed = 2.5, bezier = "irice" })
hl.animation({ leaf = "layers", enabled = true, speed = 2.5, bezier = "irice", style = "fade" })
hl.animation({ leaf = "workspaces", enabled = true, speed = 4.0, bezier = "irice", style = "slide" })
