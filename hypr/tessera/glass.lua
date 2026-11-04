-- Liquid Glass: loads the tessera-glass plugin, if it's been built
-- (tools/build-plugin.sh), once Hyprland has started (loading while the config
-- is parsed is too early: there's no renderer yet). If it's missing or built
-- for another Hyprland version, nothing breaks: the shell uses Frosted glass.
local data = (os.getenv("XDG_DATA_HOME") ~= nil and os.getenv("XDG_DATA_HOME") ~= "") and os.getenv("XDG_DATA_HOME") or (os.getenv("HOME") .. "/.local/share")
-- The first that's there: TESSERA_GLASS_PLUGIN (a path, or a list: the
-- repo's own build in the tty session), then the installed one. A plugin
-- built while Tessera was called iRice (irice-glass.so, in either folder)
-- loads until the next build replaces it.
local candidates = {}
if type(TESSERA_GLASS_PLUGIN) == "table" then
    for _, p in ipairs(TESSERA_GLASS_PLUGIN) do table.insert(candidates, p) end
elseif TESSERA_GLASS_PLUGIN then
    table.insert(candidates, TESSERA_GLASS_PLUGIN)
end
table.insert(candidates, data .. "/tessera/tessera-glass.so")
table.insert(candidates, data .. "/tessera/irice-glass.so")
table.insert(candidates, data .. "/irice/irice-glass.so")
for _, plugin in ipairs(candidates) do
    local f = io.open(plugin, "r")
    if f then
        f:close()
        hl.on("hyprland.start", function()
            hl.exec_cmd("hyprctl plugin load '" .. plugin .. "'")
        end)
        break
    end
end
