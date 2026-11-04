-- Tessera sandbox compositor (nested Hyprland only, never the host).
-- Loads the real Tessera modules from ../../hypr with ALT as the modifier,
-- because the host Hyprland swallows SUPER.

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
TESSERA_MOD = "ALT"
TESSERA_HYPR_DIR = root .. "/hypr/tessera"

hl.monitor({ output = "WAYLAND-1", mode = "preferred", position = "0x0", scale = 1 })
-- Headless test output (created by dev.sh up). It renders even when the sandbox
-- window is hidden on the host, so screenshots work at a fixed 1920x1080.
-- Placed far from WAYLAND-1 so the pointer can never wander onto it.
hl.monitor({ output = "IRICE-TEST", mode = "1920x1080@60", position = "20000x20000", scale = 1 })
hl.config({
    input = { kb_layout = "us", follow_mouse = 1, touchpad = { natural_scroll = true } },
    -- No Hyprland update/donation pop-ups inside the test compositor
    ecosystem = { no_update_news = true, no_donation_nag = true }
})

dofile(root .. "/hypr/tessera/look.lua")
pcall(dofile, root .. "/hypr/tessera/cursor.lua") -- macOS cursor when installed
pcall(dofile, root .. "/hypr/tessera/findpointer.lua") -- shake to find the pointer
pcall(dofile, root .. "/hypr/tessera/tabs.lua") -- window tabs
dofile(root .. "/hypr/tessera/rules.lua")
pcall(dofile, appDir(os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")) .. "/window-rules.lua")
dofile(root .. "/hypr/tessera/keybinds.lua")
dofile(root .. "/hypr/tessera/gestures.lua")
pcall(dofile, root .. "/hypr/tessera/titlebars.lua") -- only with the hyprbars plugin
-- Liquid Glass plugin straight from the repo build (make -C hypr/plugin/tessera-glass)
-- (or the one built there while Tessera was called iRice, until it's rebuilt)
TESSERA_GLASS_PLUGIN = { root .. "/hypr/plugin/tessera-glass/tessera-glass.so", root .. "/hypr/plugin/irice-glass/irice-glass.so" }
pcall(dofile, root .. "/hypr/tessera/glass.lua")

-- (MOD + SHIFT + R is recording, MOD + Return the terminal: see keybinds.lua)
hl.bind(TESSERA_MOD .. " + CTRL + SHIFT + R", hl.dsp.exec_cmd(root .. "/sandbox/reload-shell.sh"), { description = "Sandbox: Reload shell" })

hl.on("hyprland.start", function()
    hl.exec_cmd("'" .. root .. "/shell/assets/scripts/launch-shell.sh' -p '" .. root .. "/shell'")
    hl.exec_cmd("wl-paste --watch cliphist store")
end)
