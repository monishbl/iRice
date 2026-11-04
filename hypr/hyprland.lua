-- Tessera Hyprland config. Your own changes go in ~/.config/hypr/custom.lua,
-- which is loaded last and survives Tessera updates.
local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local dir = cfg .. "/hypr"
TESSERA_HYPR_DIR = dir .. "/tessera"
-- (iRice's settings folder while it hasn't been moved yet: the shell moves it
-- at its next start, migrate-names.py)
TESSERA_USER_DIR = cfg .. "/tessera"
do
    local new, old = io.open(cfg .. "/tessera/config.json", "r"), io.open(cfg .. "/irice/config.json", "r")
    if new then new:close() elseif old then TESSERA_USER_DIR = cfg .. "/irice" end
    if old then old:close() end
end

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
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
    }
})
pcall(dofile, dir .. "/tessera/keyboard.lua") -- your keyboard layout (written by the installer, from localectl)

dofile(dir .. "/tessera/look.lua")
pcall(dofile, dir .. "/tessera/cursor.lua") -- macOS cursor when installed
pcall(dofile, dir .. "/tessera/findpointer.lua") -- shake to find the pointer
pcall(dofile, dir .. "/tessera/tabs.lua") -- window tabs
pcall(dofile, dir .. "/tessera/vm.lua") -- only in a virtual machine (written by the installer)
pcall(dofile, dir .. "/tessera/gpu.lua") -- only with an NVIDIA GPU (written by the installer)
dofile(dir .. "/tessera/rules.lua")
pcall(dofile, TESSERA_USER_DIR .. "/window-rules.lua") -- per-app rules from Settings → Window Rules
dofile(dir .. "/tessera/keybinds.lua")
dofile(dir .. "/tessera/gestures.lua")
dofile(dir .. "/tessera/execs.lua")
pcall(dofile, dir .. "/tessera/titlebars.lua") -- only does anything with hyprbars
pcall(dofile, dir .. "/tessera/glass.lua") -- Liquid Glass plugin, if built

-- Your old hyprland.conf's settings, all turned off until you uncomment one
-- (written once by the installer)
pcall(dofile, dir .. "/tessera/from-your-old-config.lua")

-- Your tweaks. Errors here are reported but don't break the rest of the config.
local custom = dir .. "/custom.lua"
local f = io.open(custom, "r")
if f then
    f:close()
    local ok, err = pcall(dofile, custom)
    if not ok then
        print("Tessera: error in custom.lua: " .. tostring(err))
    end
end
