-- iRice test session on its own TTY (real Hyprland, real input; not nested).
-- Started by `./dev.sh tty` from a text console. Uses the real iRice modules
-- with SUPER as the modifier. Super+Shift+Escape (or Ctrl+Alt+Backspace)
-- ends the session; Super+Ctrl+Alt+R force-restarts a frozen shell. If the
-- whole session hangs, run `./dev.sh tty-stop` from another console.

local root = os.getenv("IRICE_ROOT")
IRICE_MOD = "SUPER"

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = 1 })
hl.config({
    input = {
        kb_layout = "us",
        follow_mouse = 1,
        repeat_delay = 250,
        repeat_rate = 35,
        touchpad = { natural_scroll = true, clickfinger_behavior = true, scroll_factor = 0.7 }
    },
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

-- The shell runs on the host's Quickshell via tty-shell.sh, with config,
-- state and cache in the sandbox home; a watchdog restarts it if it freezes.
local ctl = root .. "/sandbox/tty-shell.sh"

hl.bind("SUPER + SHIFT + Escape", hl.dsp.exit(), { description = "Session: End test session" })
hl.bind("CTRL + ALT + BackSpace", hl.dsp.exit(), { description = "Session: End test session (emergency)" })
hl.bind("SUPER + CTRL + ALT + R", hl.dsp.exec_cmd(ctl .. " restart"), { description = "Session: Force-restart a frozen shell" })

hl.on("hyprland.start", function()
    hl.exec_cmd(ctl .. " start")
    hl.exec_cmd(ctl .. " watchdog")
    hl.exec_cmd("wl-paste --watch cliphist store")
end)
