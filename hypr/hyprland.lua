-- iRice Hyprland config. Your own changes go in ~/.config/hypr/custom.lua,
-- which is loaded last and survives iRice updates.
local dir = os.getenv("HOME") .. "/.config/hypr"

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
hl.config({
    input = {
        kb_layout = "us",
        follow_mouse = 1,
        repeat_delay = 250,
        repeat_rate = 35,
        touchpad = { natural_scroll = true, clickfinger_behavior = true, scroll_factor = 0.7 }
    }
})

dofile(dir .. "/irice/look.lua")
dofile(dir .. "/irice/rules.lua")
dofile(dir .. "/irice/keybinds.lua")
dofile(dir .. "/irice/gestures.lua")
dofile(dir .. "/irice/execs.lua")
pcall(dofile, dir .. "/irice/titlebars.lua") -- only does anything with hyprbars
pcall(dofile, dir .. "/irice/glass.lua") -- Liquid Glass plugin, if built

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
