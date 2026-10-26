-- Touchpad gestures. The defaults are data (gestures.json next to this file:
-- end4's set plus iRice's on spare finger counts); your changes from
-- Settings → Trackpad live in ~/.config/irice/keybinds.json under
-- "gestures": { changes = { id = { fingers, direction, action, disabled } },
-- custom = [ ... ] }. An action is a Hyprland gesture action ("workspace",
-- "move", "fullscreen", "close", "float", ...) or { shell = name }.
-- Scroll on the menu bar's island (or Super+scroll) also steps through spaces.
local here = IRICE_HYPR_DIR or (debug.getinfo(1, "S").source:match("^@(.*)/[^/]*$") or ".")
local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local userDir = IRICE_USER_DIR or (cfg .. "/irice")
local json = dofile(here .. "/json.lua")

local function log(msg)
    io.stderr:write("[irice gestures] " .. msg .. "\n")
end

local function actionOf(a)
    if type(a) == "string" then return a end
    if type(a) == "table" and a.shell then
        local name = a.shell
        return function() hl.dispatch(hl.dsp.global("irice:" .. name)) end
    end
    return nil
end

local function add(g)
    local action = actionOf(g.action)
    if not action or not g.fingers or not g.direction then
        log("skipped " .. tostring(g.id))
        return
    end
    local ok, err = pcall(hl.gesture, { fingers = g.fingers, direction = g.direction, action = action })
    if not ok then log("couldn't add " .. tostring(g.id) .. ": " .. tostring(err)) end
end

local defaults = json.read(here .. "/gestures.json") or {}
local user = (json.read(userDir .. "/keybinds.json") or {}).gestures or {}
local changes = user.changes or {}
for _, g in ipairs(defaults) do
    local c = changes[g.id]
    if not (c and c.disabled) then
        if c then
            for k, v in pairs(c) do g[k] = v end
        end
        add(g)
    end
end
for _, g in ipairs(user.custom or {}) do
    if type(g) == "table" and not g.disabled then add(g) end
end

hl.config({ gestures = { workspace_swipe_touch = true } })
