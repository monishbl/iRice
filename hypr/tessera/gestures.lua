-- Touchpad gestures. The defaults are data (gestures.json next to this file:
-- end4's set plus Tessera's on spare finger counts); your changes from
-- Settings → Trackpad live in ~/.config/tessera/keybinds.json under
-- "gestures": { changes = { id = { fingers, direction, action, disabled } },
-- custom = [ ... ] }. An action is a Hyprland gesture action ("workspace",
-- "move", "fullscreen", "close", "float", ...) or { shell = name }.
-- Scroll on the menu bar's island (or Super+scroll) also steps through spaces.
local here = TESSERA_HYPR_DIR or (debug.getinfo(1, "S").source:match("^@(.*)/[^/]*$") or ".")
local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local userDir = TESSERA_USER_DIR or (cfg .. "/tessera")
local json = dofile(here .. "/json.lua")

local function log(msg)
    io.stderr:write("[tessera gestures] " .. msg .. "\n")
end

local function actionOf(a)
    if type(a) == "string" then return a end
    if type(a) == "table" and a.shell then
        local name = a.shell
        return function() hl.dispatch(hl.dsp.global("tessera:" .. name)) end
    end
    return nil
end

-- Hyprland's "pinchin" runs on a spread and "pinchout" on a pinch (seen on
-- a real trackpad): swapped here, so a gesture named Pinch (fingers
-- together) is one
local FINGERS = { pinchin = "pinchout", pinchout = "pinchin" }

-- Long presses ("hold": fingers resting still) aren't Hyprland gestures:
-- the tessera-glass plugin sees them and calls TESSERA_HOLD(fingers) (its
-- hold.cpp). Only Tessera actions: Hyprland's own need a swipe.
TESSERA_HOLDS = {}
function TESSERA_HOLD(fingers)
    local run = TESSERA_HOLDS[tonumber(fingers)]
    if run then run() end
end
-- (a glass plugin built before iRice became Tessera calls this name)
IRICE_HOLD = TESSERA_HOLD

local function add(g)
    local action = actionOf(g.action)
    if not action or not g.fingers or not g.direction then
        log("skipped " .. tostring(g.id))
        return
    end
    if g.direction == "hold" then
        if type(action) ~= "function" then
            log("skipped " .. tostring(g.id) .. ": a long press runs Tessera actions only")
        else
            TESSERA_HOLDS[g.fingers] = action
        end
        return
    end
    local ok, err = pcall(hl.gesture, { fingers = g.fingers, direction = FINGERS[g.direction] or g.direction, action = action })
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
