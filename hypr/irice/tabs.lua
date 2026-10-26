-- Window tabs (Hyprland groups), styled like macOS (Settings → Appearance →
-- Window tabs):
--   hyprland  Hyprland's own tab bar, restyled: rounded tabs with titles
--   finder    the bar's space is kept (invisible) and iRice draws a Finder-
--             style tab strip there (shell/modules/tabs/TabStrip.qml)
-- Merge windows from the window menu or with Super+T; drag a window onto
-- a tab bar to add it.
local function iriceOption(key)
    local cfg = (os.getenv("XDG_CONFIG_HOME") ~= nil and os.getenv("XDG_CONFIG_HOME") ~= "") and os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    local f = io.open(cfg .. "/irice/config.json", "r")
    if not f then return nil end
    local s = f:read("*a")
    f:close()
    return s:match('"' .. key .. '"%s*:%s*"(%w+)"')
end
local finder = iriceOption("windowTabs") == "finder"

hl.config({
    group = {
        drag_into_group = 2, -- only when dropped on a tab bar
        merge_groups_on_drag = true,
        insert_after_current = true,
        col = { border_active = "rgba(ffffff38)", border_inactive = "rgba(ffffff10)" },
        groupbar = finder and {
            -- (titles and fills stay on, invisible: with both off Hyprland
            -- doesn't keep the bar's height)
            enabled = true, height = 30, render_titles = true, gradients = true,
            text_color = "rgba(00000000)", text_color_inactive = "rgba(00000000)",
            indicator_height = 0, keep_upper_gap = true, gaps_in = 0, gaps_out = 4,
            col = { active = "rgba(00000000)", inactive = "rgba(00000000)", locked_active = "rgba(00000000)", locked_inactive = "rgba(00000000)" }
        } or {
            enabled = true, height = 24, render_titles = true, gradients = true,
            font_family = "Inter", font_size = 11, font_weight_active = "semibold", font_weight_inactive = "normal",
            text_color = "rgba(ffffffee)", text_color_inactive = "rgba(ffffff99)",
            indicator_height = 0, rounding = 8, gradient_rounding = 8, gaps_in = 4, gaps_out = 4, keep_upper_gap = true,
            col = { active = "rgba(ffffff30)", inactive = "rgba(ffffff0d)", locked_active = "rgba(ffffff30)", locked_inactive = "rgba(ffffff0d)" }
        }
    }
})
