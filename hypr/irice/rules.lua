-- Shell glass: liquid glass draws its own blurred backdrop; frosted glass shows
-- Hyprland's blur of the windows behind. Blur only where the glass is (alpha
-- above the soft shadows, which stay unblurred).
hl.layer_rule({ match = { namespace = "irice:.*" }, no_anim = true }) -- the shell animates itself
hl.layer_rule({ match = { namespace = "irice:.*" }, blur = true })
hl.layer_rule({ match = { namespace = "irice:.*" }, ignore_alpha = 0.4 })
-- (The menu bar takes blur: the island's glass is drawn there.)
hl.layer_rule({ match = { namespace = "irice:(wallpaper|corners|lockveil|dim)" }, blur = false }) -- (fading veils would blur, then snap sharp)

-- Menus and dropdowns of XWayland apps (Electron ones like Webex, Slack…)
-- are windows of their own with no class or title. With focus following the
-- pointer, moving onto one took focus from its app, and the menu closed.
-- They never take focus (Hyprland's own recommended rule).
hl.window_rule({
    name = "irice-xwayland-popups",
    match = { class = "^$", title = "^$", xwayland = true, float = true, fullscreen = false, pin = false },
    no_focus = true
})

-- Settings window floats like an app window
hl.window_rule({ match = { title = "^iRice Settings$" }, float = true })
hl.window_rule({ match = { title = "^iRice Settings$" }, center = true })

-- File pickers and account dialogs float, centred, on top. Opened from a
-- floating window (Settings' "Choose…", Connect Google), a tiled one would
-- sit behind it, as floating windows stay above tiled ones. The portals'
-- own windows: their file chooser, and the screen-sharing and app pickers.
for _, m in ipairs({
    { class = "^(org\\.freedesktop\\.impl\\.portal\\.desktop\\.kde|xdg-desktop-portal-gtk|xdg-desktop-portal-gnome|hyprland-share-picker)$" },
    { class = "^(org\\.irice\\.GoogleSignIn|org\\.gnome\\.OnlineAccounts|polkit-kde-authentication-agent-1|org\\.kde\\.polkit-kde-authentication-agent-1)$" },
}) do
    hl.window_rule({ match = m, float = true })
    hl.window_rule({ match = m, center = true })
end
hl.window_rule({
    match = { class = "^(org\\.freedesktop\\.impl\\.portal\\.desktop\\.kde|xdg-desktop-portal-gtk|xdg-desktop-portal-gnome)$" },
    size = "(monitor_w*0.6) (monitor_h*0.7)"
})
hl.window_rule({ match = { class = "^(org.pulseaudio.pavucontrol|nm-connection-editor|blueman-manager)$" }, float = true })

-- Fully opaque whatever the window transparency setting (Settings →
-- Appearance): video, games and YouTube keep their colours exact
for _, m in ipairs({
    { class = "(?i)^(mpv|vlc|org\\.videolan\\.VLC|io\\.github\\.celluloid_player\\.Celluloid|smplayer|com\\.github\\.rafostar\\.Clapper|org\\.gnome\\.Totem|com\\.obsproject\\.Studio)$" },
    { class = "(?i)^(steam_app.*|gamescope|heroic|lutris|net\\.lutris\\.Lutris|com\\.heroicgameslauncher\\.hgl|.*\\.exe)$" },
    { title = "(?i).*YouTube.*" },
    { title = "^(Picture[- ]in[- ][Pp]icture)$" },
}) do
    hl.window_rule({ match = m, opacity = "1.0 override 1.0 override" })
end

-- Picture in Picture (Firefox, Chromium): a small video in the bottom-right
-- corner, on every space, keeping its shape. (Move takes plain numbers here.)
hl.window_rule({
    match = { title = "^(Picture[- ]in[- ][Pp]icture)$" },
    float = true, pin = true, keep_aspect_ratio = true,
    size = "480 270", move = "monitor_w-480-24 monitor_h-270-24"
})
