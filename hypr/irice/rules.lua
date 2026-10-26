-- Shell glass: liquid glass draws its own blurred backdrop; frosted glass shows
-- Hyprland's blur of the windows behind. Blur only where the glass is (alpha
-- above the soft shadows, which stay unblurred).
hl.layer_rule({ match = { namespace = "irice:.*" }, no_anim = true }) -- the shell animates itself
hl.layer_rule({ match = { namespace = "irice:.*" }, blur = true })
hl.layer_rule({ match = { namespace = "irice:.*" }, ignore_alpha = 0.4 })
-- (The menu bar takes blur: the island's glass is drawn there.)
hl.layer_rule({ match = { namespace = "irice:(wallpaper|corners)" }, blur = false })

-- Settings window floats like an app window
hl.window_rule({ match = { title = "^iRice Settings$" }, float = true })
hl.window_rule({ match = { title = "^iRice Settings$" }, center = true })
hl.window_rule({ match = { class = "^(org.pulseaudio.pavucontrol|nm-connection-editor|blueman-manager)$" }, float = true })
