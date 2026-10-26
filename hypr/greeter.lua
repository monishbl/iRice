-- iRice login screen: the compositor greetd runs for its "greeter" user
-- (installed to /usr/share/irice by ./install.sh --greeter). It shows the
-- iRice greeter and exits once you log in, so greetd can start your session.

hl.config({
    general = { gaps_in = 0, gaps_out = 0, border_size = 0 },
    decoration = {
        rounding = 0,
        blur = { enabled = true, size = 8, passes = 3, new_optimizations = true, xray = false, popups = false }
    },
    animations = { enabled = false }, -- the greeter animates itself
    misc = { disable_hyprland_logo = true, disable_splash_rendering = true, force_default_wallpaper = 0 },
    ecosystem = { no_update_news = true, no_donation_nag = true }
})

hl.layer_rule({ match = { namespace = "irice:.*" }, blur = true })
hl.layer_rule({ match = { namespace = "irice:.*" }, ignore_alpha = 0.4 })
hl.layer_rule({ match = { namespace = "irice:wallpaper" }, blur = false })

hl.on("hyprland.start", function()
    hl.exec_cmd("sh -c 'IRICE_GREETER_WALLPAPER=/usr/share/irice/wallpaper.jpg qs -p /usr/share/irice/shell/greeter.qml; hyprctl dispatch \"hl.dsp.exit()\"'")
end)
