-- Programs started with the session.
hl.on("hyprland.start", function()
    hl.exec_cmd("command -v hyprpm >/dev/null && hyprpm reload -n") -- optional plugins (hyprbars)
    hl.exec_cmd(os.getenv("HOME") .. "/.config/quickshell/irice/assets/scripts/launch-shell.sh -c irice") -- picks the icon theme
    hl.exec_cmd("wl-paste --watch cliphist store")
    hl.exec_cmd("gnome-keyring-daemon --start --components=secrets")
    hl.exec_cmd("hypridle")
    hl.exec_cmd("dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP")
end)
