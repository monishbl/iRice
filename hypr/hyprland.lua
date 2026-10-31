-- iRice Hyprland config. Your own changes go in ~/.config/hypr/custom.lua,
-- which is loaded last and survives iRice updates.
local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local dir = cfg .. "/hypr"
IRICE_HYPR_DIR = dir .. "/irice"
IRICE_USER_DIR = cfg .. "/irice"

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
pcall(dofile, IRICE_USER_DIR .. "/monitors.lua") -- per-display settings from Settings → Displays
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
pcall(dofile, dir .. "/irice/keyboard.lua") -- your keyboard layout (written by the installer, from localectl)

dofile(dir .. "/irice/look.lua")
pcall(dofile, dir .. "/irice/cursor.lua") -- macOS cursor when installed
pcall(dofile, dir .. "/irice/findpointer.lua") -- shake to find the pointer
pcall(dofile, dir .. "/irice/tabs.lua") -- window tabs
pcall(dofile, dir .. "/irice/vm.lua") -- only in a virtual machine (written by the installer)
pcall(dofile, dir .. "/irice/gpu.lua") -- only with an NVIDIA GPU (written by the installer)
dofile(dir .. "/irice/rules.lua")
pcall(dofile, cfg .. "/irice/window-rules.lua") -- per-app rules from Settings → Window Rules
dofile(dir .. "/irice/keybinds.lua")
dofile(dir .. "/irice/gestures.lua")
dofile(dir .. "/irice/execs.lua")
pcall(dofile, dir .. "/irice/titlebars.lua") -- only does anything with hyprbars
pcall(dofile, dir .. "/irice/glass.lua") -- Liquid Glass plugin, if built

-- Your old hyprland.conf's settings, all turned off until you uncomment one
-- (written once by the installer)
pcall(dofile, dir .. "/irice/from-your-old-config.lua")

-- Your tweaks. Errors here are reported but don't break the rest of the config.
local custom = dir .. "/custom.lua"
local f = io.open(custom, "r")
if f then
    f:close()
    local ok, err = pcall(dofile, custom)
    if not ok then
        print("iRice: error in custom.lua: " .. tostring(err))
    end
end
