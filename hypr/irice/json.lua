-- A small JSON reader for iRice's own data files (keymap, gestures and the
-- user's changes to them, written by Settings). Returns nil and a message on
-- bad input instead of raising, so a broken file never stops Hyprland.
local M = {}

local function decode(s)
    local pos = 1
    local function err(msg) error({ msg = msg .. " at " .. pos }) end
    local function ws()
        pos = s:find("[^ \t\r\n]", pos) or #s + 1
    end
    local value
    local function str()
        pos = pos + 1
        local out = {}
        while true do
            local c = s:sub(pos, pos)
            if c == "" then err("unfinished string") end
            if c == '"' then
                pos = pos + 1
                return table.concat(out)
            elseif c == "\\" then
                local e = s:sub(pos + 1, pos + 1)
                local map = { ['"'] = '"', ["\\"] = "\\", ["/"] = "/", b = "\b", f = "\f", n = "\n", r = "\r", t = "\t" }
                if map[e] then
                    out[#out + 1] = map[e]
                    pos = pos + 2
                elseif e == "u" then
                    local cp = tonumber(s:sub(pos + 2, pos + 5), 16) or err("bad \\u escape")
                    pos = pos + 6
                    -- a surrogate pair
                    if cp >= 0xD800 and cp <= 0xDBFF and s:sub(pos, pos + 1) == "\\u" then
                        local lo = tonumber(s:sub(pos + 2, pos + 5), 16)
                        if lo and lo >= 0xDC00 and lo <= 0xDFFF then
                            cp = 0x10000 + (cp - 0xD800) * 0x400 + (lo - 0xDC00)
                            pos = pos + 6
                        end
                    end
                    out[#out + 1] = utf8.char(cp)
                else
                    err("bad escape")
                end
            else
                local j = s:find('["\\]', pos) or #s + 1
                out[#out + 1] = s:sub(pos, j - 1)
                pos = j
            end
        end
    end
    function value()
        ws()
        local c = s:sub(pos, pos)
        if c == "{" then
            pos = pos + 1
            local t = {}
            ws()
            if s:sub(pos, pos) == "}" then pos = pos + 1 return t end
            while true do
                ws()
                if s:sub(pos, pos) ~= '"' then err("expected a key") end
                local k = str()
                ws()
                if s:sub(pos, pos) ~= ":" then err("expected ':'") end
                pos = pos + 1
                t[k] = value()
                ws()
                local d = s:sub(pos, pos)
                pos = pos + 1
                if d == "}" then return t end
                if d ~= "," then err("expected ',' or '}'") end
            end
        elseif c == "[" then
            pos = pos + 1
            local t = {}
            ws()
            if s:sub(pos, pos) == "]" then pos = pos + 1 return t end
            while true do
                t[#t + 1] = value()
                ws()
                local d = s:sub(pos, pos)
                pos = pos + 1
                if d == "]" then return t end
                if d ~= "," then err("expected ',' or ']'") end
            end
        elseif c == '"' then
            return str()
        elseif s:sub(pos, pos + 3) == "true" then
            pos = pos + 4
            return true
        elseif s:sub(pos, pos + 4) == "false" then
            pos = pos + 5
            return false
        elseif s:sub(pos, pos + 3) == "null" then
            pos = pos + 4
            return nil
        else
            local num = s:match("^-?%d+%.?%d*[eE]?[-+]?%d*", pos)
            if not num or num == "" then err("unexpected '" .. c .. "'") end
            pos = pos + #num
            return tonumber(num)
        end
    end
    local ok, res = pcall(value)
    if not ok then return nil, type(res) == "table" and res.msg or tostring(res) end
    return res
end

M.decode = decode

-- Reads and decodes a file; nil when it's missing or broken
function M.read(path)
    local f = io.open(path, "r")
    if not f then return nil, "missing" end
    local s = f:read("*a")
    f:close()
    return decode(s)
end

return M
