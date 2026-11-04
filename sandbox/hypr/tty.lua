-- Tessera test session on its own TTY (real Hyprland, real input; not nested).
-- Started by `./dev.sh tty` from a text console. Uses the real Tessera modules
-- with SUPER as the modifier. Super+Shift+Escape (or Ctrl+Alt+Backspace)
-- ends the session; Super+Ctrl+Alt+R force-restarts a frozen shell. If the
-- whole session hangs, run `./dev.sh tty-stop` from another console.

local root = (os.getenv("TESSERA_ROOT") or os.getenv("IRICE_ROOT"))
-- Tessera's folder under base, or iRice's while it hasn't been moved yet
-- (the shell moves it at its next start: migrate-names.py)
local function appDir(base)
    local f = io.open(base .. "/tessera/config.json", "r")
    if f then f:close(); return base .. "/tessera" end
    f = io.open(base .. "/irice/config.json", "r")
    if f then f:close(); return base .. "/irice" end
    return base .. "/tessera"
end
TESSERA_MOD = "SUPER"
TESSERA_HYPR_DIR = root .. "/hypr/tessera"
-- (the shell here keeps its config in the sandbox home)
TESSERA_USER_DIR = appDir(root .. "/.sandbox/home/.config")

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = 1 })
pcall(dofile, TESSERA_USER_DIR .. "/monitors.lua") -- per-display settings from Settings → Displays
hl.config({
    input = {
        kb_layout = "us", -- (only until keyboard.lua, below, sets yours)
        follow_mouse = 1,
        repeat_delay = 250,
        repeat_rate = 35,
        numlock_by_default = true,
        off_window_axis_events = 2, -- (end4) scrolling outside a window goes to the nearest one
        touchpad = { natural_scroll = true, disable_while_typing = false, clickfinger_behavior = true, scroll_factor = 0.7 }
    },
    ecosystem = { no_update_news = true, no_donation_nag = true }
})
-- Your keyboard layout: the installed Tessera's (install.sh writes it from
-- localectl), else the one tools/setup-tty.sh wrote for this folder
do
    local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    -- (hypr/irice: an installer from before iRice became Tessera wrote them there)
    if not pcall(dofile, cfg .. "/hypr/tessera/keyboard.lua") and not pcall(dofile, cfg .. "/hypr/irice/keyboard.lua") then pcall(dofile, root .. "/.sandbox/keyboard.lua") end
    if not pcall(dofile, cfg .. "/hypr/tessera/gpu.lua") then pcall(dofile, cfg .. "/hypr/irice/gpu.lua") end -- NVIDIA's settings, when the installer wrote them
end

dofile(root .. "/hypr/tessera/look.lua")
pcall(dofile, root .. "/hypr/tessera/cursor.lua") -- macOS cursor when installed
pcall(dofile, root .. "/hypr/tessera/findpointer.lua") -- shake to find the pointer
pcall(dofile, root .. "/hypr/tessera/tabs.lua") -- window tabs
dofile(root .. "/hypr/tessera/rules.lua")
-- (the shell here keeps its config in the sandbox home)
pcall(dofile, TESSERA_USER_DIR .. "/window-rules.lua")
dofile(root .. "/hypr/tessera/keybinds.lua")
dofile(root .. "/hypr/tessera/gestures.lua")
pcall(dofile, root .. "/hypr/tessera/titlebars.lua") -- only with the hyprbars plugin
-- Liquid Glass plugin straight from the repo build (make -C hypr/plugin/tessera-glass)
-- (or the one built there while Tessera was called iRice, until it's rebuilt)
TESSERA_GLASS_PLUGIN = { root .. "/hypr/plugin/tessera-glass/tessera-glass.so", root .. "/hypr/plugin/irice-glass/irice-glass.so" }
pcall(dofile, root .. "/hypr/tessera/glass.lua")

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
    hl.exec_cmd("dbus-update-activation-environment WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE QT_QPA_PLATFORMTHEME QT_QPA_PLATFORM ELECTRON_OZONE_PLATFORM_HINT XDG_MENU_PREFIX CLAUDE_PASSWORD_STORE PATH XDG_DATA_DIRS")
    hl.exec_cmd(ctl .. " start")
    hl.exec_cmd(ctl .. " watchdog")
    hl.exec_cmd("wl-paste --watch cliphist store")
    -- Apps that start with your session (XDG autostart), once per session
    -- (the switch in Settings → Login Items is in this session's own Tessera settings)
    hl.exec_cmd("TESSERA_USER_DIR='" .. TESSERA_USER_DIR .. "' python3 -I " .. root .. "/shell/assets/scripts/xdg-autostart.py --wait-tray 15")
    -- ./demo.sh: a playing track and a few notifications once the shell is up
    if (os.getenv("TESSERA_DEMO") or os.getenv("IRICE_DEMO")) == "1" then
        hl.exec_cmd(root .. "/sandbox/demo-extras.sh")
    end
end)
-- The watchdog keeps the shell going; this keeps the watchdog going (a second
-- one exits at once, so starting it again is harmless)
hl.timer(function() hl.exec_cmd(ctl .. " watchdog") end, { timeout = 60000, type = "repeat" })
