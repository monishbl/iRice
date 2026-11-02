-- macOS-style title bars via the hyprbars plugin (optional). Loaded only when
-- the plugin is present; on/off follows Settings → Appearance → Decorations,
-- read from iRice's config here and switched live by the shell. In the
-- Material look: a tonal bar with round icon buttons on the right
-- (minimize, maximize, close), as ChromeOS draws a window; the shell reloads
-- the config when the look changes.
if not (hl.plugin and hl.plugin.hyprbars) then
    return
end

local function iriceConfig()
    local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    local f = io.open(cfg .. "/irice/config.json", "r")
    if not f then return "" end
    local s = f:read("*a")
    f:close()
    return s
end
local config = iriceConfig()
local function iriceDecorations()
    return config:match('"decorations"%s*:%s*"(%w+)"') or "none"
end
local material = config:match('"look"%s*:%s*"(%w+)"') == "material"

local function act(lua)
    return "hyprctl dispatch '" .. lua .. "'"
end

hl.config({
    plugin = {
        hyprbars = {
            enabled = iriceDecorations() == "titlebars",
            bar_height = 28,
            bar_color = "rgba(1c1c20d9)",
            bar_blur = true,
            ["col.text"] = "rgba(ffffffcc)",
            bar_text_font = "Inter",
            bar_text_size = 11,
            bar_text_weight = "semibold",
            bar_text_align = "center",
            bar_buttons_alignment = "left",
            bar_part_of_window = true,
            bar_precedence_over_border = true,
            bar_padding = 12,
            bar_button_padding = 8,
            icon_on_hover = true,
            inactive_button_color = "rgba(ffffff33)",
            on_double_click = act('hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" })')
        }
    }
})

if material then
    -- A dark tonal bar (opaque; Glass's is dark too) and plain round icon
    -- buttons in a soft light grey, on the right
    hl.config({
        plugin = {
            hyprbars = {
                bar_height = 32,
                bar_color = "rgb(1d1b20)",
                bar_blur = false,
                ["col.text"] = "rgba(e6e1e5ff)",
                bar_text_align = "left",
                bar_buttons_alignment = "right",
                bar_padding = 16,
                bar_button_padding = 10,
                icon_on_hover = false,
                inactive_button_color = "rgba(00000000)"
            }
        }
    })
    -- Minimize, maximize, close (listed right to left)
    local function button(icon, action)
        hl.plugin.hyprbars.add_button({ bg_color = "rgba(00000000)", fg_color = "rgba(cac4d0ff)", size = 16, icon = icon, action = action })
    end
    button("×", act("hl.dsp.window.close()"))
    button("□", act('hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" })'))
    button("−", act('hl.dsp.window.move({ workspace = "special:minimized", follow = false })'))
    return
end

-- Traffic lights: close, minimize, zoom (listed right to left)
hl.plugin.hyprbars.add_button({ bg_color = "rgb(28C840)", fg_color = "rgba(0000008c)", size = 12, icon = "+", action = act('hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" })') })
hl.plugin.hyprbars.add_button({ bg_color = "rgb(FEBC2E)", fg_color = "rgba(0000008c)", size = 12, icon = "−", action = act('hl.dsp.window.move({ workspace = "special:minimized", follow = false })') })
hl.plugin.hyprbars.add_button({ bg_color = "rgb(FF5F57)", fg_color = "rgba(0000008c)", size = 12, icon = "×", action = act("hl.dsp.window.close()") })
