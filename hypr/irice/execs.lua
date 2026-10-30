-- Programs started with the session.
hl.on("hyprland.start", function()
    hl.exec_cmd("command -v hyprpm >/dev/null && hyprpm reload -n") -- optional plugins (hyprbars)
    local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    hl.exec_cmd(cfg .. "/quickshell/irice/assets/scripts/launch-shell.sh -c irice") -- picks the icon theme
    hl.exec_cmd("wl-paste --watch cliphist store")
    hl.exec_cmd("gnome-keyring-daemon --start --components=secrets")
    -- launch-shell starts the session-specific idle/sleep locker.
    hl.exec_cmd("dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP QT_QPA_PLATFORMTHEME QT_QPA_PLATFORM ELECTRON_OZONE_PLATFORM_HINT XDG_MENU_PREFIX")
end)

-- If the shell ever stops (a crash), it comes back within a minute
do
    local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    hl.timer(function() hl.exec_cmd(cfg .. "/quickshell/irice/assets/scripts/keep-shell.sh -c irice") end, { timeout = 30000, type = "repeat" })
end
