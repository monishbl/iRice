-- Cursor (Settings → Appearance → Pointer): the macOS pointer (apple_cursor
-- for apps, apple_hyprcursor for Hyprland itself, sharper at any scale) or
-- Bibata Modern Classic like end4, when installed; else the system's. Same
-- size everywhere. The shell switches it live; this sets it at login.
local function installed(name)
    for _, dir in ipairs({ "/usr/share/icons/", (os.getenv("HOME") or "") .. "/.local/share/icons/", (os.getenv("HOME") or "") .. "/.icons/" }) do
        local f = io.open(dir .. name .. "/index.theme", "r") or io.open(dir .. name .. "/manifest.hl", "r")
        if f then
            f:close()
            return true
        end
    end
    return false
end

local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local f = io.open((IRICE_USER_DIR or (cfg .. "/irice")) .. "/config.json", "r")
local choice = "macos"
if f then
    choice = f:read("*a"):match('"cursor"%s*:%s*"(%w+)"') or "macos"
    f:close()
end

hl.env("XCURSOR_SIZE", "24")
hl.env("HYPRCURSOR_SIZE", "24")
local theme, hyprTheme
if choice == "bibata" and installed("Bibata-Modern-Classic") then
    theme = "Bibata-Modern-Classic"
elseif installed("macOS") then
    theme = "macOS"
    if installed("macOS-hypr") then hyprTheme = "macOS-hypr" end
end
if theme then
    hl.env("XCURSOR_THEME", theme)
    hl.exec_cmd("gsettings set org.gnome.desktop.interface cursor-theme " .. theme)
end
if hyprTheme then
    hl.env("HYPRCURSOR_THEME", hyprTheme)
end
