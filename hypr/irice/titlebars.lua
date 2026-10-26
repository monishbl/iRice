-- macOS-style title bars via the hyprbars plugin (optional). Loaded only when
-- the plugin is present; on/off follows Settings → Appearance → Decorations,
-- read from iRice's config here and switched live by the shell.
if not (hl.plugin and hl.plugin.hyprbars) then
    return
end

local function iriceDecorations()
    local f = io.open(os.getenv("HOME") .. "/.config/irice/config.json", "r")
    if not f then return "none" end
    local s = f:read("*a")
    f:close()
    return s:match('"decorations"%s*:%s*"(%w+)"') or "none"
end

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

-- Traffic lights: close, minimize, zoom (listed right to left)
hl.plugin.hyprbars.add_button({ bg_color = "rgb(28C840)", fg_color = "rgba(0000008c)", size = 12, icon = "+", action = act('hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" })') })
hl.plugin.hyprbars.add_button({ bg_color = "rgb(FEBC2E)", fg_color = "rgba(0000008c)", size = 12, icon = "−", action = act('hl.dsp.window.move({ workspace = "special:minimized", follow = false })') })
hl.plugin.hyprbars.add_button({ bg_color = "rgb(FF5F57)", fg_color = "rgba(0000008c)", size = 12, icon = "×", action = act("hl.dsp.window.close()") })
