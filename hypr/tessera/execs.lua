-- Programs started with the session.
hl.on("hyprland.start", function()
    -- optional plugins (hyprbars), only when some are added: with none,
    -- hyprpm's reload said "run hyprpm update" on screen at every login
    hl.exec_cmd([[command -v hyprpm >/dev/null && awk '/^\[/ && $0 != "[state]" { f = 1 } END { exit !f }' "/var/cache/hyprpm/$(id -un)/state.toml" 2>/dev/null && hyprpm reload -n]])
    local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    -- (launch-shell.sh picks the icon theme; with rust.standalone set in Settings'
    -- config it starts `tessera-ui --standalone` instead of Quickshell, and falls
    -- back to Quickshell when that binary is missing)
    hl.exec_cmd(cfg .. "/quickshell/tessera/assets/scripts/launch-shell.sh -c tessera")
    hl.exec_cmd("wl-paste --watch cliphist store")
    -- Passwords for apps (the Secret Service), unless something already
    -- provides it: a keyring unlocked at login (PAM), KWallet, KeePassXC
    hl.exec_cmd("busctl --user status org.freedesktop.secrets >/dev/null 2>&1 || gnome-keyring-daemon --start --components=secrets")
    -- launch-shell starts the session-specific idle/sleep locker.
    hl.exec_cmd("dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP QT_QPA_PLATFORMTHEME QT_QPA_PLATFORM ELECTRON_OZONE_PLATFORM_HINT XDG_MENU_PREFIX CLAUDE_PASSWORD_STORE")
    -- Apps that start with your session (~/.config/autostart, /etc/xdg/autostart),
    -- once the shell's tray is there for them; once per session
    hl.exec_cmd("python3 -I " .. cfg .. "/quickshell/tessera/assets/scripts/xdg-autostart.py --wait-tray 15")
end)

-- If the shell ever stops (a crash), it comes back within ~10 s (~5 s
-- while the screen is locked). A check is two pgreps, nothing more
do
    local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    hl.timer(function() hl.exec_cmd(cfg .. "/quickshell/tessera/assets/scripts/keep-shell.sh -c tessera") end, { timeout = 5000, type = "repeat" })
end
