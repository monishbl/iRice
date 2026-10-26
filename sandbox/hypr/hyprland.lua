-- iRice sandbox compositor (nested Hyprland only, never the host).
-- Loads the real iRice modules from ../../hypr with ALT as the modifier,
-- because the host Hyprland swallows SUPER.

local root = os.getenv("IRICE_ROOT")
IRICE_MOD = "ALT"

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

dofile(root .. "/hypr/irice/look.lua")
dofile(root .. "/hypr/irice/rules.lua")
dofile(root .. "/hypr/irice/keybinds.lua")
dofile(root .. "/hypr/irice/gestures.lua")
pcall(dofile, root .. "/hypr/irice/titlebars.lua") -- only with the hyprbars plugin
-- Liquid Glass plugin straight from the repo build (make -C hypr/plugin/irice-glass)
IRICE_GLASS_PLUGIN = root .. "/hypr/plugin/irice-glass/irice-glass.so"
pcall(dofile, root .. "/hypr/irice/glass.lua")

-- (MOD + SHIFT + R is recording, MOD + Return the terminal: see keybinds.lua)
hl.bind(IRICE_MOD .. " + CTRL + SHIFT + R", hl.dsp.exec_cmd(root .. "/sandbox/reload-shell.sh"), { description = "Sandbox: Reload shell" })

hl.on("hyprland.start", function()
    hl.exec_cmd("'" .. root .. "/shell/assets/scripts/launch-shell.sh' -p '" .. root .. "/shell'")
    hl.exec_cmd("wl-paste --watch cliphist store")
end)
