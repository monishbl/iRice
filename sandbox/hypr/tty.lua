-- iRice test session on its own TTY (real Hyprland, real input; not nested).
-- Started by `./dev.sh tty` from a text console. Uses the real iRice modules
-- with SUPER as the modifier. Super+Shift+Escape (or Ctrl+Alt+Backspace)
-- ends the session; Super+Ctrl+Alt+R force-restarts a frozen shell. If the
-- whole session hangs, run `./dev.sh tty-stop` from another console.

local root = os.getenv("IRICE_ROOT")
IRICE_MOD = "SUPER"
IRICE_HYPR_DIR = root .. "/hypr/irice"
-- (the shell here keeps its config in the sandbox home)
IRICE_USER_DIR = root .. "/.sandbox/home/.config/irice"

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = 1 })
pcall(dofile, IRICE_USER_DIR .. "/monitors.lua") -- per-display settings from Settings → Displays
hl.config({
    input = {
        kb_layout = "us",
        follow_mouse = 1,
        repeat_delay = 250,
        repeat_rate = 35,
        numlock_by_default = true,
        off_window_axis_events = 2, -- (end4) scrolling outside a window goes to the nearest one
        touchpad = { natural_scroll = true, disable_while_typing = false, clickfinger_behavior = true, scroll_factor = 0.7 }
    },
    ecosystem = { no_update_news = true, no_donation_nag = true }
})

dofile(root .. "/hypr/irice/look.lua")
pcall(dofile, root .. "/hypr/irice/cursor.lua") -- macOS cursor when installed
pcall(dofile, root .. "/hypr/irice/tabs.lua") -- window tabs
dofile(root .. "/hypr/irice/rules.lua")
-- (the shell here keeps its config in the sandbox home)
pcall(dofile, root .. "/.sandbox/home/.config/irice/window-rules.lua")
dofile(root .. "/hypr/irice/keybinds.lua")
dofile(root .. "/hypr/irice/gestures.lua")
pcall(dofile, root .. "/hypr/irice/titlebars.lua") -- only with the hyprbars plugin
-- Liquid Glass plugin straight from the repo build (make -C hypr/plugin/irice-glass)
IRICE_GLASS_PLUGIN = root .. "/hypr/plugin/irice-glass/irice-glass.so"
pcall(dofile, root .. "/hypr/irice/glass.lua")

-- The shell runs on the host's Quickshell via tty-shell.sh, with config,
-- state and cache in the sandbox home; a watchdog restarts it if it freezes.
local ctl = root .. "/sandbox/tty-shell.sh"

hl.bind("SUPER + SHIFT + Escape", hl.dsp.exit(), { description = "Session: End test session" })
hl.bind("CTRL + ALT + BackSpace", hl.dsp.exit(), { description = "Session: End test session (emergency)" })
hl.bind("SUPER + CTRL + ALT + R", hl.dsp.exec_cmd(ctl .. " restart"), { description = "Session: Force-restart a frozen shell" })

hl.on("hyprland.start", function()
    -- This session's display for its own (private, dbus-run-session) bus, so
    -- its portals open here. Not for the user systemd (HYPRLAND_NO_SD_VARS,
    -- dev.sh/demo.sh): that one belongs to your usual desktop, which lost its
    -- display to this session (and, at its exit, any display at all: apps
    -- then waited ~25 s on portals that couldn't start)
    hl.exec_cmd("dbus-update-activation-environment WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE QT_QPA_PLATFORMTHEME QT_QPA_PLATFORM ELECTRON_OZONE_PLATFORM_HINT XDG_MENU_PREFIX PATH XDG_DATA_DIRS")
    hl.exec_cmd(ctl .. " start")
    hl.exec_cmd(ctl .. " watchdog")
    hl.exec_cmd("wl-paste --watch cliphist store")
    -- ./demo.sh: a playing track and a few notifications once the shell is up
    if os.getenv("IRICE_DEMO") == "1" then
        hl.exec_cmd(root .. "/sandbox/demo-extras.sh")
    end
end)
-- The watchdog keeps the shell going; this keeps the watchdog going (a second
-- one exits at once, so starting it again is harmless)
hl.timer(function() hl.exec_cmd(ctl .. " watchdog") end, { timeout = 60000, type = "repeat" })
