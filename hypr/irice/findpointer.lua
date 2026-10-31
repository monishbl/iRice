-- Shake to find the pointer (Settings → Mouse): move the mouse quickly back
-- and forth and the shell makes the pointer big for a moment, with a ring
-- around it (irice:findPointer). Hyprland has no "pointer moved" event, so
-- the position is looked at here, inside Hyprland: every 40 ms while the
-- pointer moves, every 200 ms while it's still. Nothing is started for it.
-- The shell turns it on and off live: IRICE_FIND_POINTER (hyprctl eval).
local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local f = io.open((IRICE_USER_DIR or (cfg .. "/irice")) .. "/config.json", "r")
IRICE_FIND_POINTER = true
if f then
    local on = f:read("*a"):match('"findPointer"%s*:%s*{[^}]-"enabled"%s*:%s*(%a+)')
    f:close()
    if on == "false" then IRICE_FIND_POINTER = false end
end

local FAST, SLOW = 40, 200
local STROKE = 80      -- px: a stroke shorter than this isn't part of a shake…
local SPEED = 0.8      -- …nor one slower than this (px per ms: drawing, dragging)
local TURNS = 4        -- direction changes…
local WINDOW = 1000    -- …within this many ms
local REST = 1500      -- ms before another shake counts

-- (two timers: Hyprland's set_timeout doesn't change a running one. The
-- slow one only notices movement and wakes the fast one, which goes back to
-- sleep after a still moment)
local clock = 0
local last = nil
local still = 0
local axes = { x = { dir = 0, run = 0, since = 0 }, y = { dir = 0, run = 0, since = 0 } }
local turns = {}
local quietUntil = 0
local fast

local function turned(a, d)
    local s = axes[a]
    local sign = d > 0 and 1 or -1
    if s.dir == 0 then s.since = clock - FAST end
    if s.dir == sign or s.dir == 0 then
        s.dir = sign
        s.run = s.run + math.abs(d)
        return false
    end
    -- the other way: a turn, if the stroke before it was long and quick
    local shake = s.run >= STROKE and s.run / math.max(FAST, clock - s.since) >= SPEED
    s.dir, s.run, s.since = sign, math.abs(d), clock - FAST
    return shake
end

local function sample()
    local p = hl.get_cursor_pos()
    if not p then return nil end
    local moved = last and (math.abs(p.x - last.x) + math.abs(p.y - last.y)) or 0
    local dx, dy = last and p.x - last.x or 0, last and p.y - last.y or 0
    last = { x = p.x, y = p.y }
    return moved, dx, dy
end

local function fastTick()
    clock = clock + FAST
    local moved, dx, dy = sample()
    if not moved then return end
    if moved < 2 then
        still = still + FAST
        if still > 1200 then
            fast:set_enabled(false)
            for _, a in pairs(axes) do a.dir, a.run = 0, 0 end
            turns = {}
        end
        return
    end
    still = 0
    local t = false
    if math.abs(dx) > 3 then t = turned("x", dx) or t end
    if math.abs(dy) > 3 then t = turned("y", dy) or t end
    if t then turns[#turns + 1] = clock end
    while #turns > 0 and clock - turns[1] > WINDOW do table.remove(turns, 1) end
    if #turns >= TURNS and clock >= quietUntil then
        turns = {}
        quietUntil = clock + REST
        hl.dispatch(hl.dsp.global("irice:findPointer"))
    end
end

local function slowTick()
    if not IRICE_FIND_POINTER then
        last = nil
        return
    end
    if fast:is_enabled() then return end -- (the fast one keeps the time then)
    clock = clock + SLOW
    local moved = sample()
    if moved and moved >= 2 then
        still = 0
        fast:set_enabled(true)
    end
end

fast = hl.timer(fastTick, { timeout = FAST, type = "repeat" })
fast:set_enabled(false)
hl.timer(slowTick, { timeout = SLOW, type = "repeat" })
