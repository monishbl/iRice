-- iRice keybinds. The defaults are data (keymap.json next to this file,
-- written by tools/gen-keymap.py; they follow end4); your changes from
-- Settings → Keyboard Shortcuts live in ~/.config/irice/keybinds.json:
--   keybinds   { id = { keys = "...", disabled = true } }  changes to defaults
--   shortcuts  [ { id, keys, label, action } ]           your own shortcuts
-- Labels ("Group: Action") feed the cheatsheet and Settings. MOD is SUPER
-- normally (ALT in the nested sandbox). A bad entry is skipped and logged;
-- it never stops the rest from loading.
local MOD = IRICE_MOD or "SUPER"
local here = IRICE_HYPR_DIR or (debug.getinfo(1, "S").source:match("^@(.*)/[^/]*$") or ".")
local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local userDir = IRICE_USER_DIR or (cfg .. "/irice")
-- The shell's folder (repair script, app launcher)
local shellDir = os.getenv("IRICE_ROOT") and (os.getenv("IRICE_ROOT") .. "/shell") or (cfg .. "/quickshell/irice")
local json = dofile(here .. "/json.lua")

local function log(msg)
    io.stderr:write("[irice keybinds] " .. msg .. "\n")
end

local function keysOf(k)
    k = k:gsub("%$MODKEY", MOD == "SUPER" and "Super_L" or "Alt_L"):gsub("%$MOD", MOD)
    return k
end

-- A bind's action as a Hyprland dispatcher (or function)
local function zoom(step)
    return function()
        local z = (hl.get_config("cursor:zoom_factor") or 1) + step
        hl.config({ cursor = { zoom_factor = math.max(1.0, math.min(3.0, z)) } })
    end
end
local function actionOf(a)
    if type(a) ~= "table" then return nil, "no action" end
    if a.shell then return hl.dsp.global("irice:" .. a.shell) end
    if a.exec then return hl.dsp.exec_cmd((a.exec:gsub("%$IRICE_SHELL", shellDir))) end
    if a.app then return hl.dsp.exec_cmd("IRICE_USER_DIR='" .. userDir .. "' '" .. shellDir .. "/assets/scripts/open-app.sh' " .. a.app) end
    if a.zoom then return zoom(a.zoom) end
    if a.dsp then
        local f, err = load("return " .. a.dsp, "dispatcher", "t", { hl = hl })
        if not f then return nil, err end
        local ok, d = pcall(f)
        if not ok then return nil, d end
        return d
    end
    return nil, "unknown action"
end

-- Keys already taken: a second bind on the same keys is skipped (it happens
-- when MOD is ALT in the sandbox, or a custom shortcut reuses a default's keys)
local taken = {}
local function bind(e, user)
    local keys = keysOf(e.keys)
    local norm = keys:upper():gsub("%s", "")
    if taken[norm] then
        log("skipped " .. tostring(e.id) .. ": " .. keys .. " is already " .. taken[norm])
        return
    end
    local action, err = actionOf(e.action)
    if not action then
        log("skipped " .. tostring(e.id) .. ": " .. tostring(err))
        return
    end
    local opts = {}
    for k, v in pairs(e.opts or {}) do opts[k] = v end
    opts.description = (e.group or (user and "Custom" or "Other")) .. ": " .. (e.label or e.id)
    local ok, berr = pcall(hl.bind, keys, action, opts)
    if ok then
        taken[norm] = tostring(e.id)
        -- Lua-backed global dispatchers do not get the legacy global
        -- dispatcher's automatic release event. Deliver it explicitly, even
        -- after the modifier itself has gone up. The shell's arm/cancel state
        -- still decides whether this was a lone tap or part of a chord.
        if e.action.shell == "superTap" then
            local releaseKey = keys:match("([^+]+)$"):match("^%s*(.-)%s*$")
            local released, releaseError = pcall(hl.bind, releaseKey, action, {
                release = true, ignore_mods = true, non_consuming = true,
                description = "Shell: Release tap modifier"
            })
            if not released then log("couldn't bind tap release: " .. tostring(releaseError)) end
        end
        -- Other Super shortcuts cancel a pending tap.
        if e.action.shell ~= "superTap" and norm:find("^" .. MOD .. "%+") then
            pcall(hl.bind, keys, hl.dsp.global("irice:superTapCancel"), {})
        end
    else
        log("couldn't bind " .. tostring(e.id) .. " (" .. keys .. "): " .. tostring(berr))
    end
end

local defaults = json.read(here .. "/keymap.json")
if not defaults then
    log("keymap.json is missing or broken; no default keybinds")
    defaults = {}
end
local user = json.read(userDir .. "/keybinds.json") or {}
local changes = user.keybinds or {}

-- Changed defaults first, then your own shortcuts, then the untouched
-- defaults: what you set wins a clash
local later = {}
for _, e in ipairs(defaults) do
    local c = changes[e.id]
    if not (c and c.disabled) then
        if c and c.keys then
            e.keys = c.keys
            bind(e, false)
        else
            later[#later + 1] = e
        end
    end
end
for _, e in ipairs(user.shortcuts or {}) do
    if type(e) == "table" and e.keys and not e.disabled then bind(e, true) end
end
for _, e in ipairs(later) do bind(e, false) end

-- Moving or resizing a window with Super and the mouse isn't a tap either
pcall(hl.bind, MOD .. " + mouse:272", hl.dsp.global("irice:superTapCancel"), { non_consuming = true })
pcall(hl.bind, MOD .. " + mouse:273", hl.dsp.global("irice:superTapCancel"), { non_consuming = true })
