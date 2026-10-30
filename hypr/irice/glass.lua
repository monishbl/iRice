-- Liquid Glass: loads the irice-glass plugin, if it's been built
-- (tools/build-plugin.sh), once Hyprland has started (loading while the config
-- is parsed is too early: there's no renderer yet). If it's missing or built
-- for another Hyprland version, nothing breaks: the shell uses Frosted glass.
local data = (os.getenv("XDG_DATA_HOME") ~= nil and os.getenv("XDG_DATA_HOME") ~= "") and os.getenv("XDG_DATA_HOME") or (os.getenv("HOME") .. "/.local/share")
local plugin = IRICE_GLASS_PLUGIN or (data .. "/irice/irice-glass.so")
local f = io.open(plugin, "r")
if f then
    f:close()
    hl.on("hyprland.start", function()
        hl.exec_cmd("hyprctl plugin load '" .. plugin .. "'")
    end)
end
